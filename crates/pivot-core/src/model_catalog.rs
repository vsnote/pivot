//! 模型能力目录：从 models.dev 拉取模型元数据（输入模态、上下文窗口、推理能力、定价），
//! 本地缓存保证离线可用。查询命中时优先于名称启发式，未命中由调用方回落到启发式。
//!
//! 数据来源：https://models.dev （MIT）。缓存位置与数据库同目录。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

const CATALOG_URL: &str = "https://models.dev/api.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCost {
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default)]
    pub cache_read: f64,
    #[serde(default)]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    pub provider: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub input_modalities: Vec<String>,
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub max_output: Option<u64>,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub cost: Option<ModelCost>,
}

#[derive(Debug, Default)]
struct Catalog {
    index: HashMap<String, Vec<CatalogModel>>,
    model_count: usize,
    fetched_at: i64,
}

impl CatalogModel {
    /// 客户端（Pi / Codex）只消费 text 与 image，其余模态（pdf、audio…）不透传
    pub fn client_input_modalities(&self) -> Vec<&'static str> {
        self.input_modalities
            .iter()
            .filter_map(|s| match s.as_str() {
                "text" => Some("text"),
                "image" => Some("image"),
                _ => None,
            })
            .collect()
    }
}

#[derive(Serialize, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    fetched_at: i64,
    #[serde(default)]
    source: String,
    #[serde(default)]
    models: Vec<CatalogModel>,
}

static CATALOG: OnceLock<RwLock<Catalog>> = OnceLock::new();
static DYNAMIC_NAMES: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

fn dynamic_names() -> &'static RwLock<HashMap<String, String>> {
    DYNAMIC_NAMES.get_or_init(|| RwLock::new(HashMap::new()))
}

/// 动态注册服务商返回的模型友好名称（如 DeepSeek-V4.1-Flash）
pub fn register_dynamic_model(id: &str, name: &str, _provider: Option<&str>) {
    let clean_id = id.trim();
    let clean_name = name.trim();
    if clean_id.is_empty() || clean_name.is_empty() {
        return;
    }
    if let Ok(mut map) = dynamic_names().write() {
        map.insert(clean_id.to_lowercase(), clean_name.to_string());
    }
}

/// 解析模型的用户友好显示名称 (name)，无法匹配时回落至 model_id
pub fn resolve_model_display_name(model_id: &str, provider_hint: Option<&str>) -> String {
    let trimmed = model_id.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_lowercase();
    let sub = lower.split('/').last().unwrap_or(&lower);

    // 1. 优先查动态获取的最新官方名字
    if let Ok(map) = dynamic_names().read() {
        if let Some(name) = map.get(&lower).or_else(|| map.get(sub)) {
            return name.clone();
        }
    }

    // 2. 查静态知名模型映射表
    let known_name = match sub {
        "deepseek-flash" => Some("DeepSeek-V4.1-Flash"),
        "deepseek-v4-pro" => Some("DeepSeek-V4-Pro"),
        "deepseek-chat" => Some("DeepSeek-V3"),
        "deepseek-reasoner" => Some("DeepSeek-R1"),
        "gemini-3.8-flash-high" => Some("Gemini 3.8 Flash (High)"),
        "gemini-3.8-flash" => Some("Gemini 3.8 Flash"),
        "gemini-3-flash" => Some("Gemini 3 Flash"),
        "gemini-2.5-pro" => Some("Gemini 2.5 Pro"),
        "gemini-2.5-flash" => Some("Gemini 2.5 Flash"),
        "gemini-2.5-flash-thinking" => Some("Gemini 2.5 Flash Thinking"),
        "claude-opus-4-6-thinking" => Some("Claude Opus 4.6 Thinking"),
        "claude-sonnet-4-6" => Some("Claude Sonnet 4.6"),
        "claude-3-7-sonnet" | "claude-3-7-sonnet-20250219" => Some("Claude 3.7 Sonnet"),
        "claude-3-5-sonnet" | "claude-3-5-sonnet-20241022" => Some("Claude 3.5 Sonnet"),
        "claude-3-5-haiku" | "claude-3-5-haiku-20241022" => Some("Claude 3.5 Haiku"),
        "gpt-6.1-sol" => Some("GPT-6.1 Sol"),
        "gpt-6.1-terra" => Some("GPT-6.1 Terra"),
        "gpt-6-astra" => Some("GPT-6 Astra"),
        "gpt-6-sol" => Some("GPT-6 Sol"),
        "gpt-6-luna" => Some("GPT-6 Luna"),
        "gpt-5.6-sol" => Some("GPT-5.6 Sol"),
        "gpt-5.6-terra" => Some("GPT-5.6 Terra"),
        "gpt-5.6-luna" => Some("GPT-5.6 Luna"),
        "gpt-4o" => Some("GPT-4o"),
        "gpt-4o-mini" => Some("GPT-4o mini"),
        "o1" => Some("o1"),
        "o3-mini" => Some("o3-mini"),
        _ => None,
    };
    if let Some(n) = known_name {
        return n.to_string();
    }

    // 3. 查本地模型目录 (models.dev 缓存)
    if let Some(m) = lookup(trimmed, provider_hint) {
        if let Some(n) = m.name {
            if !n.trim().is_empty() {
                return n.trim().to_string();
            }
        }
    }

    trimmed.to_string()
}

