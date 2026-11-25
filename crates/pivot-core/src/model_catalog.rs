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
static DYNAMIC_NAMES_PATH: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelEffort {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl ModelEffort {
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Minimal => "Min",
            Self::Low => "Low",
            Self::Medium => "Med",
            Self::High => "High",
            Self::XHigh => "XHigh",
            Self::Max => "Max",
        }
    }

    pub fn full_label(self) -> &'static str {
        match self {
            Self::Minimal => "Minimal",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::XHigh => "XHigh",
            Self::Max => "Max",
        }
    }

    pub fn api_value(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }

    /// 仅从模型 ID 尾部识别规则推理强度后缀，严禁把 `-max` 当作 effort 孤立剥离
    pub fn from_suffix(id: &str) -> Option<(&str, Self)> {
        let lower = id.to_ascii_lowercase();
        for (suffix, effort) in [
            ("-xhigh", Self::XHigh),
            ("-high", Self::High),
            ("-medium", Self::Medium),
            ("-low", Self::Low),
            ("-minimal", Self::Minimal),
        ] {
            if lower.ends_with(suffix) && id.len() > suffix.len() {
                let stem = &id[..id.len() - suffix.len()];
                if !stem.ends_with('/') {
                    return Some((stem, effort));
                }
            }
        }
        None
    }

    /// 匹配尾部 `-max` 后缀，仅供同池存在标准 effort 变体时的上下文识别
    pub fn from_max_suffix(id: &str) -> Option<&str> {
        let lower = id.to_ascii_lowercase();
        if lower.ends_with("-max") && id.len() > 4 {
            let stem = &id[..id.len() - 4];
            if !stem.ends_with('/') {
                return Some(stem);
            }
        }
        None
    }

    pub fn parse_request_effort(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" | "med" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::XHigh),
            "max" | "ultra" => Some(Self::Max),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffortVariant {
    pub model_id: String,
    pub effort: ModelEffort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalModelEntry {
    /// 多档位组为基础模型 ID（如 "gemini-4-neon"、"gemini-4-hydrogen-tiered"）；单模型为原始 ID
    pub logical_id: String,
    pub base_name: String,
    pub extra_tags: Vec<String>,
    /// 仅单模型且自带孤立 effort 时有值（如 "gpt-oss-120b-medium"）
    pub single_effort: Option<ModelEffort>,
    /// 多档位合并组时 len >= 2 且按 ModelEffort 升序；单模型时为空
    pub variants: Vec<EffortVariant>,
    /// 底层全部真实上游模型 ID
    pub raw_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDisplayInfo {
    pub raw_id: String,
    /// 用于同源档位聚合的规范化键（小写；保留 org/ 前缀；不含 effort，保留 tiered/thinking/extra 等本质区别）
    pub group_key: String,
    /// 清晰的基础名称（不含 effort，但融入 Tiered / Thinking 等本质型号修饰）
    pub base_name: String,
    /// 识别出的规则推理档位
    pub effort: Option<ModelEffort>,
    /// 除 effort 外保留的语义/渠道/容量标签（如 "1M", "cc-format", "reversed", "Free"）
    pub extra_tags: Vec<String>,
    /// 完整无损单行显示名（如 "Gemini 4 Neon (XHigh)"、"Gemini 4 Hydrogen Tiered (High)"、"GPT-6 Astra (cc-format)"）
    pub display_name: String,
}

fn dynamic_names() -> &'static RwLock<HashMap<String, String>> {
    DYNAMIC_NAMES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn dynamic_names_path() -> &'static RwLock<Option<PathBuf>> {
    DYNAMIC_NAMES_PATH.get_or_init(|| RwLock::new(None))
}

pub fn is_meaningful_dynamic_name(id: &str, name: &str) -> bool {
    let clean_id = id.trim();
    let clean_name = name.trim();
    if clean_id.is_empty() || clean_name.is_empty() {
        return false;
    }
    let name_lower = clean_name.to_lowercase();
    if name_lower == "undefined" || name_lower.starts_with("models/") {
        return false;
    }
    let tail = clean_id.rsplit('/').next().unwrap_or(clean_id);
    if clean_name.eq_ignore_ascii_case(clean_id) || clean_name.eq_ignore_ascii_case(tail) {
        return false;
    }
    true
}

fn load_dynamic_names_from_path(path: &Path) -> HashMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    let Ok(raw) = serde_json::from_str::<HashMap<String, String>>(&text) else {
        return HashMap::new();
    };
    let mut cleaned = HashMap::new();
    for (k, v) in raw {
        let key = k.trim().to_lowercase();
        let val = v.trim().to_string();
        if is_meaningful_dynamic_name(&key, &val) {
            cleaned.insert(key, val);
        }
    }
    cleaned
}

fn save_dynamic_names_to_path(path: &Path, map: &HashMap<String, String>) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    if let Ok(text) = serde_json::to_string(map) {
        let _ = std::fs::write(path, text);
    }
}

/// 批量动态注册服务商返回的模型友好名称并按需持久化
pub fn register_dynamic_models(entries: &[(String, String)]) {
    let mut cleaned = Vec::new();
    for (id, name) in entries {
        let clean_id = id.trim();
        let clean_name = name.trim();
        if is_meaningful_dynamic_name(clean_id, clean_name) {
            cleaned.push((clean_id.to_lowercase(), clean_name.to_string()));
        }
    }
    if cleaned.is_empty() {
        return;
    }

    let snapshot = {
        let Ok(mut map) = dynamic_names().write() else {
            return;
        };
        let mut changed = false;
        for (k, v) in cleaned {
            if map.get(&k) != Some(&v) {
                map.insert(k, v);
                changed = true;
            }
        }
        if !changed {
            return;
        }
        map.clone()
    };

    if let Ok(guard) = dynamic_names_path().read() {
        if let Some(path) = guard.as_ref() {
            save_dynamic_names_to_path(path, &snapshot);
        }
    }
}

