use crate::db::{Provider, RouteRule};
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
        routes: &[RouteRule],
    ) -> Result<RouteMatch, String> {
        let plan = plan_route(incoming_model, providers, routes)?;
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
/// 2. enabled routing rules — advanced overrides for specific patterns
/// 3. exact model name declared by an enabled provider
/// 4. the provider marked as default, asked for its own `default_model`
///
/// A model name nobody declares is an error unless a default provider exists;
/// silently landing on the highest priority provider is what used to hide
/// misconfiguration.
pub fn plan_route(
    incoming_model: &str,
    providers: &[Provider],
    routes: &[RouteRule],
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
            return Ok(RoutePlan {
                primary: planned(provider, sub_model),
                fallbacks: automatic_fallbacks(&active_providers, &provider.id, sub_model),
            });
        }
    }

    // 2. Routing rules stay available as explicit overrides.
    let mut sorted_rules = routes.to_vec();
    sorted_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

    for rule in &sorted_rules {
        if !rule.enabled || !pattern_matches(&rule.pattern, incoming_model) {
            continue;
        }
        let Some(primary_provider) = provider_map.get(rule.target_provider_id.as_str()) else {
            continue;
        };

        let desired_model = rule
            .target_model
            .clone()
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| incoming_model.to_string());

        // A rule pins the provider, but every candidate is still asked for the
        // model name its own configuration knows, and providers that know none
        // are dropped instead of being handed a name they would reject.
        let fallbacks: Vec<PlannedCandidate> = rule
            .fallback_provider_ids
            .iter()
            .filter_map(|id| provider_map.get(id.as_str()).copied())
            .filter(|p| p.id != primary_provider.id)
            .filter_map(|p| candidate_model(p, &desired_model).map(|model| planned(p, &model)))
            .collect();

        return Ok(RoutePlan {
            primary: planned(primary_provider, &desired_model),
            fallbacks,
        });
    }

    // 3. Exact model name declared by an enabled provider.
    if let Some(provider) = active_providers
        .iter()
        .find(|p| lists_model(p, incoming_model))
    {
        return Ok(RoutePlan {
            primary: planned(provider, incoming_model),
            fallbacks: automatic_fallbacks(&active_providers, &provider.id, incoming_model),
        });
    }

    // 4. Unknown model name: hand it to the default provider.
    let default_provider = active_providers
        .iter()
        .find(|p| p.is_default)
        .ok_or_else(|| {
            format!(
                "Model '{}' is not declared by any enabled provider and no default provider is configured; mark one provider as default in Pivot",
                incoming_model
            )
        })?;
    let model = candidate_model(default_provider, incoming_model)
        .unwrap_or_else(|| incoming_model.to_string());

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

fn declared_default_model(provider: &Provider) -> Option<String> {
    provider
        .default_model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string)
}

/// Model name to ask `provider` for when `model` is requested: the name itself
/// when the provider declares it, otherwise the provider's own default model.
/// `None` means this provider cannot serve the request at all.
fn candidate_model(provider: &Provider, model: &str) -> Option<String> {
    if lists_model(provider, model) {
        Some(model.to_string())
    } else {
        declared_default_model(provider)
    }
}

