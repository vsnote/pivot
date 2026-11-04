use crate::db::Provider;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

static RR_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone)]
pub struct RouteCandidate {
    pub provider: Provider,
    pub target_model: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RouteMatch {
    pub primary: RouteCandidate,
    pub fallbacks: Vec<RouteCandidate>,
}

/// One provider/model decision, before an API key is attached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedCandidate {
    pub provider_id: String,
    pub model: String,
}

/// Resolution result for an incoming model name. `primary` is the provider the
/// gateway prefers, `fallbacks` is the queue it degrades through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlan {
    pub primary: PlannedCandidate,
    pub fallbacks: Vec<PlannedCandidate>,
}

pub struct RoutingEngine {
    session_map: Mutex<HashMap<String, usize>>, // (session_id + provider_id) -> key_index
}

impl Default for RoutingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RoutingEngine {
    pub fn new() -> Self {
        Self {
            session_map: Mutex::new(HashMap::new()),
        }
    }

    /// Select API key using session affinity or round-robin
    pub fn select_api_key(&self, provider: &Provider, session_id: Option<&str>) -> Option<String> {
        if provider.api_keys.is_empty() {
            return None;
        }

        let key_count = provider.api_keys.len();
        if key_count == 1 {
            return Some(provider.api_keys[0].clone());
        }

        if let Some(sess) = session_id {
            let cache_key = format!("{}:{}", provider.id, sess);
            let mut map = self.session_map.lock().unwrap();
            if let Some(&idx) = map.get(&cache_key) {
                if idx < key_count {
                    return Some(provider.api_keys[idx].clone());
                }
            }
            // Hash session to an index for deterministic affinity
            let mut hash: usize = 0;
            for b in sess.bytes() {
                hash = hash.wrapping_mul(31).wrapping_add(b as usize);
            }
            let idx = hash % key_count;
            map.insert(cache_key, idx);
            return Some(provider.api_keys[idx].clone());
        }

        // Round robin
        let current = RR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let idx = current % key_count;
        Some(provider.api_keys[idx].clone())
    }

