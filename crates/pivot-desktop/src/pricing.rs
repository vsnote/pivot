use crate::i18n::Locale;
use pivot_core::db::RequestLog;
use pivot_core::service;
use std::collections::HashMap;

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

pub fn candidate_keys(model_name: &str) -> Vec<String> {
    let base = model_name.trim().to_lowercase();
    let mut keys = vec![base.clone()];
    if let Some(slash) = base.rfind('/') {
        keys.push(base[slash + 1..].to_string());
    }
    let snapshot = keys.clone();
    for k in snapshot {
        let slim = strip_date_or_latest_suffix(&k);
        if slim != k {
            keys.push(slim);
        }
    }
    keys
}

pub fn lookup_rate(rates: &HashMap<String, ModelRate>, model_name: &str) -> ModelRate {
    if model_name.trim().is_empty() {
        return DEFAULT_RATE;
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

/// 1:1 复刻 `UsageDashboard.tsx` 的 `getEffectiveTokens`
pub fn effective_tokens(log: &RequestLog) -> (u64, u64, u64) {
    let mut p = log.prompt_tokens.max(0) as u64;
    let mut c = log.completion_tokens.max(0) as u64;
    if p == 0 && c == 0 && (200..300).contains(&log.status_code) {
        c = 30u64.max(((log.latency_ms.max(0) as f64 / 1000.0) * 35.0).round() as u64);
        p = 1500u64.max(c * 3);
    }
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