/// Automatic fallback queue for `model`: providers that declare the very same
/// model name first (highest priority first), then providers that rewrite the
/// request to their own default model, and nothing else. Passing one model name
/// to every candidate is what made cross-provider fallbacks fail with a 400.
fn automatic_fallbacks(
    active_providers: &[&Provider],
    excluded_id: &str,
    model: &str,
) -> Vec<PlannedCandidate> {
    let mut same_name = Vec::new();
    let mut rewritten = Vec::new();

    for provider in active_providers.iter().filter(|p| p.id != excluded_id) {
        if lists_model(provider, model) {
            same_name.push(planned(provider, model));
        } else if let Some(default_model) = declared_default_model(provider) {
            rewritten.push(planned(provider, &default_model));
        }
    }

    same_name.extend(rewritten);
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

    fn rule(pattern: &str, target: &str, fallbacks: &[&str]) -> RouteRule {
        RouteRule {
            id: format!("rule-{}", pattern),
            pattern: pattern.to_string(),
            target_provider_id: target.to_string(),
            target_model: None,
            fallback_provider_ids: fallbacks.iter().map(|id| id.to_string()).collect(),
            enabled: true,
            priority: 100,
            description: None,
        }
    }

    #[test]
    fn test_pattern_matching() {
        assert!(pattern_matches("claude-*", "claude-3-7-sonnet-20250219"));
        assert!(pattern_matches("gpt-*", "gpt-4o"));
        assert!(pattern_matches("o[13]*", "o3-mini"));
        assert!(pattern_matches("o[13]*", "o1-preview"));
        assert!(pattern_matches("*", "any-model-name"));
        assert!(pattern_matches("deepseek/*", "deepseek/deepseek-r1"));
        assert!(!pattern_matches("claude-*", "gpt-4o"));
        assert!(!pattern_matches("gpt-*", "claude-3-5-sonnet"));
    }

    #[test]
    fn prefix_routing_pins_the_named_provider_and_keeps_its_model() {
        let providers = vec![
            provider("anthropic", &["claude-3-7-sonnet"], 10),
            with_default_model(provider("aggregator", &["gemini-3.8-flash-high"], 5), "gemini-3.8-flash-high"),
        ];

        let plan = plan_route("anthropic/claude-3-7-sonnet", &providers, &[]).unwrap();

        assert_eq!(plan.primary.provider_id, "anthropic");
        assert_eq!(plan.primary.model, "claude-3-7-sonnet");
        // The aggregator does not declare that model, so it is only reached
        // through its own default model.
        assert_eq!(plan.fallbacks.len(), 1);
        assert_eq!(plan.fallbacks[0].provider_id, "aggregator");
        assert_eq!(plan.fallbacks[0].model, "gemini-3.8-flash-high");
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

        let plan = plan_route("gemini-3.8-flash-high", &providers, &[]).unwrap();

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

        let plan = plan_route("claude-3-5-haiku-20241022", &providers, &[]).unwrap();

        assert_eq!(plan.primary.provider_id, "antigravity");
        assert_eq!(plan.primary.model, "gemini-3.8-flash-high");
        assert!(plan.fallbacks.is_empty(), "deepseek does not declare that model");
    }

    #[test]
    fn fallbacks_prefer_the_same_model_name_then_providers_with_their_own_default() {
        let providers = vec![
            with_default_model(as_default(provider("primary", &["claude-3-7-sonnet"], 30)), "gemini-3.8-flash-high"),
            with_default_model(provider("renaming", &["claude-4"], 20), "deepseek-v4-pro"),
            provider("same-name", &["claude-3-7-sonnet"], 10),
            provider("unrelated", &["qwen3-max"], 5),
        ];

        let plan = plan_route("claude-3-7-sonnet", &providers, &[]).unwrap();

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
                ("renaming", "deepseek-v4-pro"),
            ]
        );
    }

    #[test]
    fn bare_default_provider_passes_unknown_models_through() {
        let providers = vec![as_default(provider("catch-all", &["some-model"], 10))];

        let plan = plan_route("brand-new-model", &providers, &[]).unwrap();

        assert_eq!(plan.primary.provider_id, "catch-all");
        assert_eq!(plan.primary.model, "brand-new-model");
        assert!(plan.fallbacks.is_empty());
    }

    #[test]
    fn unknown_model_without_default_provider_is_a_clear_error() {
        let providers = vec![provider("deepseek", &["deepseek-v4-pro"], 10)];

        let error = plan_route("claude-3-7-sonnet", &providers, &[]).unwrap_err();

        assert!(error.contains("claude-3-7-sonnet"), "{}", error);
        assert!(error.contains("default provider"), "{}", error);
    }

    #[test]
    fn disabled_default_provider_does_not_resolve_unknown_models() {
        let mut disabled = as_default(with_default_model(provider("offline", &[], 10), "gemini-3.8-flash-high"));
        disabled.enabled = false;
        let providers = vec![provider("deepseek", &["deepseek-v4-pro"], 5), disabled];

        assert!(plan_route("claude-3-7-sonnet", &providers, &[]).is_err());
    }

    #[test]
    fn routing_rules_still_override_automatic_resolution() {
        let providers = vec![
            provider("deepseek", &["deepseek-v4-pro"], 20),
            with_default_model(as_default(provider("antigravity", &["gemini-3.8-flash-high"], 10)), "gemini-3.8-flash-high"),
        ];
        let mut override_rule = rule("deepseek-v4-pro", "deepseek", &["antigravity"]);
        override_rule.target_model = Some("deepseek-v4-pro".to_string());

        let plan = plan_route("deepseek-v4-pro", &providers, &[override_rule]).unwrap();

        assert_eq!(plan.primary.provider_id, "deepseek");
        assert_eq!(plan.primary.model, "deepseek-v4-pro");
        assert_eq!(plan.fallbacks.len(), 1);
        assert_eq!(plan.fallbacks[0].provider_id, "antigravity");
        assert_eq!(plan.fallbacks[0].model, "gemini-3.8-flash-high");
    }

    #[test]
    fn rule_fallbacks_that_cannot_serve_the_target_model_are_skipped() {
        let providers = vec![
            provider("anthropic", &["claude-3-7-sonnet"], 20),
            provider("openrouter", &["anthropic/claude-3.7-sonnet"], 10),
        ];
        let mut mapping = rule("claude-*", "anthropic", &["openrouter"]);
        mapping.target_model = Some("gpt-4o".to_string());

        let plan = plan_route("claude-3-7-sonnet", &providers, &[mapping]).unwrap();

        assert_eq!(plan.primary.model, "gpt-4o");
        // Neither provider declares the rule's rewritten model, but openrouter
        // has no default model either, so it is not handed a name it would 400 on.
        assert!(plan.fallbacks.is_empty());
    }

    #[test]
    fn resolve_route_attaches_provider_configuration_and_keeps_session_affinity() {
        let engine = RoutingEngine::new();
        let providers = vec![
            provider("anthropic", &["claude-3-7-sonnet"], 20),
            with_default_model(provider("antigravity", &["gemini-3.8-flash-high"], 10), "gemini-3.8-flash-high"),
        ];

        let matched = engine
            .resolve_route("claude-3-7-sonnet", Some("session-1"), &providers, &[])
            .unwrap();

        assert_eq!(matched.primary.provider.id, "anthropic");
        assert_eq!(matched.primary.target_model, "claude-3-7-sonnet");
        assert_eq!(matched.primary.api_key.as_deref(), Some("sk-anthropic"));
        assert_eq!(matched.fallbacks.len(), 1);
        assert_eq!(matched.fallbacks[0].provider.id, "antigravity");
        // Cross-provider fallbacks carry their own model name now.
        assert_eq!(matched.fallbacks[0].target_model, "gemini-3.8-flash-high");

        let key1 = engine.select_api_key(&providers[0], Some("user_session_abc")).unwrap();
        let key2 = engine.select_api_key(&providers[0], Some("user_session_abc")).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn nested_prefix_models_keep_the_full_sub_model_name() {
        let providers = vec![provider("openrouter", &["anthropic/claude-3.7-sonnet"], 10)];

        let plan = plan_route("openrouter/anthropic/claude-3.7-sonnet", &providers, &[]).unwrap();

        assert_eq!(plan.primary.provider_id, "openrouter");
        assert_eq!(plan.primary.model, "anthropic/claude-3.7-sonnet");
    }

    #[test]
    fn prefix_routing_matches_provider_name_case_insensitively() {
        let mut p = provider("prov_uuid_12345", &["deepseek-v4-pro"], 10);
        p.name = "DeepSeek".to_string();
        let providers = vec![p];

        let plan = plan_route("deepseek/deepseek-v4-pro", &providers, &[]).unwrap();
        assert_eq!(plan.primary.provider_id, "prov_uuid_12345");
        assert_eq!(plan.primary.model, "deepseek-v4-pro");
    }

    #[test]
    fn no_enabled_providers_is_reported_before_any_lookup() {
        let mut disabled = provider("anthropic", &["claude-3-7-sonnet"], 10);
        disabled.enabled = false;

        let error = plan_route("claude-3-7-sonnet", &[disabled], &[]).unwrap_err();

        assert_eq!(error, "No active providers enabled in Pivot");
    }
}