    /// Resolve routing decision with primary candidate and fallback queue
    pub fn resolve_route(
        &self,
        incoming_model: &str,
        session_id: Option<&str>,
        providers: &[Provider],
    ) -> Result<RouteMatch, String> {
        let plan = plan_route(incoming_model, providers)?;
        let provider_map: HashMap<&str, &Provider> =
            providers.iter().map(|p| (p.id.as_str(), p)).collect();

        let to_candidate = |planned: &PlannedCandidate| -> Result<RouteCandidate, String> {
            let provider = provider_map.get(planned.provider_id.as_str()).ok_or_else(|| {
                format!(
                    "Provider {} disappeared while routing {}",
                    planned.provider_id, incoming_model
                )
            })?;
            Ok(RouteCandidate {
                provider: (*provider).clone(),
                target_model: planned.model.clone(),
                api_key: self.select_api_key(provider, session_id),
            })
        };

        Ok(RouteMatch {
            primary: to_candidate(&plan.primary)?,
            fallbacks: plan
                .fallbacks
                .iter()
                .map(to_candidate)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

/// Decide which provider serves `incoming_model` and which model name each
/// candidate must be asked for.
///
/// The provider configuration alone answers this, in order:
/// 1. `provider/model` prefix — the caller named a provider explicitly
/// 2. exact model name declared by an enabled provider
/// 3. normalized model name declared by an enabled provider
/// 4. the provider marked as default, asked for its own `default_model`
pub fn plan_route(
    incoming_model: &str,
    providers: &[Provider],
) -> Result<RoutePlan, String> {
    plan_route_inner(incoming_model, providers, &mut None)
}

/// One evaluated step in the resolution pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteStep {
    pub layer: String,
    pub matched: bool,
    pub detail: String,
}

/// `plan_route` plus the trace of every layer it tried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteExplanation {
    pub incoming: String,
    pub steps: Vec<RouteStep>,
    pub plan: Option<RoutePlan>,
    pub error: Option<String>,
}

/// Resolve `model` and explain the result. Unlike `plan_route`, this never
/// fails: the error is part of the explanation.
pub fn explain_route(
    incoming_model: &str,
    providers: &[Provider],
) -> RouteExplanation {
    let mut steps = Vec::new();
    let result = {
        let mut trace = Some(&mut steps);
        plan_route_inner(incoming_model, providers, &mut trace)
    };
    let (plan, error) = match result {
        Ok(plan) => (Some(plan), None),
        Err(error) => (None, Some(error)),
    };
    RouteExplanation {
        incoming: incoming_model.to_string(),
        steps,
        plan,
        error,
    }
}

/// Record a trace step. The closure only runs when a trace is attached, so the
/// gateway's hot path stays allocation-free.
fn note(
    trace: &mut Option<&mut Vec<RouteStep>>,
    layer: &str,
    matched: bool,
    detail: impl FnOnce() -> String,
) {
    if let Some(steps) = trace.as_deref_mut() {
        steps.push(RouteStep {
            layer: layer.to_string(),
            matched,
            detail: detail(),
        });
    }
}

fn plan_route_inner(
    incoming_model: &str,
    providers: &[Provider],
    trace: &mut Option<&mut Vec<RouteStep>>,
) -> Result<RoutePlan, String> {
    let mut active_providers: Vec<&Provider> = providers.iter().filter(|p| p.enabled).collect();
    if active_providers.is_empty() {
        return Err("No active providers enabled in Pivot".to_string());
    }
    // Candidate order is the provider priority, independent of how the caller
    // happened to hand the list over.
    active_providers.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.name.cmp(&b.name))
    });

    let provider_map: HashMap<&str, &Provider> = active_providers
        .iter()
        .map(|p| (p.id.as_str(), *p))
        .collect();

    // 1. Prefix routing (e.g. "openai/gpt-4o", "anthropic/claude-3-7-sonnet"):
    // the model name after the prefix is forwarded as requested.
    if let Some((prefix, sub_model)) = incoming_model.split_once('/') {
        let prefix_lower = prefix.to_lowercase();
        let matched = provider_map
            .get(prefix_lower.as_str())
            .copied()
            .or_else(|| {
                active_providers
                    .iter()
                    .find(|p| {
                        p.name.to_lowercase() == prefix_lower
                            || p.name.to_lowercase().replace(' ', "-") == prefix_lower
                    })
                    .copied()
            });
        if let Some(provider) = matched {
            note(trace, "prefix", true, || {
                format!("前缀 {} 直接命中服务商 {}", prefix, provider.name)
            });
            return Ok(RoutePlan {
                primary: planned(provider, sub_model),
                fallbacks: automatic_fallbacks(&active_providers, &provider.id, sub_model),
            });
        }
        note(trace, "prefix", false, || {
            format!("前缀 {} 不对应任何已启用的服务商", prefix)
        });
    } else {
        note(trace, "prefix", false, || "模型名不含服务商前缀".to_string());
    }

    // 2. Exact model name declared by an enabled provider.
    if let Some(provider) = active_providers
        .iter()
        .find(|p| lists_model(p, incoming_model))
    {
        note(trace, "exact", true, || {
            format!("{} 在服务商优先级中最先声明该模型", provider.name)
        });
        return Ok(RoutePlan {
            primary: planned(provider, incoming_model),
            fallbacks: automatic_fallbacks(&active_providers, &provider.id, incoming_model),
        });
    }
    note(trace, "exact", false, || {
        "没有已启用的服务商精确声明该模型".to_string()
    });

    // 2.5. Smart normalized match (e.g. date versions, bracketed [1M], latest suffixes)
    for provider in &active_providers {
        if let Some(matched) = find_declared_or_normalized_model(provider, incoming_model) {
            note(trace, "normalized", true, || {
                format!("{} 通过归一化匹配到 {}", provider.name, matched)
            });
            return Ok(RoutePlan {
                primary: planned(provider, &matched),
                fallbacks: automatic_fallbacks(&active_providers, &provider.id, &matched),
            });
        }
    }
    note(trace, "normalized", false, || {
        "没有可归一化匹配的模型".to_string()
    });

    // 2.8. Provider family heuristic match (e.g. deepseek-* -> deepseek provider)
    let lower_incoming = incoming_model.to_lowercase();
    if lower_incoming.starts_with("deepseek-") || lower_incoming.starts_with("deepseek/") {
        if let Some(ds_provider) = active_providers.iter().find(|p| {
            p.name.to_lowercase().contains("deepseek") || p.preset_id.as_deref() == Some("deepseek")
        }) {
            let target = ds_provider
                .default_model
                .as_deref()
                .or_else(|| ds_provider.models.first().map(String::as_str))
                .unwrap_or(incoming_model);
            note(trace, "family", true, || {
                format!("厂商家族推断：DeepSeek 模型由 {} 承接 (目标 {})", ds_provider.name, target)
            });
            return Ok(RoutePlan {
                primary: planned(ds_provider, target),
                fallbacks: automatic_fallbacks(&active_providers, &ds_provider.id, target),
            });
        }
    }

    // 3. Unknown model name: hand it to the default provider / model.
    let Some(default_provider) = active_providers
        .iter()
        .find(|p| p.default_model.is_some())
        .or_else(|| active_providers.iter().find(|p| p.is_default))
    else {
        note(trace, "default", false, || {
            "没有配置默认模型".to_string()
        });
        return Err(format!(
            "Model '{}' is not declared by any enabled provider and no default provider is configured; mark one provider as default in Pivot",
            incoming_model
        ));
    };
    let model = candidate_model(default_provider, incoming_model)
        .unwrap_or_else(|| incoming_model.to_string());
    note(trace, "default", true, || {
        format!(
            "未命中前几层，交给默认模型 {} / {}",
            default_provider.name, model
        )
    });

    Ok(RoutePlan {
        primary: planned(default_provider, &model),
        fallbacks: automatic_fallbacks(&active_providers, &default_provider.id, &model),
    })
}