fn store() -> &'static RwLock<Catalog> {
    CATALOG.get_or_init(|| RwLock::new(Catalog::default()))
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 启动时加载本地缓存。缓存缺失或损坏时留空，等后台刷新填充。
pub fn init(cache_path: PathBuf) {
    let Ok(text) = std::fs::read_to_string(&cache_path) else {
        return;
    };
    let Ok(file) = serde_json::from_str::<CatalogFile>(&text) else {
        return;
    };
    let model_count = file.models.len();
    if let Ok(mut guard) = store().write() {
        *guard = Catalog {
            index: build_index(file.models),
            model_count,
            fetched_at: file.fetched_at,
        };
    }
}

/// 拉取最新目录并替换内存索引；成功时顺带刷新本地缓存。
pub async fn refresh(client: &reqwest::Client, cache_path: &Path) -> Result<usize, String> {
    let raw: Value = client
        .get(CATALOG_URL)
        .timeout(std::time::Duration::from_secs(25))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("invalid json: {e}"))?;

    let models = parse_catalog(&raw);
    if models.is_empty() {
        return Err("catalog contained no models".to_string());
    }

    let count = models.len();
    let fetched_at = now_secs();

    let file = CatalogFile {
        fetched_at,
        source: CATALOG_URL.to_string(),
        models: models.clone(),
    };
    if let Ok(text) = serde_json::to_string(&file) {
        let _ = std::fs::write(cache_path, text);
    }

    if let Ok(mut guard) = store().write() {
        *guard = Catalog {
            index: build_index(models),
            model_count: count,
            fetched_at,
        };
    }
    Ok(count)
}

pub fn model_count() -> usize {
    store().read().map(|c| c.model_count).unwrap_or(0)
}

/// 前端成本估算用费率表：规范化模型名 → [输入价, 输出价]（USD / 1M tokens）。
/// 只为第一方 provider 有报价的条目生成，别名键由前端的规范化逻辑覆盖。
pub fn rate_table() -> Value {
    let Ok(guard) = store().read() else {
        return Value::Object(serde_json::Map::new());
    };
    let mut out = serde_json::Map::new();
    for (key, candidates) in &guard.index {
        let Some(cost) = &pick(candidates, None).cost else {
            continue;
        };
        if cost.input > 0.0 || cost.output > 0.0 {
            out.insert(key.clone(), serde_json::json!([cost.input, cost.output]));
        }
    }
    Value::Object(out)
}

pub fn fetched_at() -> i64 {
    store().read().map(|c| c.fetched_at).unwrap_or(0)
}

/// 缓存是否仍在 TTL 内，用于跳过不必要的网络刷新
pub fn is_fresh(max_age_secs: i64) -> bool {
    let fetched = fetched_at();
    fetched > 0 && now_secs().saturating_sub(fetched) < max_age_secs
}

/// 按模型名查目录。`provider_hint` 用于同一模型出现在多个服务商时消歧。
pub fn lookup(model_name: &str, provider_hint: Option<&str>) -> Option<CatalogModel> {
    let guard = store().read().ok()?;
    if guard.index.is_empty() {
        return None;
    }
    for key in canonical_keys(model_name) {
        if let Some(candidates) = guard.index.get(&key) {
            return Some(pick(candidates, provider_hint).clone());
        }
    }
    None
}

