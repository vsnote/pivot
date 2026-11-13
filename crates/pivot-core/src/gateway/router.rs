use crate::db::Provider;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

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

pub struct RoutingEngine;

impl Default for RoutingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RoutingEngine {
    pub fn new() -> Self {
        Self
    }

    /// Select API key using stateless deterministic session affinity or round-robin
    pub fn select_api_key(&self, provider: &Provider, session_id: Option<&str>) -> Option<String> {
        if provider.api_keys.is_empty() {
            return None;
        }

        let key_count = provider.api_keys.len();
        if key_count == 1 {
            return Some(provider.api_keys[0].clone());
        }

        if let Some(sess) = session_id {
            // 无状态确定性 FNV-1a 哈希：避免 session_map 随请求无限增长泄漏内存
            let mut hash: u64 = 0xcbf29ce484222325;
            for b in provider.id.bytes().chain(*b":").chain(sess.bytes()) {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
            let idx = (hash as usize) % key_count;
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

/// 统一计算服务商名称路由前缀（全端唯一标准，确保名称含 `/` 或多空格时前后端一致可路由）
pub fn provider_route_prefix(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = false;
    for ch in name.trim().chars() {
        if ch == '/' || ch == '\\' || ch.is_whitespace() || ch == '-' {
            if !out.is_empty() && !prev_dash {
                out.push('-');
                prev_dash = true;
            }
        } else {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
            prev_dash = false;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
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

    // 1. Prefix routing (e.g. "openai/gpt-4o", "opencode-go-plus/kimi-k2.5"):
    // 只有当匹配服务商确实声明（或同模型归一化匹配）了 sub_model 时才命中，避免截胡 OpenRouter 等自带 "/" 的模型名
    if let Some((prefix, sub_model)) = incoming_model.split_once('/') {
        let prefix_lower = prefix.to_lowercase();
        let prefix_slug = provider_route_prefix(prefix);
        let matched_providers: Vec<&Provider> = active_providers
            .iter()
            .copied()
            .filter(|p| {
                p.id.eq_ignore_ascii_case(prefix)
                    || p.name.to_lowercase() == prefix_lower
                    || (!prefix_slug.is_empty() && provider_route_prefix(&p.name) == prefix_slug)
            })
            .collect();

        if !matched_providers.is_empty() {
            for provider in &matched_providers {
                if let Some(resolved_sub) = find_declared_or_normalized_model(provider, sub_model) {
                    note(trace, "prefix", true, || {
                        format!("前缀 {} 命中服务商 {} (目标 {})", prefix, provider.name, resolved_sub)
                    });
                    return Ok(RoutePlan {
                        primary: planned(provider, &resolved_sub),
                        fallbacks: Vec::new(),
                    });
                }
            }

            // 匹配了服务商前缀，但其未声明 sub_model：若其他服务商声明了完整的 incoming_model（如 openrouter 声明 "anthropic/claude-3-opus"），放行到 exact 层
            let any_declared_full = active_providers
                .iter()
                .any(|p| find_declared_or_normalized_model(p, incoming_model).is_some());
            if !any_declared_full {
                let first_name = &matched_providers[0].name;
                note(trace, "prefix", false, || {
                    format!("服务商 {} 未声明模型 {}", first_name, sub_model)
                });
                return Err(format!(
                    "Model '{}' is not declared by provider '{}'",
                    sub_model, first_name
                ));
            }
            note(trace, "prefix", false, || {
                format!("前缀 {} 服务商未声明 {}，继续尝试完整模型名匹配", prefix, sub_model)
            });
        } else {
            note(trace, "prefix", false, || {
                format!("前缀 {} 不对应任何已启用的服务商", prefix)
            });
        }
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
            fallbacks: Vec::new(),
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
                fallbacks: Vec::new(),
            });
        }
    }
    note(trace, "normalized", false, || {
        "没有可归一化匹配的模型".to_string()
    });

    // 3. Unknown model name: 严格遵守铁律，绝不做跨模型家族兜底或静默改写
    note(trace, "route", false, || {
        format!("未找到支持模型 '{}' 的可用服务商", incoming_model)
    });
    Err(format!(
        "Model '{}' is not declared by any enabled provider",
        incoming_model
    ))
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

    // 3. Strip date suffix -\d{8} or -\d{4}-\d{2}-\d{2} (仅做同模型去修饰归一化，禁止硬编码跨模型映射)
    static RE_DATE_8: OnceLock<Regex> = OnceLock::new();
    static RE_DATE_ISO: OnceLock<Regex> = OnceLock::new();
    let re8 = RE_DATE_8.get_or_init(|| Regex::new(r"-\d{8}$").unwrap());
    let re_iso = RE_DATE_ISO.get_or_init(|| Regex::new(r"-\d{4}-\d{2}-\d{2}$").unwrap());

    let without_date = if let Some(m) = re8.find(without_latest) {
        &without_latest[..m.start()]
    } else if let Some(m) = re_iso.find(without_latest) {
        &without_latest[..m.start()]
    } else {
        without_latest
    };

    if without_date != trimmed && !without_date.is_empty() {
        candidates.push(without_date.to_string());
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



    #[test]
    fn prefix_routing_pins_the_named_provider_and_keeps_its_model() {
        let providers = vec![
            provider("anthropic", &["claude-3-7-sonnet"], 10),
            provider("aggregator", &["claude-3-7-sonnet"], 5),
        ];

        let plan = plan_route("anthropic/claude-3-7-sonnet", &providers).unwrap();

        assert_eq!(plan.primary.provider_id, "anthropic");
        assert_eq!(plan.primary.model, "claude-3-7-sonnet");
        assert!(plan.fallbacks.is_empty());
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
        assert!(plan.fallbacks.is_empty());
    }

    #[test]
    fn unknown_model_returns_clear_error() {
        let providers = vec![
            provider("deepseek", &["deepseek-v4-pro"], 20),
            provider("antigravity", &["gemini-3.8-flash-high"], 10),
        ];

        let err = plan_route("claude-3-5-haiku-20241022", &providers).unwrap_err();
        assert!(err.contains("claude-3-5-haiku-20241022"));
        assert!(err.contains("not declared"));
    }

    #[test]
    fn no_cross_provider_fallbacks_are_generated() {
        let providers = vec![
            provider("primary", &["claude-3-7-sonnet"], 30),
            provider("renaming", &["claude-4"], 20),
            provider("same-name", &["claude-3-7-sonnet"], 10),
            provider("unrelated", &["qwen3-max"], 5),
        ];

        let plan = plan_route("claude-3-7-sonnet", &providers).unwrap();

        assert_eq!(plan.primary.provider_id, "primary");
        assert!(plan.fallbacks.is_empty());
    }

    #[test]
    fn disabled_provider_does_not_resolve_models() {
        let mut disabled = provider("offline", &["claude-3-7-sonnet"], 10);
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
        assert!(matched.fallbacks.is_empty());

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
            provider("google", &["gemini-3.8-flash-high"], 10),
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
        assert!(explained.plan.is_none());
        assert!(explained
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("not declared"));
        assert!(explained
            .steps
            .iter()
            .any(|step| step.layer == "route" && !step.matched));
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
            .contains("not declared"));
        assert!(explained
            .steps
            .iter()
            .any(|step| step.layer == "route" && !step.matched));
    }

    #[test]
    fn provider_route_prefix_sanitizes_slashes_and_extra_whitespace() {
        assert_eq!(provider_route_prefix("OpenCode Go / Plus"), "opencode-go-plus");
        assert_eq!(provider_route_prefix("OpenAI / DeepSeek"), "openai-deepseek");
        assert_eq!(provider_route_prefix("DeepSeek"), "deepseek");
    }

    #[test]
    fn prefix_routing_does_not_hijack_openrouter_slash_models() {
        let providers = vec![
            provider("anthropic", &["claude-sonnet-4"], 20),
            provider("openrouter", &["anthropic/claude-3-opus"], 10),
        ];

        let plan = plan_route("anthropic/claude-3-opus", &providers).unwrap();
        assert_eq!(plan.primary.provider_id, "openrouter");
        assert_eq!(plan.primary.model, "anthropic/claude-3-opus");
    }

    #[test]
    fn deepseek_unknown_model_is_never_downgraded() {
        let providers = vec![provider("deepseek", &["deepseek-chat"], 10)];
        let err = plan_route("deepseek-reasoner", &providers).unwrap_err();
        assert!(err.contains("deepseek-reasoner"));
    }

    #[test]
    fn opencode_go_plus_prefix_routes_cleanly() {
        let mut p = provider("prov_ocg", &["kimi-k2.5"], 10);
        p.name = "OpenCode Go / Plus".to_string();
        let plan = plan_route("opencode-go-plus/kimi-k2.5", &[p]).unwrap();
        assert_eq!(plan.primary.provider_id, "prov_ocg");
        assert_eq!(plan.primary.model, "kimi-k2.5");
    }

}
