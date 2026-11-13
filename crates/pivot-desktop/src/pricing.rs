#![allow(dead_code)]

use crate::i18n::Locale;
use chrono::{Local, TimeZone, Utc};
use pivot_core::db::RequestLog;
use pivot_core::service;
use pivot_core::state::AppState;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default)]
pub struct TodayUsageSummary {
    pub requests: usize,
    pub tokens: u64,
    pub cost_usd: f64,
}

pub fn compute_today_summary_from_db(
    state: &AppState,
    rates: &HashMap<String, ModelRate>,
) -> TodayUsageSummary {
    let now_utc = Utc::now();
    let now_sec = now_utc.timestamp();
    let local_now = now_utc.with_timezone(&Local);
    let today_start_epoch = local_now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|naive| Local.from_local_datetime(&naive).single())
        .map(|dt| dt.with_timezone(&Utc).timestamp())
        .unwrap_or_else(|| now_sec - now_sec.rem_euclid(86_400));

    let Ok(raw) =
        service::get_dashboard_stats(state, Some(today_start_epoch), None, today_start_epoch)
    else {
        return TodayUsageSummary::default();
    };

    let mut tokens = 0u64;
    let mut cost_usd = 0.0f64;
    for s in &raw.route_model_slices {
        tokens += s.prompt_tokens + s.completion_tokens;
        cost_usd += estimate_cost(rates, &s.model, s.prompt_tokens, s.completion_tokens);
    }

    TodayUsageSummary {
        requests: raw.total_reqs,
        tokens,
        cost_usd,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModelRate {
    pub prompt: f64,
    pub completion: f64,
}

pub const DEFAULT_RATE: ModelRate = ModelRate {
    prompt: 2.0,
    completion: 8.0,
};

pub fn build_rate_map() -> HashMap<String, ModelRate> {
    let val = service::get_model_rates();
    let mut map = HashMap::new();
    if let Some(obj) = val.as_object() {
        for (k, v) in obj {
            if let Some(arr) = v.as_array() {
                if arr.len() >= 2 {
                    let p = arr[0].as_f64().unwrap_or(2.0);
                    let c = arr[1].as_f64().unwrap_or(8.0);
                    map.insert(
                        k.to_lowercase(),
                        ModelRate {
                            prompt: p,
                            completion: c,
                        },
                    );
                }
            }
        }
    }
    map
}

fn strip_date_or_latest_suffix(key: &str) -> String {
    let without_latest = key.strip_suffix("-latest").unwrap_or(key);
    // 去 `-YYYY-MM-DD` (11 chars) 或 `-YYYYMMDD` (9 chars)
    let bytes = without_latest.as_bytes();
    if bytes.len() > 11 && bytes[bytes.len() - 11] == b'-' {
        let tail = &without_latest[bytes.len() - 10..];
        let parts: Vec<&str> = tail.split('-').collect();
        if parts.len() == 3
            && parts[0].len() == 4
            && parts[1].len() == 2
            && parts[2].len() == 2
            && parts.iter().all(|s| s.chars().all(|c| c.is_ascii_digit()))
        {
            return without_latest[..bytes.len() - 11].to_string();
        }
    }
    if bytes.len() > 9 && bytes[bytes.len() - 9] == b'-' {
        let tail = &without_latest[bytes.len() - 8..];
        if tail.chars().all(|c| c.is_ascii_digit()) {
            return without_latest[..bytes.len() - 9].to_string();
        }
    }
    without_latest.to_string()
}

fn strip_bracket_suffix(key: &str) -> &str {
    if let Some(idx) = key.find('[') {
        if key.ends_with(']') && idx > 0 {
            return &key[..idx];
        }
    }
    key
}

fn strip_reasoning_tier_suffix(key: &str) -> &str {
    for suffix in [
        "-thinking",
        "-minimal",
        "-xhigh",
        "-medium",
        "-high",
        "-low",
        "-max",
    ] {
        if let Some(stripped) = key.strip_suffix(suffix) {
            if !stripped.is_empty() {
                return stripped;
            }
        }
    }
    key
}

pub fn candidate_keys(model_name: &str) -> Vec<String> {
    fn push_unique(keys: &mut Vec<String>, candidate: String) {
        if !candidate.is_empty() && !keys.contains(&candidate) {
            keys.push(candidate);
        }
    }

    let base = model_name.trim().to_lowercase();
    let mut keys = Vec::new();

    push_unique(&mut keys, base.clone());
    let without_bracket = strip_bracket_suffix(&base).to_string();
    push_unique(&mut keys, without_bracket.clone());

    if let Some(slash) = without_bracket.rfind('/') {
        push_unique(&mut keys, without_bracket[slash + 1..].to_string());
    }

    let snapshot = keys.clone();
    for k in snapshot {
        if let Some((before_colon, _)) = k.split_once(':') {
            push_unique(&mut keys, before_colon.to_string());
        }
    }

    let snapshot = keys.clone();
    for k in snapshot {
        let no_tier = strip_reasoning_tier_suffix(&k).to_string();
        push_unique(&mut keys, no_tier);
    }

    let snapshot = keys.clone();
    for k in snapshot {
        let slim = strip_date_or_latest_suffix(&k);
        push_unique(&mut keys, slim);
    }

    keys
}

pub fn lookup_rate(rates: &HashMap<String, ModelRate>, model_name: &str) -> ModelRate {
    let trimmed = model_name.trim();
    if trimmed.is_empty() {
        return DEFAULT_RATE;
    }
    let lower = trimmed.to_lowercase();
    if strip_bracket_suffix(&lower).ends_with(":free") {
        return ModelRate {
            prompt: 0.0,
            completion: 0.0,
        };
    }
    for key in candidate_keys(model_name) {
        if let Some(rate) = rates.get(&key) {
            return *rate;
        }
    }
    DEFAULT_RATE
}

pub fn estimate_cost(
    rates: &HashMap<String, ModelRate>,
    model_name: &str,
    prompt_tokens: u64,
    completion_tokens: u64,
) -> f64 {
    if prompt_tokens == 0 && completion_tokens == 0 {
        return 0.0;
    }
    let rate = lookup_rate(rates, model_name);
    (prompt_tokens as f64 / 1_000_000.0) * rate.prompt
        + (completion_tokens as f64 / 1_000_000.0) * rate.completion
}

pub fn effective_tokens(log: &RequestLog) -> (u64, u64, u64) {
    let p = log.prompt_tokens.max(0) as u64;
    let c = log.completion_tokens.max(0) as u64;
    (p, c, p + c)
}

pub fn format_currency(amount: f64) -> String {
    if amount <= 0.0 || amount.is_nan() {
        return "$0.00".to_string();
    }
    if amount < 0.0001 {
        return "<$0.0001".to_string();
    }
    if amount < 0.01 {
        return format!("${:.4}", amount);
    }
    if amount < 1.0 {
        return format!("${:.3}", amount);
    }
    format!("${:.2}", amount)
}

pub fn format_tokens(tokens: u64) -> String {
    if tokens == 0 {
        return "0".to_string();
    }
    if tokens >= 1_000_000 {
        return format!("{:.2} M", tokens as f64 / 1_000_000.0);
    }
    if tokens >= 1_000 {
        return format!("{:.1} K", tokens as f64 / 1_000.0);
    }
    tokens.to_string()
}

pub fn format_tokens_compact(tokens: u64) -> String {
    if tokens == 0 {
        return "0".to_string();
    }
    if tokens >= 100_000_000 {
        return format!("{:.0}M", tokens as f64 / 1_000_000.0);
    }
    if tokens >= 10_000_000 {
        return format!("{:.1}M", tokens as f64 / 1_000_000.0);
    }
    if tokens >= 1_000_000 {
        return format!("{:.2}M", tokens as f64 / 1_000_000.0);
    }
    if tokens >= 100_000 {
        return format!("{:.0}K", tokens as f64 / 1_000.0);
    }
    if tokens >= 1_000 {
        return format!("{:.1}K", tokens as f64 / 1_000.0);
    }
    tokens.to_string()
}

pub fn format_latency(ms: f64) -> String {
    if ms <= 0.0 || ms.is_nan() {
        return "0ms".to_string();
    }
    if ms >= 1000.0 {
        return format!("{:.2}s", ms / 1000.0);
    }
    format!("{}ms", ms.round() as i64)
}

pub fn format_uptime(secs: u64, locale: Locale) -> String {
    if secs < 5 {
        return locale.pick("刚启动", "Just started").to_string();
    }
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        match locale {
            Locale::ZhCn => format!("{}小时 {}分钟", hours, minutes),
            Locale::En => format!("{}h {}m", hours, minutes),
        }
    } else if minutes > 0 {
        match locale {
            Locale::ZhCn => format!("{}分钟", minutes),
            Locale::En => format!("{}m", minutes),
        }
    } else {
        match locale {
            Locale::ZhCn => format!("{}秒", secs),
            Locale::En => format!("{}s", secs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pricing_variant_matching_and_free_models() {
        let mut rates = HashMap::new();
        rates.insert(
            "claude-opus-4-6".to_string(),
            ModelRate {
                prompt: 15.0,
                completion: 75.0,
            },
        );
        rates.insert(
            "gpt-5.4".to_string(),
            ModelRate {
                prompt: 2.5,
                completion: 10.0,
            },
        );

        let opus_1m = lookup_rate(&rates, "claude-opus-4-6[1m]");
        assert_eq!(opus_1m.prompt, 15.0);
        assert_eq!(opus_1m.completion, 75.0);

        let gpt_high = lookup_rate(&rates, "openai/gpt-5.4-high");
        assert_eq!(gpt_high.prompt, 2.5);
        assert_eq!(gpt_high.completion, 10.0);

        let free_model = lookup_rate(&rates, "openrouter/gpt-5.4:free");
        assert_eq!(free_model.prompt, 0.0);
        assert_eq!(free_model.completion, 0.0);
        assert_eq!(estimate_cost(&rates, "openrouter/gpt-5.4:free", 1_000_000, 1_000_000), 0.0);
    }
}