fn planned(provider: &Provider, model: &str) -> PlannedCandidate {
    PlannedCandidate {
        provider_id: provider.id.clone(),
        model: model.to_string(),
    }
}

fn lists_model(provider: &Provider, model: &str) -> bool {
    provider
        .models
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(model))
}

pub fn normalize_model_candidates(model: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return candidates;
    }

    // 1. Strip brackets like `[1M]`, `[thinking]`
    let clean = if let Some(idx) = trimmed.find('[') {
        trimmed[..idx].trim()
    } else {
        trimmed
    };

    // 2. Strip -latest suffix
    let without_latest = if clean.ends_with("-latest") {
        &clean[..clean.len() - 7]
    } else {
        clean
    };

    // 3. Strip date suffix -\d{8} or -\d{4}-\d{2}-\d{2}
    let without_date = if let Ok(re8) = Regex::new(r"-\d{8}$") {
        if let Some(m) = re8.find(without_latest) {
            &without_latest[..m.start()]
        } else if let Ok(re_iso) = Regex::new(r"-\d{4}-\d{2}-\d{2}$") {
            if let Some(m) = re_iso.find(without_latest) {
                &without_latest[..m.start()]
            } else {
                without_latest
            }
        } else {
            without_latest
        }
    } else {
        without_latest
    };

    if without_date != trimmed && !without_date.is_empty() {
        candidates.push(without_date.to_string());
    }

    // 4. Common shorthands
    match without_date.to_lowercase().as_str() {
        "sonnet" => {
            candidates.push("claude-3-7-sonnet".to_string());
            candidates.push("claude-3-5-sonnet".to_string());
        }
        "haiku" => {
            candidates.push("claude-3-5-haiku".to_string());
        }
        "opus" => {
            candidates.push("claude-3-opus".to_string());
        }
        "deepseek-chat" | "deepseek-v3" | "deepseek" => {
            candidates.push("deepseek-flash".to_string());
            candidates.push("deepseek-v4-pro".to_string());
        }
        "deepseek-reasoner" | "deepseek-r1" => {
            candidates.push("deepseek-v4-pro".to_string());
            candidates.push("deepseek-flash".to_string());
        }
        _ => {}
    }

    candidates
}

fn find_declared_or_normalized_model(provider: &Provider, incoming: &str) -> Option<String> {
    if lists_model(provider, incoming) {
        return Some(incoming.to_string());
    }
    for candidate in normalize_model_candidates(incoming) {
        if lists_model(provider, &candidate) {
            return Some(candidate);
        }
    }
    for declared in &provider.models {
        for candidate in normalize_model_candidates(declared) {
            if candidate.eq_ignore_ascii_case(incoming) {
                return Some(declared.clone());
            }
        }
    }
    None
}

