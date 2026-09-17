use crate::db::{Provider, RouteRule};
use regex::Regex;
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
        let active_providers: Vec<&Provider> = providers.iter().filter(|p| p.enabled).collect();
        if active_providers.is_empty() {
            return Err("No active providers enabled in OmniSwitch".to_string());
        }

        let provider_map: HashMap<&str, &Provider> = active_providers
            .iter()
            .map(|p| (p.id.as_str(), *p))
            .collect();

        // 1. Check for prefix routing (e.g. "openai/gpt-4o", "anthropic/claude-3-7-sonnet")
        if let Some((prefix, sub_model)) = incoming_model.split_once('/') {
            let clean_prefix = prefix.to_lowercase();
            if let Some(provider) = provider_map.get(clean_prefix.as_str()) {
                let primary = RouteCandidate {
                    provider: (*provider).clone(),
                    target_model: sub_model.to_string(),
                    api_key: self.select_api_key(provider, session_id),
                };

                // Fallback to openrouter or other active providers
                let fallbacks: Vec<RouteCandidate> = active_providers
                    .iter()
                    .filter(|p| p.id != provider.id)
                    .map(|p| RouteCandidate {
                        provider: (*p).clone(),
                        target_model: sub_model.to_string(),
                        api_key: self.select_api_key(p, session_id),
                    })
                    .collect();

                return Ok(RouteMatch { primary, fallbacks });
            }
        }

        // 2. Evaluate Dynamic Routing Rules
        let mut sorted_rules = routes.to_vec();
        sorted_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        for rule in &sorted_rules {
            if !rule.enabled {
                continue;
            }

            if pattern_matches(&rule.pattern, incoming_model) {
                if let Some(primary_provider) = provider_map.get(rule.target_provider_id.as_str()) {
                    let target_model = rule
                        .target_model
                        .clone()
                        .filter(|m| !m.is_empty())
                        .unwrap_or_else(|| incoming_model.to_string());

                    let primary = RouteCandidate {
                        provider: (*primary_provider).clone(),
                        target_model: target_model.clone(),
                        api_key: self.select_api_key(primary_provider, session_id),
                    };

                    let mut fallbacks = Vec::new();
                    for fb_id in &rule.fallback_provider_ids {
                        if let Some(fb_prov) = provider_map.get(fb_id.as_str()) {
                            if fb_prov.id != primary_provider.id {
                                fallbacks.push(RouteCandidate {
                                    provider: (*fb_prov).clone(),
                                    target_model: target_model.clone(),
                                    api_key: self.select_api_key(fb_prov, session_id),
                                });
                            }
                        }
                    }

                    return Ok(RouteMatch { primary, fallbacks });
                }
            }
        }

        // 3. Fallback: match any provider that contains the model in its models list
        for provider in &active_providers {
            if provider
                .models
                .iter()
                .any(|m| m.eq_ignore_ascii_case(incoming_model))
            {
                let primary = RouteCandidate {
                    provider: (*provider).clone(),
                    target_model: incoming_model.to_string(),
                    api_key: self.select_api_key(provider, session_id),
                };

                let fallbacks: Vec<RouteCandidate> = active_providers
                    .iter()
                    .filter(|p| p.id != provider.id)
                    .map(|p| RouteCandidate {
                        provider: (*p).clone(),
                        target_model: incoming_model.to_string(),
                        api_key: self.select_api_key(p, session_id),
                    })
                    .collect();

                return Ok(RouteMatch { primary, fallbacks });
            }
        }

        // 4. Default to the highest priority active provider
        let default_provider = active_providers[0];
        let primary = RouteCandidate {
            provider: (*default_provider).clone(),
            target_model: incoming_model.to_string(),
            api_key: self.select_api_key(default_provider, session_id),
        };

        let fallbacks: Vec<RouteCandidate> = active_providers
            .iter()
            .skip(1)
            .map(|p| RouteCandidate {
                provider: (*p).clone(),
                target_model: incoming_model.to_string(),
                api_key: self.select_api_key(p, session_id),
            })
            .collect();

        Ok(RouteMatch { primary, fallbacks })
    }
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
    fn test_route_resolution_and_fallback() {
        let engine = RoutingEngine::new();

        let p1 = Provider {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            protocol: "anthropic".to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            preset_id: None,
            api_keys: vec!["sk-ant-1".to_string(), "sk-ant-2".to_string()],
            models: vec!["claude-3-7-sonnet".to_string()],
            enabled: true,
            priority: 10,
            weight: 1,
            timeout_secs: 60,
            latency_ms: None,
            created_at: None,
            updated_at: None,
        };

        let p2 = Provider {
            id: "openrouter".to_string(),
            name: "OpenRouter".to_string(),
            protocol: "openai".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            preset_id: None,
            api_keys: vec!["sk-or-1".to_string()],
            models: vec!["anthropic/claude-3.7-sonnet".to_string()],
            enabled: true,
            priority: 5,
            weight: 1,
            timeout_secs: 60,
            latency_ms: None,
            created_at: None,
            updated_at: None,
        };

        let routes = vec![RouteRule {
            id: "r1".to_string(),
            pattern: "claude-*".to_string(),
            target_provider_id: "anthropic".to_string(),
            target_model: None,
            fallback_provider_ids: vec!["openrouter".to_string()],
            enabled: true,
            priority: 100,
            description: None,
        }];

        let providers = vec![p1.clone(), p2.clone()];

        // Test normal route match
        let rm = engine
            .resolve_route("claude-3-7-sonnet", Some("sess_123"), &providers, &routes)
            .unwrap();
        assert_eq!(rm.primary.provider.id, "anthropic");
        assert_eq!(rm.primary.target_model, "claude-3-7-sonnet");
        assert_eq!(rm.fallbacks.len(), 1);
        assert_eq!(rm.fallbacks[0].provider.id, "openrouter");

        // Test Session Affinity gives deterministic key
        let key1 = engine
            .select_api_key(&p1, Some("user_session_abc"))
            .unwrap();
        let key2 = engine
            .select_api_key(&p1, Some("user_session_abc"))
            .unwrap();
        assert_eq!(key1, key2);

        // Test Prefix routing (e.g. "openrouter/anthropic/claude-3.7-sonnet")
        let rm_prefix = engine
            .resolve_route(
                "openrouter/anthropic/claude-3.7-sonnet",
                None,
                &providers,
                &routes,
            )
            .unwrap();
        assert_eq!(rm_prefix.primary.provider.id, "openrouter");
        assert_eq!(
            rm_prefix.primary.target_model,
            "anthropic/claude-3.7-sonnet"
        );
    }
}