/// 动态注册服务商返回的模型友好名称（如 DeepSeek-V4.1-Flash）
pub fn register_dynamic_model(id: &str, name: &str, _provider: Option<&str>) {
    register_dynamic_models(&[(id.to_string(), name.to_string())]);
}

fn known_static_name(sub_lower: &str) -> Option<&'static str> {
    match sub_lower {
        "deepseek-flash" => Some("DeepSeek-V4.1-Flash"),
        "deepseek-v4-pro" => Some("DeepSeek-V4-Pro"),
        "deepseek-chat" => Some("DeepSeek-V3"),
        "deepseek-reasoner" => Some("DeepSeek-R1"),
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
    }
}

fn split_capacity_brackets(id: &str) -> (&str, Vec<String>) {
    let mut cur = id.trim();
    let mut tags = Vec::new();
    while cur.ends_with(']') {
        let Some(pos) = cur.rfind('[') else {
            break;
        };
        let inner = cur[pos + 1..cur.len() - 1].trim();
        let prefix = cur[..pos].trim_end();
        if prefix.is_empty() {
            break;
        }
        if !inner.is_empty() {
            tags.push(inner.to_ascii_uppercase());
        }
        cur = prefix;
    }
    tags.reverse();
    (cur, tags)
}

fn strip_effort_decoration_from_name(name: &str, effort: ModelEffort) -> String {
    let trimmed = name.trim();
    let mut patterns: Vec<String> = Vec::new();
    let full = effort.full_label().to_ascii_lowercase();
    let short = effort.short_label().to_ascii_lowercase();
    for tok in if full == short { vec![full] } else { vec![full, short] } {
        patterns.push(format!(" ({})", tok));
        patterns.push(format!("({})", tok));
        patterns.push(format!(" - {}", tok));
        patterns.push(format!(" {}", tok));
    }

    let lower = trimmed.to_ascii_lowercase();
    for pat in patterns {
        if lower.ends_with(&pat) && trimmed.len() > pat.len() {
            let stripped = trimmed[..trimmed.len() - pat.len()].trim_end();
            if !stripped.is_empty() {
                return stripped.to_string();
            }
        }
    }
    trimmed.to_string()
}

fn push_unique_tag_ci(tags: &mut Vec<String>, base_name_lower: &str, tag: String) {
    let t = tag.trim();
    if t.is_empty() {
        return;
    }
    let t_lower = t.to_lowercase();
    if base_name_lower.contains(&t_lower) {
        return;
    }
    if !tags.iter().any(|existing| existing.eq_ignore_ascii_case(t)) {
        tags.push(t.to_string());
    }
}