fn declared_default_model(provider: &Provider) -> Option<String> {
    provider
        .default_model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string)
}

/// Model name to ask `provider` for when `model` is requested: the name itself
/// when the provider declares it (or matches via normalization), otherwise the provider's own default model.
/// `None` means this provider cannot serve the request at all.
fn candidate_model(provider: &Provider, model: &str) -> Option<String> {
    if let Some(matched) = find_declared_or_normalized_model(provider, model) {
        Some(matched)
    } else {
        declared_default_model(provider)
    }
}

/// Automatic fallback queue for `model`: providers that declare the very same
/// model name (highest priority first). Cross-model rewriting is forbidden
/// to ensure predictability and compatibility.
fn automatic_fallbacks(
    active_providers: &[&Provider],
    excluded_id: &str,
    model: &str,
) -> Vec<PlannedCandidate> {
    let mut same_name = Vec::new();

    for provider in active_providers.iter().filter(|p| p.id != excluded_id) {
        if let Some(matched) = find_declared_or_normalized_model(provider, model) {
            same_name.push(planned(provider, &matched));
        }
    }

    same_name
}

/// Helper function to match pattern against model name (supports wildcard `*`, `?`, `[...]` or regex)
pub fn pattern_matches(pattern: &str, text: &str) -> bool {
    if pattern == "*" || pattern.eq_ignore_ascii_case(text) {
        return true;
    }

    // Try direct regex first
    if let Ok(re) = Regex::new(&format!("(?i)^{}$", pattern)) {
        if re.is_match(text) {
            return true;
        }
    }

    // Convert glob pattern: `*` -> `.*`, `?` -> `.`, preserves `[...]` classes
    let mut regex_str = String::from("^");
    let mut in_bracket = false;
    for c in pattern.chars() {
        match c {
            '[' => {
                in_bracket = true;
                regex_str.push('[');
            }
            ']' => {
                in_bracket = false;
                regex_str.push(']');
            }
            '*' if !in_bracket => regex_str.push_str(".*"),
            '?' if !in_bracket => regex_str.push('.'),
            '.' | '+' | '(' | ')' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex_str.push('\\');
                regex_str.push(c);
            }
            _ => regex_str.push(c),
        }
    }
    regex_str.push('$');

    if let Ok(re) = Regex::new(&format!("(?i){}", regex_str)) {
        return re.is_match(text);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(id: &str, models: &[&str], priority: i32) -> Provider {
        Provider {
            id: id.to_string(),
            name: id.to_string(),
            protocol: "openai".to_string(),
            base_url: format!("https://{}.example/v1", id),
            preset_id: None,
            auth_type: None,
            api_keys: vec![format!("sk-{}", id)],
            models: models.iter().map(|m| m.to_string()).collect(),
            default_model: None,
            is_default: false,
            enabled: true,
            priority,
            weight: 1,
            timeout_secs: 60,
            latency_ms: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn with_default_model(mut provider: Provider, model: &str) -> Provider {
        provider.default_model = Some(model.to_string());
        provider
    }

    fn as_default(mut provider: Provider) -> Provider {
        provider.is_default = true;
        provider
    }

    #[test]
    fn prefix_routing_pins_the_named_provider_and_keeps_its_model() {
        let providers = vec![
            provider("anthropic", &["claude-3-7-sonnet"], 10),
            provider("aggregator", &["claude-3-7-sonnet"], 5),
        ];

        let plan = plan_route("anthropic/claude-3-7-sonnet", &providers).unwrap();

        assert_eq!(plan.primary.provider_id, "anthropic");
        assert_eq!(plan.primary.model, "claude-3-7-sonnet");
        assert_eq!(plan.fallbacks.len(), 1);
        assert_eq!(plan.fallbacks[0].provider_id, "aggregator");
        assert_eq!(plan.fallbacks[0].model, "claude-3-7-sonnet");
    }

    #[test]
    fn exact_model_match_uses_priority_and_ignores_disabled_providers() {
        let mut disabled = provider("antigravity", &["gemini-3.8-flash-high"], 99);
        disabled.enabled = false;
        let providers = vec![
            provider("fallback-a", &["gemini-3.8-flash-high"], 5),
            provider("antigravity", &["gemini-3.8-flash-high"], 20),
            disabled,
        ];

        let plan = plan_route("gemini-3.8-flash-high", &providers).unwrap();

        assert_eq!(plan.primary.provider_id, "antigravity");
        assert_eq!(plan.primary.model, "gemini-3.8-flash-high");
        assert_eq!(plan.fallbacks.len(), 1);
        assert_eq!(plan.fallbacks[0].provider_id, "fallback-a");
        assert_eq!(plan.fallbacks[0].model, "gemini-3.8-flash-high");
    }

    #[test]
    fn unknown_model_is_rewritten_by_the_default_provider() {
        let providers = vec![
            provider("deepseek", &["deepseek-v4-pro"], 20),
            with_default_model(as_default(provider("antigravity", &["gemini-3.8-flash-high"], 10)), "gemini-3.8-flash-high"),
        ];

        let plan = plan_route("claude-3-5-haiku-20241022", &providers).unwrap();

        assert_eq!(plan.primary.provider_id, "antigravity");
        assert_eq!(plan.primary.model, "gemini-3.8-flash-high");
        assert!(plan.fallbacks.is_empty(), "deepseek does not declare that model");
    }

    #[test]
    fn fallbacks_only_route_to_providers_with_the_same_model() {
        let providers = vec![
            with_default_model(as_default(provider("primary", &["claude-3-7-sonnet"], 30)), "gemini-3.8-flash-high"),
            with_default_model(provider("renaming", &["claude-4"], 20), "deepseek-v4-pro"),
            provider("same-name", &["claude-3-7-sonnet"], 10),
            provider("unrelated", &["qwen3-max"], 5),
        ];

        let plan = plan_route("claude-3-7-sonnet", &providers).unwrap();

        assert_eq!(plan.primary.provider_id, "primary");
        let chain: Vec<(&str, &str)> = plan
            .fallbacks
            .iter()
            .map(|c| (c.provider_id.as_str(), c.model.as_str()))
            .collect();
        assert_eq!(
            chain,
            vec![
                ("same-name", "claude-3-7-sonnet"),
            ]
        );
    }

    #[test]
    fn bare_default_provider_passes_unknown_models_through() {
        let providers = vec![as_default(provider("catch-all", &["some-model"], 10))];

        let plan = plan_route("brand-new-model", &providers).unwrap();

        assert_eq!(plan.primary.provider_id, "catch-all");
        assert_eq!(plan.primary.model, "brand-new-model");
        assert!(plan.fallbacks.is_empty());
    }

    #[test]
    fn unknown_model_without_default_provider_is_a_clear_error() {
        let providers = vec![provider("deepseek", &["deepseek-v4-pro"], 10)];

        let error = plan_route("claude-3-7-sonnet", &providers).unwrap_err();

        assert!(error.contains("claude-3-7-sonnet"), "{}", error);
        assert!(error.contains("default provider"), "{}", error);
    }

    #[test]
    fn disabled_default_provider_does_not_resolve_unknown_models() {
        let mut disabled = as_default(with_default_model(provider("offline", &[], 10), "gemini-3.8-flash-high"));
        disabled.enabled = false;
        let providers = vec![provider("deepseek", &["deepseek-v4-pro"], 5), disabled];

        assert!(plan_route("claude-3-7-sonnet", &providers).is_err());
    }

    #[test]
    fn resolve_route_attaches_provider_configuration_and_keeps_session_affinity() {
        let engine = RoutingEngine::new();
        let providers = vec![
            provider("anthropic", &["claude-3-7-sonnet"], 20),
            provider("antigravity", &["claude-3-7-sonnet"], 10),
        ];

        let matched = engine
            .resolve_route("claude-3-7-sonnet", Some("session-1"), &providers)
            .unwrap();

        assert_eq!(matched.primary.provider.id, "anthropic");
        assert_eq!(matched.primary.target_model, "claude-3-7-sonnet");
        assert_eq!(matched.primary.api_key.as_deref(), Some("sk-anthropic"));
        assert_eq!(matched.fallbacks.len(), 1);
        assert_eq!(matched.fallbacks[0].provider.id, "antigravity");
        assert_eq!(matched.fallbacks[0].target_model, "claude-3-7-sonnet");

        let key1 = engine.select_api_key(&providers[0], Some("user_session_abc")).unwrap();
        let key2 = engine.select_api_key(&providers[0], Some("user_session_abc")).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn nested_prefix_models_keep_the_full_sub_model_name() {
        let providers = vec![provider("openrouter", &["anthropic/claude-3.7-sonnet"], 10)];

        let plan = plan_route("openrouter/anthropic/claude-3.7-sonnet", &providers).unwrap();

        assert_eq!(plan.primary.provider_id, "openrouter");
        assert_eq!(plan.primary.model, "anthropic/claude-3.7-sonnet");
    }

    #[test]
    fn prefix_routing_matches_provider_name_case_insensitively() {
        let mut p = provider("prov_uuid_12345", &["deepseek-v4-pro"], 10);
        p.name = "DeepSeek".to_string();
        let providers = vec![p];

        let plan = plan_route("deepseek/deepseek-v4-pro", &providers).unwrap();
        assert_eq!(plan.primary.provider_id, "prov_uuid_12345");
        assert_eq!(plan.primary.model, "deepseek-v4-pro");
    }

    #[test]
    fn no_enabled_providers_is_reported_before_any_lookup() {
        let mut disabled = provider("anthropic", &["claude-3-7-sonnet"], 10);
        disabled.enabled = false;

        let error = plan_route("claude-3-7-sonnet", &[disabled]).unwrap_err();

        assert_eq!(error, "No active providers enabled in Pivot");
    }

    #[test]
    fn normalized_model_matching_resolves_dates_and_modifiers() {
        let providers = vec![
            provider("anthropic", &["claude-3-7-sonnet"], 20),
            provider("google", &["gemini-3.8-flash-high"], 15),
            provider("openai", &["gpt-4o-2024-08-06"], 10),
        ];

        let plan1 = plan_route("claude-3-7-sonnet-20250219", &providers).unwrap();
        assert_eq!(plan1.primary.provider_id, "anthropic");
        assert_eq!(plan1.primary.model, "claude-3-7-sonnet");

        let plan2 = plan_route("gemini-3.8-flash-high[1M]", &providers).unwrap();
        assert_eq!(plan2.primary.provider_id, "google");
        assert_eq!(plan2.primary.model, "gemini-3.8-flash-high");

        let plan3 = plan_route("gpt-4o", &providers).unwrap();
        assert_eq!(plan3.primary.provider_id, "openai");
        assert_eq!(plan3.primary.model, "gpt-4o-2024-08-06");
    }

    #[test]
    fn explain_route_traces_every_layer() {
        let providers = vec![
            provider("anthropic", &["claude-3-7-sonnet"], 20),
            as_default(with_default_model(
                provider("catch-all", &["some-model"], 10),
                "gemini-3.8-flash-high",
            )),
        ];

        let explained = explain_route("anthropic/claude-3-7-sonnet", &providers);
        assert_eq!(explained.steps[0].layer, "prefix");
        assert!(explained.steps[0].matched);
        assert_eq!(explained.plan.unwrap().primary.model, "claude-3-7-sonnet");

        let explained = explain_route("claude-3-7-sonnet", &providers);
        assert!(explained
            .steps
            .iter()
            .any(|step| step.layer == "exact" && step.matched));

        let dated = vec![provider("anthropic", &["claude-3-7-sonnet-20250219"], 20)];
        let explained = explain_route("claude-3-7-sonnet", &dated);
        assert!(explained
            .steps
            .iter()
            .any(|step| step.layer == "normalized" && step.matched));

        let explained = explain_route("brand-new-model", &providers);
        assert!(explained
            .steps
            .iter()
            .any(|step| step.layer == "default" && step.matched));
        assert_eq!(explained.plan.unwrap().primary.provider_id, "catch-all");
    }

    #[test]
    fn explain_route_reports_failures_with_the_step_that_failed() {
        let providers = vec![provider("deepseek", &["deepseek-v4-pro"], 10)];
        let explained = explain_route("brand-new-model", &providers);

        assert!(explained.plan.is_none());
        assert!(explained
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("default provider"));
        assert!(explained
            .steps
            .iter()
            .any(|step| step.layer == "default" && !step.matched));
    }

}