fn parse_catalog(raw: &Value) -> Vec<CatalogModel> {
    let mut out = Vec::new();
    let Some(providers) = raw.as_object() else {
        return out;
    };

    for (provider_id, provider) in providers {
        let Some(models) = provider.get("models").and_then(|m| m.as_object()) else {
            continue;
        };
        for (model_key, model) in models {
            let id = model
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(model_key)
                .trim()
                .to_string();
            if id.is_empty() {
                continue;
            }

            let input_modalities = model
                .pointer("/modalities/input")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                        .collect()
                })
                .unwrap_or_default();

            let cost = model.get("cost").and_then(|c| {
                let input = c.get("input").and_then(|v| v.as_f64())?;
                let output = c.get("output").and_then(|v| v.as_f64())?;
                Some(ModelCost {
                    input,
                    output,
                    cache_read: c.get("cache_read").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    cache_write: c.get("cache_write").and_then(|v| v.as_f64()).unwrap_or(0.0),
                })
            });

            out.push(CatalogModel {
                id,
                provider: provider_id.clone(),
                name: model
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                input_modalities,
                context_window: model
                    .pointer("/limit/context")
                    .and_then(|v| v.as_u64())
                    .filter(|v| *v > 0),
                max_output: model
                    .pointer("/limit/output")
                    .and_then(|v| v.as_u64())
                    .filter(|v| *v > 0),
                reasoning: model
                    .get("reasoning")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                cost,
            });
        }
    }
    out
}

fn build_index(models: Vec<CatalogModel>) -> HashMap<String, Vec<CatalogModel>> {
    let mut index: HashMap<String, Vec<CatalogModel>> = HashMap::new();
    for model in models {
        for key in canonical_keys(&model.id) {
            let bucket = index.entry(key).or_default();
            if !bucket.iter().any(|m| m.id == model.id && m.provider == model.provider) {
                bucket.push(model.clone());
            }
        }
    }
    index
}

/// 一次查询需要尝试的所有等价写法：原名、去服务商前缀、去变体后缀、去日期/版本后缀。
fn canonical_keys(id: &str) -> Vec<String> {
    let base = id.trim().to_lowercase();
    if base.is_empty() {
        return Vec::new();
    }

    let mut keys = vec![base.clone()];
    if let Some((_, rest)) = base.rsplit_once('/') {
        if !rest.is_empty() && !keys.contains(&rest.to_string()) {
            keys.push(rest.to_string());
        }
    }

    // 变体与思考等级剥离（如 -high, -medium, -low, -thinking 等）
    for key in keys.clone() {
        if let Some(stripped) = strip_variant_suffix(&key) {
            if !keys.contains(&stripped) {
                keys.push(stripped);
            }
        }
    }

    for key in keys.clone() {
        if let Some(stripped) = strip_version_suffix(&key) {
            if !keys.contains(&stripped) {
                keys.push(stripped);
            }
        }
    }

    // 组合剥离：变体 + 版本
    for key in keys.clone() {
        if let Some(v) = strip_variant_suffix(&key) {
            if let Some(d) = strip_version_suffix(&v) {
                if !keys.contains(&d) {
                    keys.push(d);
                }
            }
        }
    }

    keys
}

/// 剥离常见渠道代理或模型变体后缀（如 -high, -medium, -low, -thinking 等）。
fn strip_variant_suffix(id: &str) -> Option<String> {
    for suffix in [
        "-high",
        "-medium",
        "-low",
        "-thinking",
        "-thought",
        "-think",
        "-nothink",
        "-nothinking",
        "-preview",
        "-exp",
        "-search",
        "-online",
        "-free",
        "-fc",
    ] {
        if let Some(stripped) = id.strip_suffix(suffix) {
            if !stripped.is_empty() {
                return Some(stripped.to_string());
            }
        }
    }
    None
}

/// 去掉 `-latest` 与日期后缀（`-2024-08-06` / `-20241022`）。
fn strip_version_suffix(id: &str) -> Option<String> {
    let mut current = id.to_string();
    let mut changed = false;

    for suffix in ["-latest", ":latest"] {
        if let Some(stripped) = current.strip_suffix(suffix) {
            current = stripped.to_string();
            changed = true;
        }
    }

    for (idx, _) in id.match_indices('-').rev() {
        let tail = &id[idx + 1..];
        let digits = tail.chars().filter(|c| c.is_ascii_digit()).count();
        let only_date = tail.len() >= 8
            && digits >= 8
            && tail.chars().all(|c| c.is_ascii_digit() || c == '-');
        if only_date {
            current = id[..idx].to_string();
            changed = true;
            break;
        }
    }

    if changed && !current.is_empty() {
        Some(current)
    } else {
        None
    }
}

/// models.dev 里同一模型常被几十个聚合商重复收录，字段质量参差。
/// 已知第一方 provider 优先，其余保持 provider id 字典序，保证结果确定。
const FIRST_PARTY_PROVIDERS: &[&str] = &[
    "anthropic",
    "openai",
    "google",
    "google-vertex",
    "azure",
    "amazon-bedrock",
    "deepseek",
    "xai",
    "alibaba",
    "moonshotai",
    "zai",
    "zhipuai",
    "mistral",
    "meta",
    "cohere",
    "openrouter",
    "vercel",
];