enum SemanticSuffix {
    ModelModifier(&'static str),
    ExtraTag(&'static str),
}

fn strip_one_semantic_suffix(id: &str) -> Option<(&str, SemanticSuffix)> {
    let lower = id.to_ascii_lowercase();
    for (suffix, kind) in [
        ("-tiered", SemanticSuffix::ModelModifier("Tiered")),
        ("-nothinking", SemanticSuffix::ExtraTag("No-Think")),
        ("-nothink", SemanticSuffix::ExtraTag("No-Think")),
        ("-thinking", SemanticSuffix::ModelModifier("Thinking")),
        ("-thought", SemanticSuffix::ModelModifier("Thinking")),
        ("-think", SemanticSuffix::ModelModifier("Thinking")),
        ("-cc-format", SemanticSuffix::ExtraTag("cc-format")),
        ("-cc", SemanticSuffix::ExtraTag("cc")),
        ("-reversed", SemanticSuffix::ExtraTag("reversed")),
        ("-preview", SemanticSuffix::ExtraTag("Preview")),
        ("-exp", SemanticSuffix::ExtraTag("Exp")),
        ("-search", SemanticSuffix::ExtraTag("Search")),
        ("-online", SemanticSuffix::ExtraTag("Online")),
        ("-free", SemanticSuffix::ExtraTag("Free")),
        ("-fc", SemanticSuffix::ExtraTag("fc")),
    ] {
        if lower.ends_with(suffix) && id.len() > suffix.len() {
            let prefix = &id[..id.len() - suffix.len()];
            if !prefix.ends_with('/') {
                return Some((prefix, kind));
            }
        }
    }
    None
}

fn exact_catalog_match(model: &CatalogModel, target_id: &str) -> bool {
    let target_lower = target_id.trim().to_lowercase();
    let target_tail = target_lower.rsplit('/').next().unwrap_or(&target_lower);
    let target_no_date = strip_version_suffix(&target_lower);
    let target_tail_no_date = strip_version_suffix(target_tail);

    let m_lower = model.id.trim().to_lowercase();
    let m_tail = m_lower.rsplit('/').next().unwrap_or(&m_lower);
    let m_no_date = strip_version_suffix(&m_lower);
    let m_tail_no_date = strip_version_suffix(m_tail);

    let mut targets = vec![target_lower.as_str(), target_tail];
    if let Some(ref nd) = target_no_date {
        targets.push(nd.as_str());
    }
    if let Some(ref nd) = target_tail_no_date {
        targets.push(nd.as_str());
    }

    let mut m_forms = vec![m_lower.as_str(), m_tail];
    if let Some(ref nd) = m_no_date {
        m_forms.push(nd.as_str());
    }
    if let Some(ref nd) = m_tail_no_date {
        m_forms.push(nd.as_str());
    }

    targets.iter().any(|t| m_forms.contains(t))
}

fn exact_catalog_name_from_slice(
    candidates: &[CatalogModel],
    query_id: &str,
    provider_hint: Option<&str>,
) -> Option<String> {
    let matched: Vec<CatalogModel> = candidates
        .iter()
        .filter(|m| exact_catalog_match(m, query_id))
        .filter(|m| {
            m.name
                .as_deref()
                .map(|n| !n.trim().is_empty())
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    if matched.is_empty() {
        return None;
    }
    let chosen = pick(&matched, provider_hint);
    chosen
        .name
        .as_deref()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
}

fn exact_catalog_name_in_store(query_id: &str, provider_hint: Option<&str>) -> Option<String> {
    let guard = store().read().ok()?;
    if guard.index.is_empty() {
        return None;
    }
    let lower = query_id.trim().to_lowercase();
    let mut keys = vec![lower.clone()];
    if let Some((_, rest)) = lower.rsplit_once('/') {
        if !rest.is_empty() && !keys.contains(&rest.to_string()) {
            keys.push(rest.to_string());
        }
    }
    for k in keys.clone() {
        if let Some(nd) = strip_version_suffix(&k) {
            if !keys.contains(&nd) {
                keys.push(nd);
            }
        }
    }
    for key in keys {
        if let Some(candidates) = guard.index.get(&key) {
            if let Some(name) = exact_catalog_name_from_slice(candidates, query_id, provider_hint) {
                return Some(name);
            }
        }
    }
    None
}

fn lookup_exact_base_name_with<C>(
    query: &str,
    provider_hint: Option<&str>,
    catalog_exact: &C,
) -> Option<String>
where
    C: Fn(&str, Option<&str>) -> Option<String>,
{
    let lower = query.trim().to_lowercase();
    if lower.is_empty() {
        return None;
    }
    let tail = lower.rsplit('/').next().unwrap_or(&lower);
    let tail_no_date = strip_version_suffix(tail);

    if let Some(n) = known_static_name(&lower)
        .or_else(|| known_static_name(tail))
        .or_else(|| tail_no_date.as_deref().and_then(known_static_name))
    {
        return Some(n.to_string());
    }

    if let Some(n) = catalog_exact(query, provider_hint) {
        return Some(n);
    }
    if let Some((_, rest)) = query.rsplit_once('/') {
        if !rest.is_empty() {
            if let Some(n) = catalog_exact(rest, provider_hint) {
                return Some(n);
            }
        }
    }
    None
}

fn parse_model_display_with<D, C>(
    model_id: &str,
    provider_hint: Option<&str>,
    dynamic_lookup: D,
    catalog_exact: C,
) -> ModelDisplayInfo
where
    D: Fn(&str) -> Option<String>,
    C: Fn(&str, Option<&str>) -> Option<String>,
{
    let trimmed = model_id.trim();
    if trimmed.is_empty() {
        return ModelDisplayInfo {
            raw_id: String::new(),
            group_key: String::new(),
            base_name: String::new(),
            effort: None,
            extra_tags: Vec::new(),
            display_name: String::new(),
        };
    }

    let (without_brackets, capacity_tags) = split_capacity_brackets(trimmed);
    let (stem, effort) = match ModelEffort::from_suffix(without_brackets) {
        Some((s, eff)) => (s, Some(eff)),
        None => (without_brackets, None),
    };
    let group_key = stem.to_lowercase();

    let raw_lower = trimmed.to_lowercase();
    let raw_tail_lower = raw_lower.rsplit('/').next().unwrap_or(&raw_lower).to_string();
    let wb_lower = without_brackets.to_lowercase();
    let wb_tail_lower = wb_lower.rsplit('/').next().unwrap_or(&wb_lower).to_string();

    let dyn_hit = dynamic_lookup(&raw_lower)
        .or_else(|| dynamic_lookup(&wb_lower))
        .or_else(|| dynamic_lookup(&raw_tail_lower))
        .or_else(|| dynamic_lookup(&wb_tail_lower));

    let mut extra_tags: Vec<String> = Vec::new();
    let base_name = if let Some(dyn_name) = dyn_hit {
        let clean_base = if let Some(eff) = effort {
            strip_effort_decoration_from_name(&dyn_name, eff)
        } else {
            dyn_name.trim().to_string()
        };
        let base_lower = clean_base.to_lowercase();
        for cap in capacity_tags {
            push_unique_tag_ci(&mut extra_tags, &base_lower, cap);
        }
        clean_base
    } else if let Some(exact_name) =
        lookup_exact_base_name_with(stem, provider_hint, &catalog_exact)
    {
        let base_lower = exact_name.to_lowercase();
        for cap in capacity_tags {
            push_unique_tag_ci(&mut extra_tags, &base_lower, cap);
        }
        exact_name
    } else {
        // 逐层剥离已知语义修饰后缀并记录，试图匹配基础型号
        let mut cur = stem;
        let mut mods_rev: Vec<&'static str> = Vec::new();
        let mut tags_rev: Vec<&'static str> = Vec::new();
        let mut matched_base: Option<String> = None;

        for _ in 0..8 {
            if let Some((next_cur, kind)) = strip_one_semantic_suffix(cur) {
                match kind {
                    SemanticSuffix::ModelModifier(m) => mods_rev.push(m),
                    SemanticSuffix::ExtraTag(t) => tags_rev.push(t),
                }
                cur = next_cur;
                if let Some(found) =
                    lookup_exact_base_name_with(cur, provider_hint, &catalog_exact)
                {
                    matched_base = Some(found);
                    break;
                }
            } else if let Some(no_ver) = strip_version_suffix(cur) {
                // 剥离日期/latest 后继续尝试，使用临时拥有的 String 转接
                let mut cur_owned = no_ver;
                let mut inner_found: Option<String> =
                    lookup_exact_base_name_with(&cur_owned, provider_hint, &catalog_exact);
                if inner_found.is_none() {
                    for _ in 0..4 {
                        if let Some((next_inner, kind)) = strip_one_semantic_suffix(&cur_owned) {
                            match kind {
                                SemanticSuffix::ModelModifier(m) => mods_rev.push(m),
                                SemanticSuffix::ExtraTag(t) => tags_rev.push(t),
                            }
                            cur_owned = next_inner.to_string();
                            if let Some(found) = lookup_exact_base_name_with(
                                &cur_owned,
                                provider_hint,
                                &catalog_exact,
                            ) {
                                inner_found = Some(found);
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                matched_base = inner_found;
                break;
            } else {
                break;
            }
        }

        if let Some(mut found_name) = matched_base {
            mods_rev.reverse();
            for m in mods_rev {
                if !found_name.to_lowercase().contains(&m.to_lowercase()) {
                    found_name.push(' ');
                    found_name.push_str(m);
                }
            }
            let base_lower = found_name.to_lowercase();
            tags_rev.reverse();
            for t in tags_rev {
                push_unique_tag_ci(&mut extra_tags, &base_lower, t.to_string());
            }
            for cap in capacity_tags {
                push_unique_tag_ci(&mut extra_tags, &base_lower, cap);
            }
            found_name
        } else if effort.is_some() || !capacity_tags.is_empty() {
            let base_lower = stem.to_lowercase();
            for cap in capacity_tags {
                push_unique_tag_ci(&mut extra_tags, &base_lower, cap);
            }
            stem.to_string()
        } else {
            trimmed.to_string()
        }
    };

    let mut suffix_parts = extra_tags.clone();
    if let Some(eff) = effort {
        suffix_parts.push(eff.full_label().to_string());
    }
    let display_name = if suffix_parts.is_empty() {
        base_name.clone()
    } else {
        format!("{} ({})", base_name, suffix_parts.join(", "))
    };

    ModelDisplayInfo {
        raw_id: trimmed.to_string(),
        group_key,
        base_name,
        effort,
        extra_tags,
        display_name,
    }
}

/// 解析结构化的无损模型显示信息，保留推理档位与渠道/容量变体
pub fn parse_model_display(model_id: &str, provider_hint: Option<&str>) -> ModelDisplayInfo {
    parse_model_display_with(
        model_id,
        provider_hint,
        |key| {
            dynamic_names()
                .read()
                .ok()
                .and_then(|map| map.get(key).cloned())
        },
        exact_catalog_name_in_store,
    )
}

/// 解析模型的用户友好显示名称 (name)，无损保留档位与变体后缀，无法匹配时回落至 model_id
pub fn resolve_model_display_name(model_id: &str, provider_hint: Option<&str>) -> String {
    parse_model_display(model_id, provider_hint).display_name
}

fn collapse_provider_models_with<D, C>(
    models: &[String],
    dynamic_lookup: D,
    catalog_exact: C,
) -> Vec<LogicalModelEntry>
where
    D: Fn(&str) -> Option<String>,
    C: Fn(&str, Option<&str>) -> Option<String>,
{
    let mut standard_effort_stems: std::collections::HashSet<(String, Vec<String>)> =
        std::collections::HashSet::new();
    for raw in models {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (wb, cap_tags) = split_capacity_brackets(trimmed);
        if let Some((stem, _)) = ModelEffort::from_suffix(wb) {
            standard_effort_stems.insert((stem.to_lowercase(), cap_tags));
        }
    }

    struct ParsedMember {
        logical_stem: String,
        info: ModelDisplayInfo,
    }

    let mut seen_raw = std::collections::HashSet::new();
    let mut slots: Vec<Vec<ParsedMember>> = Vec::new();
    let mut slot_map: HashMap<(String, Vec<String>), usize> = HashMap::new();

    for raw in models {
        let trimmed = raw.trim();
        if trimmed.is_empty() || !seen_raw.insert(trimmed.to_string()) {
            continue;
        }

        let (wb, cap_tags) = split_capacity_brackets(trimmed);
        let bracket_suffix = trimmed[wb.len()..].trim();

        let (logical_stem, info) = if let Some((stem, _)) = ModelEffort::from_suffix(wb) {
            let l_stem = if bracket_suffix.is_empty() {
                stem.to_string()
            } else {
                format!("{}{}", stem, bracket_suffix)
            };
            let info =
                parse_model_display_with(trimmed, None, &dynamic_lookup, &catalog_exact);
            (l_stem, info)
        } else if let Some(stem) = ModelEffort::from_max_suffix(wb) {
            if standard_effort_stems.contains(&(stem.to_lowercase(), cap_tags)) {
                let without_max = if bracket_suffix.is_empty() {
                    stem.to_string()
                } else {
                    format!("{}{}", stem, bracket_suffix)
                };
                let mut base_info = parse_model_display_with(
                    &without_max,
                    None,
                    &dynamic_lookup,
                    &catalog_exact,
                );
                let raw_lower = trimmed.to_lowercase();
                let raw_tail_lower = raw_lower
                    .rsplit('/')
                    .next()
                    .unwrap_or(&raw_lower)
                    .to_string();
                let wb_lower = wb.to_lowercase();
                let wb_tail_lower = wb_lower.rsplit('/').next().unwrap_or(&wb_lower).to_string();
                if let Some(dyn_name) = dynamic_lookup(&raw_lower)
                    .or_else(|| dynamic_lookup(&wb_lower))
                    .or_else(|| dynamic_lookup(&raw_tail_lower))
                    .or_else(|| dynamic_lookup(&wb_tail_lower))
                {
                    base_info.base_name =
                        strip_effort_decoration_from_name(&dyn_name, ModelEffort::Max);
                }
                let mut suffix_parts = base_info.extra_tags.clone();
                suffix_parts.push(ModelEffort::Max.full_label().to_string());
                let display_name =
                    format!("{} ({})", base_info.base_name, suffix_parts.join(", "));
                let info = ModelDisplayInfo {
                    raw_id: trimmed.to_string(),
                    group_key: base_info.group_key,
                    base_name: base_info.base_name,
                    effort: Some(ModelEffort::Max),
                    extra_tags: base_info.extra_tags,
                    display_name,
                };
                (without_max, info)
            } else {
                let info = parse_model_display_with(
                    trimmed,
                    None,
                    &dynamic_lookup,
                    &catalog_exact,
                );
                (trimmed.to_string(), info)
            }
        } else {
            let info = parse_model_display_with(trimmed, None, &dynamic_lookup, &catalog_exact);
            (trimmed.to_string(), info)
        };

        let key = (info.group_key.clone(), info.extra_tags.clone());
        let member = ParsedMember { logical_stem, info };
        if let Some(&idx) = slot_map.get(&key) {
            slots[idx].push(member);
        } else {
            let idx = slots.len();
            slot_map.insert(key, idx);
            slots.push(vec![member]);
        }
    }

    let mut out = Vec::new();
    for members in slots {
        let has_none_effort = members.iter().any(|m| m.info.effort.is_none());
        let unique_efforts: std::collections::HashSet<ModelEffort> = members
            .iter()
            .filter_map(|m| m.info.effort)
            .collect();

        if !has_none_effort && members.len() >= 2 && unique_efforts.len() == members.len() {
            let mut variants: Vec<EffortVariant> = members
                .iter()
                .filter_map(|m| {
                    m.info.effort.map(|effort| EffortVariant {
                        model_id: m.info.raw_id.clone(),
                        effort,
                    })
                })
                .collect();
            variants.sort_by_key(|v| v.effort);
            let raw_ids: Vec<String> = variants.iter().map(|v| v.model_id.clone()).collect();
            let first = &members[0];
            out.push(LogicalModelEntry {
                logical_id: first.logical_stem.clone(),
                base_name: first.info.base_name.clone(),
                extra_tags: first.info.extra_tags.clone(),
                single_effort: None,
                variants,
                raw_ids,
            });
        } else {
            for m in members {
                let raw_id = m.info.raw_id;
                out.push(LogicalModelEntry {
                    logical_id: raw_id.clone(),
                    base_name: m.info.base_name,
                    extra_tags: m.info.extra_tags,
                    single_effort: m.info.effort,
                    variants: Vec::new(),
                    raw_ids: vec![raw_id],
                });
            }
        }
    }

    out
}

/// 将服务商原始模型列表折叠为逻辑模型视图（保序、同源多档位聚合并支持同池 `-max` 上下文识别）
pub fn collapse_provider_models(models: &[String]) -> Vec<LogicalModelEntry> {
    collapse_provider_models_with(
        models,
        |key| {
            dynamic_names()
                .read()
                .ok()
                .and_then(|map| map.get(key).cloned())
        },
        exact_catalog_name_in_store,
    )
}

/// 在同一模型的有序档位变体集合中按请求 effort 进行精确命中或就近夹逼
pub fn select_variant_for_effort<'a>(
    variants: &'a [EffortVariant],
    requested: Option<ModelEffort>,
    thinking_disabled: bool,
) -> Option<&'a EffortVariant> {
    if variants.is_empty() {
        return None;
    }
    if thinking_disabled {
        return variants.first();
    }
    match requested {
        Some(target) => {
            if let Some(exact) = variants.iter().find(|v| v.effort == target) {
                return Some(exact);
            }
            if target < variants[0].effort {
                return variants.first();
            }
            if target > variants[variants.len() - 1].effort {
                return variants.last();
            }
            variants
                .iter()
                .find(|v| v.effort >= target)
                .or_else(|| variants.last())
        }
        None => variants
            .iter()
            .find(|v| v.effort == ModelEffort::Medium)
            .or_else(|| variants.iter().find(|v| v.effort == ModelEffort::High))
            .or_else(|| variants.get(variants.len() / 2)),
    }
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
    let dyn_path = cache_path.with_file_name("model-display-names.json");
    if let Ok(mut guard) = dynamic_names_path().write() {
        *guard = Some(dyn_path.clone());
    }
    let loaded_dyn = load_dynamic_names_from_path(&dyn_path);
    if !loaded_dyn.is_empty() {
        if let Ok(mut map) = dynamic_names().write() {
            for (k, v) in loaded_dyn {
                map.entry(k).or_insert(v);
            }
        }
    }

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

/// 一次查询需要尝试的所有等价写法：原名、去服务商前缀、去容量后缀、循环去变体后缀、去日期/版本后缀。
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

    // 剥离尾部方括号容量标签（如 [1m]、[1M]、[200k]）
    for key in keys.clone() {
        if key.ends_with(']') {
            if let Some(pos) = key.rfind('[') {
                let stripped = key[..pos].trim_end();
                if !stripped.is_empty() && !keys.contains(&stripped.to_string()) {
                    keys.push(stripped.to_string());
                }
            }
        }
    }

    // 多轮循环剥离渠道/思考变体后缀（如 claude-opus-4-6-thinking-high / claude-fable-5-1-reversed）
    for key in keys.clone() {
        let mut cur = key;
        while let Some(stripped) = strip_variant_suffix(&cur) {
            if !keys.contains(&stripped) {
                keys.push(stripped.clone());
            }
            cur = stripped;
        }
    }

    for key in keys.clone() {
        if let Some(stripped) = strip_version_suffix(&key) {
            if !keys.contains(&stripped) {
                keys.push(stripped);
            }
        }
    }

    // 组合剥离：循环变体 + 版本
    for key in keys.clone() {
        let mut cur = key;
        while let Some(v) = strip_variant_suffix(&cur) {
            if let Some(d) = strip_version_suffix(&v) {
                if !keys.contains(&d) {
                    keys.push(d);
                }
            }
            cur = v;
        }
    }

    keys
}

/// 剥离常见渠道代理、思考等级或格式后缀。
fn strip_variant_suffix(id: &str) -> Option<String> {
    for suffix in [
        "-xhigh",
        "-high",
        "-medium",
        "-low",
        "-minimal",
        "-max",
        "-thinking",
        "-thought",
        "-think",
        "-nothink",
        "-nothinking",
        "-tiered",
        "-reversed",
        "-cc-format",
        "-cc",
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
        assert!(canonical_keys("gpt-6-astra-cc-format").contains(&"gpt-6-astra".to_string()));
        assert!(canonical_keys("claude-fable-5-1-reversed[1M]").contains(&"claude-fable-5-1".to_string()));
        assert!(canonical_keys("gemini-4-hydrogen-xhigh").contains(&"gemini-4-hydrogen".to_string()));
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

    fn test_catalog_index() -> HashMap<String, Vec<CatalogModel>> {
        let models = parse_catalog(&serde_json::json!({
            "google": {
                "id": "google",
                "models": {
                    "gemini-4-neon": {
                        "id": "gemini-4-neon",
                        "name": "Gemini 4 Neon"
                    },
                    "gemini-4-hydrogen": {
                        "id": "gemini-4-hydrogen",
                        "name": "Gemini 4 Hydrogen"
                    }
                }
            },
            "anthropic": {
                "id": "anthropic",
                "models": {
                    "claude-sonnet-5-5": {
                        "id": "claude-sonnet-5-5",
                        "name": "Claude Sonnet 5.5"
                    },
                    "claude-fable-5-1": {
                        "id": "claude-fable-5-1",
                        "name": "Claude Fable 5.1"
                    }
                }
            },
            "openai": {
                "id": "openai",
                "models": {
                    "gpt-6-astra": {
                        "id": "gpt-6-astra",
                        "name": "GPT-6 Astra"
                    }
                }
            }
        }));
        build_index(models)
    }

    fn catalog_exact_from_index(
        index: &HashMap<String, Vec<CatalogModel>>,
        query_id: &str,
        hint: Option<&str>,
    ) -> Option<String> {
        let lower = query_id.trim().to_lowercase();
        let mut keys = vec![lower.clone()];
        if let Some((_, rest)) = lower.rsplit_once('/') {
            if !rest.is_empty() && !keys.contains(&rest.to_string()) {
                keys.push(rest.to_string());
            }
        }
        for k in keys.clone() {
            if let Some(nd) = strip_version_suffix(&k) {
                if !keys.contains(&nd) {
                    keys.push(nd);
                }
            }
        }
        for k in keys {
            if let Some(cands) = index.get(&k) {
                if let Some(name) = exact_catalog_name_from_slice(cands, query_id, hint) {
                    return Some(name);
                }
            }
        }
        None
    }

    #[test]
    fn parse_model_display_preserves_efforts_and_variants() {
        let index = test_catalog_index();
        let dyn_map: HashMap<String, String> = HashMap::new();
        let parse = |id: &str| {
            parse_model_display_with(
                id,
                None,
                |k| dyn_map.get(k).cloned(),
                |q, h| catalog_exact_from_index(&index, q, h),
            )
        };

        for (id, eff, disp) in [
            ("gemini-4-neon-low", ModelEffort::Low, "Gemini 4 Neon (Low)"),
            ("gemini-4-neon-medium", ModelEffort::Medium, "Gemini 4 Neon (Medium)"),
            ("gemini-4-neon-high", ModelEffort::High, "Gemini 4 Neon (High)"),
            ("gemini-4-neon-xhigh", ModelEffort::XHigh, "Gemini 4 Neon (XHigh)"),
        ] {
            let info = parse(id);
            assert_eq!(info.group_key, "gemini-4-neon");
            assert_eq!(info.base_name, "Gemini 4 Neon");
            assert_eq!(info.effort, Some(eff));
            assert!(info.extra_tags.is_empty());
            assert_eq!(info.display_name, disp);
        }

        let tiered_high = parse("gemini-4-hydrogen-tiered-high");
        assert_eq!(tiered_high.group_key, "gemini-4-hydrogen-tiered");
        assert_eq!(tiered_high.base_name, "Gemini 4 Hydrogen Tiered");
        assert_eq!(tiered_high.effort, Some(ModelEffort::High));
        assert_eq!(tiered_high.display_name, "Gemini 4 Hydrogen Tiered (High)");

        let tiered_xhigh = parse("gemini-4-hydrogen-tiered-xhigh");
        assert_eq!(tiered_xhigh.group_key, "gemini-4-hydrogen-tiered");
        assert_eq!(tiered_xhigh.base_name, "Gemini 4 Hydrogen Tiered");
        assert_eq!(tiered_xhigh.effort, Some(ModelEffort::XHigh));
        assert_eq!(tiered_xhigh.display_name, "Gemini 4 Hydrogen Tiered (XHigh)");

        let g38_high = parse("gemini-3.8-flash-high");
        let g38_low = parse("gemini-3.8-flash-low");
        assert_eq!(g38_high.group_key, "gemini-3.8-flash");
        assert_eq!(g38_low.group_key, "gemini-3.8-flash");
        assert_eq!(g38_high.base_name, "Gemini 3.8 Flash");
        assert_eq!(g38_high.display_name, "Gemini 3.8 Flash (High)");
        assert_eq!(g38_low.display_name, "Gemini 3.8 Flash (Low)");

        let cc_fmt = parse("gpt-6-astra-cc-format");
        assert_eq!(cc_fmt.base_name, "GPT-6 Astra");
        assert_eq!(cc_fmt.extra_tags, vec!["cc-format"]);
        assert_eq!(cc_fmt.display_name, "GPT-6 Astra (cc-format)");

        let rev_1m = parse("claude-fable-5-1-reversed[1M]");
        assert_eq!(rev_1m.base_name, "Claude Fable 5.1");
        assert_eq!(rev_1m.extra_tags, vec!["reversed", "1M"]);
        assert_eq!(rev_1m.display_name, "Claude Fable 5.1 (reversed, 1M)");

        // 严禁把 -max 当成 effort 剥离
        let qwen_max = parse("qwen-max");
        assert_eq!(qwen_max.effort, None);
        assert_eq!(qwen_max.base_name, "qwen-max");
        assert_eq!(qwen_max.display_name, "qwen-max");

        // 保留 org/ 前缀，防止跨服务商错合并
        let a = parse("openai/foo-high");
        let b = parse("anthropic/foo-low");
        assert_ne!(a.group_key, b.group_key);
        assert_eq!(a.group_key, "openai/foo");
        assert_eq!(b.group_key, "anthropic/foo");
    }

    #[test]
    fn parse_model_display_handles_dynamic_names_without_double_effort() {
        let index = test_catalog_index();
        let mut dyn_map = HashMap::new();
        dyn_map.insert("gemini-4-neon-low".to_string(), "Gemini 4 Neon".to_string());
        dyn_map.insert(
            "gemini-4-neon-xhigh".to_string(),
            "Gemini 4 Neon (XHigh)".to_string(),
        );
        dyn_map.insert(
            "gemini-4-neon-high".to_string(),
            "Gemini 4 Neon - High".to_string(),
        );
        dyn_map.insert(
            "gemini-4-neon-medium".to_string(),
            "Gemini 4 Neon (Med)".to_string(),
        );

        let parse = |id: &str| {
            parse_model_display_with(
                id,
                None,
                |k| dyn_map.get(k).cloned(),
                |q, h| catalog_exact_from_index(&index, q, h),
            )
        };

        assert_eq!(parse("gemini-4-neon-low").display_name, "Gemini 4 Neon (Low)");
        assert_eq!(parse("gemini-4-neon-xhigh").base_name, "Gemini 4 Neon");
        assert_eq!(
            parse("gemini-4-neon-xhigh").display_name,
            "Gemini 4 Neon (XHigh)"
        );
        assert_eq!(parse("gemini-4-neon-high").base_name, "Gemini 4 Neon");
        assert_eq!(parse("gemini-4-neon-high").display_name, "Gemini 4 Neon (High)");
        assert_eq!(parse("gemini-4-neon-medium").base_name, "Gemini 4 Neon");
        assert_eq!(
            parse("gemini-4-neon-medium").display_name,
            "Gemini 4 Neon (Medium)"
        );
    }

    #[test]
    fn dynamic_name_validation_and_persistence_roundtrip() {
        assert!(!is_meaningful_dynamic_name("gemini-2.5-pro", ""));
        assert!(!is_meaningful_dynamic_name("gemini-2.5-pro", "undefined"));
        assert!(!is_meaningful_dynamic_name(
            "gemini-2.5-pro",
            "models/gemini-2.5-pro"
        ));
        assert!(!is_meaningful_dynamic_name(
            "gemini-2.5-pro",
            "Gemini-2.5-Pro"
        ));
        assert!(!is_meaningful_dynamic_name(
            "google/gemini-2.5-pro",
            "gemini-2.5-pro"
        ));
        assert!(is_meaningful_dynamic_name(
            "gemini-4-neon-xhigh",
            "Gemini 4 Neon"
        ));

        let temp_dir = std::env::temp_dir().join(format!(
            "pivot-dyn-names-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let file_path = temp_dir.join("model-display-names.json");

        let mut map = HashMap::new();
        map.insert(
            "gemini-4-neon-xhigh".to_string(),
            "Gemini 4 Neon".to_string(),
        );
        map.insert(
            "bad-id".to_string(),
            "models/bad-id".to_string(),
        );
        save_dynamic_names_to_path(&file_path, &map);

        let reloaded = load_dynamic_names_from_path(&file_path);
        assert_eq!(
            reloaded.get("gemini-4-neon-xhigh").map(String::as_str),
            Some("Gemini 4 Neon")
        );
        assert!(!reloaded.contains_key("bad-id"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn canonical_keys_still_strip_tiered_for_capability_lookup() {
        assert!(canonical_keys("gemini-4-hydrogen-tiered-xhigh")
            .contains(&"gemini-4-hydrogen".to_string()));
    }

    #[test]
    fn collapse_provider_models_handles_subsets_max_context_and_isolated_models() {
        let index = test_catalog_index();
        let dyn_map: HashMap<String, String> = HashMap::new();
        let models: Vec<String> = vec![
            "gemini-4-neon-xhigh",
            "gemini-4-neon-low",
            "gemini-4-neon-high",
            "gemini-4-neon-medium",
            "claude-sonnet-5-5-low",
            "claude-sonnet-5-5-medium",
            "claude-sonnet-5-5-high",
            "gemini-4-hydrogen-tiered-xhigh",
            "gemini-4-hydrogen-tiered-high",
            "custom-thinker-low",
            "custom-thinker-high",
            "custom-thinker-max",
            "qwen-max",
            "gpt-oss-120b-medium",
            "claude-opus-5-5",
            "claude-opus-5-5-low",
            "claude-opus-5-5-high",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let entries = collapse_provider_models_with(
            &models,
            |k| dyn_map.get(k).cloned(),
            |q, h| catalog_exact_from_index(&index, q, h),
        );

        // neon(1) + sonnet5.5(1) + hydrogen-tiered(1) + custom-thinker(1) + qwen-max(1) + gpt-oss(1) + opus5.5本体+low+high(3) = 9
        assert_eq!(entries.len(), 9);

        assert_eq!(entries[0].logical_id, "gemini-4-neon");
        assert_eq!(entries[0].base_name, "Gemini 4 Neon");
        assert_eq!(
            entries[0]
                .variants
                .iter()
                .map(|v| v.effort)
                .collect::<Vec<_>>(),
            vec![
                ModelEffort::Low,
                ModelEffort::Medium,
                ModelEffort::High,
                ModelEffort::XHigh
            ]
        );

        assert_eq!(entries[1].logical_id, "claude-sonnet-5-5");
        assert_eq!(entries[1].variants.len(), 3);

        assert_eq!(entries[2].logical_id, "gemini-4-hydrogen-tiered");
        assert_eq!(entries[2].base_name, "Gemini 4 Hydrogen Tiered");
        assert_eq!(
            entries[2]
                .variants
                .iter()
                .map(|v| v.effort)
                .collect::<Vec<_>>(),
            vec![ModelEffort::High, ModelEffort::XHigh]
        );

        // 同池共存的 -max 正确识别为 ModelEffort::Max
        assert_eq!(entries[3].logical_id, "custom-thinker");
        assert_eq!(
            entries[3]
                .variants
                .iter()
                .map(|v| v.effort)
                .collect::<Vec<_>>(),
            vec![ModelEffort::Low, ModelEffort::High, ModelEffort::Max]
        );

        // 孤立 qwen-max 与单成员 gpt-oss-120b-medium 不合组也不误拆
        assert_eq!(entries[4].logical_id, "qwen-max");
        assert!(entries[4].variants.is_empty());
        assert_eq!(entries[4].single_effort, None);

        assert_eq!(entries[5].logical_id, "gpt-oss-120b-medium");
        assert!(entries[5].variants.is_empty());
        assert_eq!(entries[5].single_effort, Some(ModelEffort::Medium));

        // 存在无 effort 本体 claude-opus-5-5 时，不强行合并抢占 logical_id
        assert_eq!(entries[6].logical_id, "claude-opus-5-5");
        assert!(entries[6].variants.is_empty());
        assert_eq!(entries[7].logical_id, "claude-opus-5-5-low");
        assert_eq!(entries[8].logical_id, "claude-opus-5-5-high");
    }

    #[test]
    fn select_variant_for_effort_clamps_within_model_bounds() {
        let sonnet_variants = vec![
            EffortVariant {
                model_id: "claude-sonnet-5-5-low".into(),
                effort: ModelEffort::Low,
            },
            EffortVariant {
                model_id: "claude-sonnet-5-5-medium".into(),
                effort: ModelEffort::Medium,
            },
            EffortVariant {
                model_id: "claude-sonnet-5-5-high".into(),
                effort: ModelEffort::High,
            },
        ];

        assert_eq!(
            select_variant_for_effort(&sonnet_variants, Some(ModelEffort::Max), false)
                .unwrap()
                .model_id,
            "claude-sonnet-5-5-high"
        );
        assert_eq!(
            select_variant_for_effort(&sonnet_variants, Some(ModelEffort::Minimal), false)
                .unwrap()
                .model_id,
            "claude-sonnet-5-5-low"
        );
        assert_eq!(
            select_variant_for_effort(&sonnet_variants, None, false)
                .unwrap()
                .model_id,
            "claude-sonnet-5-5-medium"
        );
        assert_eq!(
            select_variant_for_effort(&sonnet_variants, Some(ModelEffort::High), true)
                .unwrap()
                .model_id,
            "claude-sonnet-5-5-low"
        );

        let tiered_variants = vec![
            EffortVariant {
                model_id: "gemini-4-hydrogen-tiered-high".into(),
                effort: ModelEffort::High,
            },
            EffortVariant {
                model_id: "gemini-4-hydrogen-tiered-xhigh".into(),
                effort: ModelEffort::XHigh,
            },
        ];
        assert_eq!(
            select_variant_for_effort(&tiered_variants, Some(ModelEffort::Low), false)
                .unwrap()
                .model_id,
            "gemini-4-hydrogen-tiered-high"
        );
        assert_eq!(
            select_variant_for_effort(&tiered_variants, None, false)
                .unwrap()
                .model_id,
            "gemini-4-hydrogen-tiered-high"
        );

        // 中间空缺夹逼：low, high 请求 medium -> high
        let gap_variants = vec![
            EffortVariant {
                model_id: "m-low".into(),
                effort: ModelEffort::Low,
            },
            EffortVariant {
                model_id: "m-high".into(),
                effort: ModelEffort::High,
            },
        ];
        assert_eq!(
            select_variant_for_effort(&gap_variants, Some(ModelEffort::Medium), false)
                .unwrap()
                .model_id,
            "m-high"
        );
    }
}