fn provider_rank(provider: &str) -> usize {
    FIRST_PARTY_PROVIDERS
        .iter()
        .position(|p| *p == provider)
        .unwrap_or(FIRST_PARTY_PROVIDERS.len())
}

fn pick<'a>(candidates: &'a [CatalogModel], hint: Option<&str>) -> &'a CatalogModel {
    if let Some(hint) = hint {
        let hint = hint.to_lowercase();
        if let Some(m) = candidates
            .iter()
            .find(|c| c.provider.to_lowercase() == hint)
        {
            return m;
        }
        if let Some(m) = candidates.iter().find(|c| {
            let p = c.provider.to_lowercase();
            p.contains(&hint) || hint.contains(&p)
        }) {
            return m;
        }
    }
    candidates
        .iter()
        .min_by_key(|c| provider_rank(&c.provider))
        .unwrap_or(&candidates[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<CatalogModel> {
        parse_catalog(&serde_json::json!({
            "anthropic": {
                "id": "anthropic",
                "models": {
                    "claude-opus-4-5": {
                        "id": "claude-opus-4-5",
                        "name": "Claude Opus 4.5",
                        "reasoning": true,
                        "modalities": { "input": ["text", "image", "pdf"], "output": ["text"] },
                        "limit": { "context": 200000, "output": 64000 },
                        "cost": { "input": 5, "output": 25, "cache_read": 0.5, "cache_write": 6.25 }
                    }
                }
            },
            "aihubmix": {
                "id": "aihubmix",
                "models": {
                    "claude-opus-4-5": {
                        "id": "claude-opus-4-5",
                        "modalities": { "input": ["text"], "output": ["text"] },
                        "limit": { "context": 999999, "output": 999999 }
                    }
                }
            },
            "openrouter": {
                "id": "openrouter",
                "models": {
                    "claude-opus-4-5": {
                        "id": "claude-opus-4-5",
                        "modalities": { "input": ["text"], "output": ["text"] },
                        "limit": { "context": 100000, "output": 32000 }
                    }
                }
            }
        }))
    }

    #[test]
    fn parses_modalities_limits_and_cost() {
        let models = sample();
        assert_eq!(models.len(), 3);
        let m = models.iter().find(|m| m.provider == "anthropic").unwrap();
        assert_eq!(m.input_modalities, vec!["text", "image", "pdf"]);
        assert_eq!(m.context_window, Some(200000));
        assert_eq!(m.max_output, Some(64000));
        assert!(m.reasoning);
        assert_eq!(m.cost.as_ref().unwrap().input, 5.0);
    }

    #[test]
    fn canonical_keys_cover_prefix_and_date_suffix() {
        assert!(canonical_keys("meta-llama/llama-4-scout").contains(&"llama-4-scout".to_string()));
        assert!(canonical_keys("claude-3-5-sonnet-20241022").contains(&"claude-3-5-sonnet".to_string()));
        assert!(canonical_keys("gpt-5-latest").contains(&"gpt-5".to_string()));
    }

    #[test]
    fn client_modalities_drop_unsupported_ones() {
        let m = CatalogModel {
            id: "claude-opus-4-5".to_string(),
            provider: "anthropic".to_string(),
            name: None,
            input_modalities: vec!["text".to_string(), "image".to_string(), "pdf".to_string()],
            context_window: None,
            max_output: None,
            reasoning: true,
            cost: None,
        };
        assert_eq!(m.client_input_modalities(), vec!["text", "image"]);

        let text_only = CatalogModel {
            input_modalities: vec!["text".to_string()],
            ..m
        };
        assert_eq!(text_only.client_input_modalities(), vec!["text"]);

        let unknown = CatalogModel {
            input_modalities: vec!["pdf".to_string()],
            ..text_only
        };
        assert!(unknown.client_input_modalities().is_empty());
    }

    #[test]
    fn provider_hint_disambiguates_duplicates() {
        let index = build_index(sample());
        let candidates = index.get("claude-opus-4-5").unwrap();
        assert_eq!(pick(candidates, Some("openrouter")).provider, "openrouter");
        assert_eq!(pick(candidates, Some("anthropic")).context_window, Some(200000));
        // 无提示时选第一方条目，而不是字典序更靠前的聚合商
        assert_eq!(pick(candidates, None).provider, "anthropic");
    }
}
