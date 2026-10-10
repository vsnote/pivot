use crate::db::{Database, Provider};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThirdPartyConfig {
    pub endpoint: String,
    pub auth_token: Option<String>,
    pub suggested_name: String,
    pub protocol: String,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterceptorInfo {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub config_path: String,
    pub is_intercepted: bool,
    pub backup_path: Option<String>,
    pub original_endpoint: Option<String>,
    pub conflict: Option<String>,
    pub third_party: Option<ThirdPartyConfig>,
    #[serde(default)]
    pub is_overwritten: bool,
}

pub struct InterceptorManager {
    db: Database,
    gateway_url: String,
}

impl InterceptorManager {
    pub fn new(db: Database, port: u16) -> Self {
        Self {
            db,
            gateway_url: format!("http://127.0.0.1:{}", port),
        }
    }

    fn home_dir() -> Option<PathBuf> {
        dirs::home_dir()
    }

    /// Check if CLI binary exists in PATH
    fn is_binary_installed(bin_name: &str) -> bool {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let bin_path = dir.join(bin_name);
                if bin_path.is_file() {
                    return true;
                }
            }
        }
        false
    }

    /// Detect status for all supported tools
    pub fn get_all_status(&self) -> Vec<InterceptorInfo> {
        vec![
            self.get_claude_code_status(),
            self.get_codex_status(),
            self.get_gemini_cli_status(),
        ]
    }

    /// Extract full third-party configuration (endpoint, auth token, suggested name, protocol, models)
    pub fn get_third_party_config(&self, tool_id: &str) -> Option<ThirdPartyConfig> {
        match tool_id {
            "claude_code" => {
                let mut ep_opt = None;
                let mut token = None;
                let mut configured_models = Vec::new();

                // 1. 优先检查 ~/.claude/settings.json (现代 Claude Code 全局配置)
                let settings_path = Self::claude_settings_path();
                if settings_path.exists() {
                    if let Ok(content) = fs::read_to_string(&settings_path) {
                        if !content.contains(&self.gateway_url) {
                            if let Ok(v) = serde_json::from_str::<Value>(&content) {
                                if let Some(env_obj) = v.get("env").and_then(|e| e.as_object()) {
                                    if let Some(url) = env_obj.get("ANTHROPIC_BASE_URL").and_then(|u| u.as_str()) {
                                        ep_opt = Some(url.to_string());
                                    }
                                    token = env_obj
                                        .get("ANTHROPIC_AUTH_TOKEN")
                                        .or_else(|| env_obj.get("ANTHROPIC_API_KEY"))
                                        .and_then(|t| t.as_str())
                                        .map(|s| s.to_string());

                                    // 提取用户真实配置的模型
                                    for (k, val) in env_obj {
                                        if (k.contains("MODEL") || k == "model")
                                            && !k.contains("HEADER")
                                            && !k.contains("EXPORTER")
                                            && !k.contains("EFFORT")
                                        {
                                            if let Some(m) = val.as_str() {
                                                let clean = m.split('[').next().unwrap_or(m).trim();
                                                if !clean.is_empty() && !configured_models.contains(&clean.to_string()) {
                                                    configured_models.push(clean.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                                if ep_opt.is_none() {
                                    if let Some(url) = v.get("anthropicBaseUrl").and_then(|u| u.as_str()) {
                                        ep_opt = Some(url.to_string());
                                    }
                                }
                            }
                        }
                    }
                }

                // 2. 检查 ~/.claude.json
                if ep_opt.is_none() {
                    let config_path = Self::claude_config_path();
                    if config_path.exists() {
                        if let Ok(content) = fs::read_to_string(&config_path) {
                            if !content.contains(&self.gateway_url) {
                                if let Ok(v) = serde_json::from_str::<Value>(&content) {
                                    let url = v
                                        .get("anthropicBaseUrl")
                                        .and_then(|u| u.as_str())
                                        .or_else(|| {
                                            v.get("env")
                                                .and_then(|e| e.get("ANTHROPIC_BASE_URL"))
                                                .and_then(|u| u.as_str())
                                        });
                                    if let Some(u) = url {
                                        ep_opt = Some(u.to_string());
                                    }
                                    if token.is_none() {
                                        token = v
                                            .get("primaryApiKey")
                                            .or_else(|| v.get("apiKey"))
                                            .and_then(|t| t.as_str())
                                            .map(|s| s.to_string());
                                    }
                                }
                            }
                        }
                    }
                }

                let raw_ep = ep_opt?;
                let clean_ep = raw_ep.trim().trim_end_matches('/').to_string();
                if clean_ep.is_empty() || clean_ep == "https://api.anthropic.com" {
                    return None;
                }

                let name = if clean_ep.contains("127.0.0.1") || clean_ep.contains("localhost") {
                    let port = clean_ep.rsplit(':').next().unwrap_or("代理");
                    format!("本地代理 ({})", port)
                } else {
                    let host = clean_ep
                        .trim_start_matches("http://")
                        .trim_start_matches("https://")
                        .split('/')
                        .next()
                        .unwrap_or("代理");
                    format!("第三方服务 ({})", host)
                };

                let models = if !configured_models.is_empty() {
                    configured_models
                } else {
                    vec![
                        "claude-3-5-sonnet-20241022".to_string(),
                        "claude-3-7-sonnet-20250219".to_string(),
                        "claude-3-opus-20240229".to_string(),
                        "claude-3-5-haiku-20241022".to_string(),
                    ]
                };

                Some(ThirdPartyConfig {
                    endpoint: clean_ep,
                    auth_token: token,
                    suggested_name: name,
                    protocol: "anthropic".to_string(),
                    models,
                })
            }
            "codex_cli" => {
                let config_path = Self::codex_config_path();
                if !config_path.exists() {
                    return None;
                }
                let content = fs::read_to_string(&config_path).ok()?;
                if content.contains(&self.gateway_url) {
                    return None;
                }
                let doc = toml::from_str::<toml::Value>(&content).ok()?;
                let current_provider = doc
                    .get("model_provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or("custom");

                let mut token = None;
                let ep = if let Some(p) = doc
                    .get("model_providers")
                    .and_then(|mp| mp.get(current_provider))
                {
                    token = p
                        .get("experimental_bearer_token")
                        .or_else(|| p.get("api_key"))
                        .and_then(|k| k.as_str())
                        .map(|s| s.to_string());
                    p.get("base_url").and_then(|u| u.as_str())
                } else {
                    doc.get("base_url").and_then(|u| u.as_str())
                }?;

                let clean_ep = ep.trim().trim_end_matches('/').to_string();
                if clean_ep.is_empty()
                    || clean_ep == "https://api.openai.com"
                    || clean_ep == "https://api.openai.com/v1"
                {
                    return None;
                }

                let name = if clean_ep.contains("127.0.0.1") || clean_ep.contains("localhost") {
                    let port = clean_ep.rsplit(':').next().unwrap_or("代理");
                    format!("本地代理 ({})", port)
                } else {
                    let host = clean_ep
                        .trim_start_matches("http://")
                        .trim_start_matches("https://")
                        .split('/')
                        .next()
                        .unwrap_or("代理");
                    format!("Codex 渠道 ({})", host)
                };

                let models = if let Some(m) = doc.get("model").and_then(|m| m.as_str()) {
                    let clean = m.trim();
                    if !clean.is_empty() {
                        vec![clean.to_string()]
                    } else {
                        vec![
                            "gpt-4o".to_string(),
                            "gpt-4o-mini".to_string(),
                            "o1".to_string(),
                            "o3-mini".to_string(),
                        ]
                    }
                } else {
                    vec![
                        "gpt-4o".to_string(),
                        "gpt-4o-mini".to_string(),
                        "o1".to_string(),
                        "o3-mini".to_string(),
                    ]
                };

                Some(ThirdPartyConfig {
                    endpoint: clean_ep,
                    auth_token: token,
                    suggested_name: name,
                    protocol: "openai".to_string(),
                    models,
                })
            }
            "gemini_cli" => {
                let mut ep_opt = None;
                let mut token = None;

                let env_path = Self::gemini_env_path();
                if env_path.exists() {
                    if let Ok(content) = fs::read_to_string(&env_path) {
                        if !content.contains(&self.gateway_url) {
                            for line in content.lines() {
                                let trimmed = line.trim();
                                if let Some(stripped) = trimmed.strip_prefix("GOOGLE_GEMINI_BASE_URL=") {
                                    ep_opt = Some(stripped.trim().trim_matches('"').trim_matches('\'').to_string());
                                } else if let Some(stripped) = trimmed.strip_prefix("GEMINI_BASE_URL=") {
                                    if ep_opt.is_none() {
                                        ep_opt = Some(stripped.trim().trim_matches('"').trim_matches('\'').to_string());
                                    }
                                } else if let Some(stripped) = trimmed.strip_prefix("GEMINI_API_KEY=") {
                                    token = Some(stripped.trim().trim_matches('"').trim_matches('\'').to_string());
                                }
                            }
                        }
                    }
                }

                if ep_opt.is_none() {
                    let config_path = Self::gemini_config_path();
                    if config_path.exists() {
                        if let Ok(content) = fs::read_to_string(&config_path) {
                            if !content.contains(&self.gateway_url) {
                                if let Ok(v) = serde_json::from_str::<Value>(&content) {
                                    if let Some(url) = v.get("baseUrl").and_then(|u| u.as_str()) {
                                        ep_opt = Some(url.to_string());
                                    }
                                }
                            }
                        }
                    }
                }

                let raw_ep = ep_opt?;
                let clean_ep = raw_ep.trim().trim_end_matches('/').to_string();
                if clean_ep.is_empty() || clean_ep.contains("googleapis.com") {
                    return None;
                }

                let name = if clean_ep.contains("127.0.0.1") || clean_ep.contains("localhost") {
                    let port = clean_ep.rsplit(':').next().unwrap_or("代理");
                    format!("本地 Gemini 代理 ({})", port)
                } else {
                    let host = clean_ep
                        .trim_start_matches("http://")
                        .trim_start_matches("https://")
                        .split('/')
                        .next()
                        .unwrap_or("代理");
                    format!("Gemini 渠道 ({})", host)
                };

                Some(ThirdPartyConfig {
                    endpoint: clean_ep,
                    auth_token: token,
                    suggested_name: name,
                    protocol: "gemini".to_string(),
                    models: vec!["gemini-2.0-flash".to_string(), "gemini-2.5-pro".to_string()],
                })
            }
            _ => None,
        }
    }

    /// Extracts the current custom endpoint from the tool's configuration if it points
    /// to a third-party service (i.e. neither official upstream nor this OmniSwitch gateway).
    fn detect_third_party_endpoint(&self, tool_id: &str) -> Option<String> {
        self.get_third_party_config(tool_id).map(|tp| tp.endpoint)
    }

    /// Generate user-facing conflict warning message if a third-party endpoint is detected.
    fn check_third_party_conflict(&self, tool_id: &str, is_overwritten: bool) -> Option<String> {
        let ep = self.detect_third_party_endpoint(tool_id)?;
        if is_overwritten {
            Some(format!(
                "检测到当前配置已被第三方工具覆盖（{}）！为避免持续争夺权限，AnyRelay 已停止自动覆盖。建议在外部工具中关闭自动同步，然后重新接管或导入为上游服务商。",
                ep
            ))
        } else {
            Some(format!(
                "检测到当前配置了第三方端点（{}）。开启接管将自动备份该配置。若后台运行有其他代理管理工具，建议先关闭其自动同步，避免配置冲突。",
                ep
            ))
        }
    }

    // ================= Claude Code =================

    fn claude_settings_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude")
            .join("settings.json")
    }

    fn claude_settings_backup_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude")
            .join("settings.json.omniswitch.bak")
    }

    fn claude_config_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude.json")
    }

    fn claude_backup_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude.json.omniswitch.bak")
    }

    pub fn get_claude_code_status(&self) -> InterceptorInfo {
        let settings_path = Self::claude_settings_path();
        let settings_backup_path = Self::claude_settings_backup_path();
        let config_path = Self::claude_config_path();
        let backup_path = Self::claude_backup_path();

        let installed = Self::is_binary_installed("claude")
            || settings_path.exists()
            || config_path.exists()
            || Self::home_dir()
                .map(|h| h.join(".claude").exists())
                .unwrap_or(false);

        let mut db_is_intercepted = false;
        let mut original_endpoint = None;

        if let Ok(Some((db_intercepted, _, orig_ep))) = self.db.get_interceptor_state("claude_code")
        {
            db_is_intercepted = db_intercepted;
            original_endpoint = orig_ep;
        }

        let mut file_intercepted = false;
        if settings_path.exists() {
            if let Ok(content) = fs::read_to_string(&settings_path) {
                if content.contains(&self.gateway_url) {
                    file_intercepted = true;
                }
            }
        }
        if !file_intercepted && config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if content.contains(&self.gateway_url) {
                    file_intercepted = true;
                }
            }
        }

        let is_intercepted = if settings_path.exists() || config_path.exists() {
            file_intercepted
        } else {
            db_is_intercepted
        };

        let is_overwritten = db_is_intercepted && !file_intercepted;

        let effective_config_path = if settings_path.exists() {
            settings_path.to_string_lossy().to_string()
        } else {
            config_path.to_string_lossy().to_string()
        };

        let effective_backup_path = if settings_backup_path.exists() {
            Some(settings_backup_path.to_string_lossy().to_string())
        } else if backup_path.exists() {
            Some(backup_path.to_string_lossy().to_string())
        } else {
            None
        };

        let third_party = self.get_third_party_config("claude_code");
        let original_endpoint = third_party.as_ref().map(|tp| tp.endpoint.clone()).or(original_endpoint);

        InterceptorInfo {
            id: "claude_code".to_string(),
            name: "Claude Code CLI".to_string(),
            installed,
            config_path: effective_config_path,
            is_intercepted,
            backup_path: effective_backup_path,
            original_endpoint,
            conflict: self.check_third_party_conflict("claude_code", is_overwritten),
            third_party,
            is_overwritten,
        }
    }

    pub fn toggle_claude_code(&self, enable: bool) -> Result<InterceptorInfo, String> {
        let settings_path = Self::claude_settings_path();
        let settings_backup_path = Self::claude_settings_backup_path();
        let config_path = Self::claude_config_path();
        let backup_path = Self::claude_backup_path();

        if enable {
            // 提取现有第三方端点作为记录
            let orig_ep = self.detect_third_party_endpoint("claude_code");

            // 1. 接管 ~/.claude/settings.json (现代 Claude Code 全局官方环境变量配置)
            let mut orig_settings = String::new();
            if settings_path.exists() {
                orig_settings = fs::read_to_string(&settings_path).unwrap_or_default();
            } else if let Some(p) = settings_path.parent() {
                let _ = fs::create_dir_all(p);
            }

            if !settings_backup_path.exists() && settings_path.exists() {
                let _ = atomic_write(&settings_backup_path, &orig_settings);
            }

            let mut settings_val: Value =
                serde_json::from_str(&orig_settings).unwrap_or_else(|_| json!({}));
            if !settings_val.is_object() {
                settings_val = json!({});
            }
            if !settings_val["env"].is_object() {
                settings_val["env"] = json!({});
            }
            settings_val["env"]["ANTHROPIC_BASE_URL"] = json!(self.gateway_url);

            if let Ok(new_settings) = serde_json::to_string_pretty(&settings_val) {
                let _ = atomic_write(&settings_path, &new_settings);
            }

            // 2. 同步接管 ~/.claude.json (如果文件存在)
            if config_path.exists() {
                let orig_config = fs::read_to_string(&config_path).unwrap_or_default();
                if !backup_path.exists() {
                    let _ = atomic_write(&backup_path, &orig_config);
                }
                if let Ok(mut config_val) = serde_json::from_str::<Value>(&orig_config) {
                    config_val["anthropicBaseUrl"] = json!(self.gateway_url);
                    if !config_val["env"].is_object() {
                        config_val["env"] = json!({});
                    }
                    config_val["env"]["ANTHROPIC_BASE_URL"] = json!(self.gateway_url);
                    if let Ok(new_config) = serde_json::to_string_pretty(&config_val) {
                        let _ = atomic_write(&config_path, &new_config);
                    }
                }
            }

            let _ = self.db.save_interceptor_state(
                "claude_code",
                true,
                Some(&orig_settings),
                orig_ep.as_deref(),
            );
        } else {
            // 恢复 ~/.claude/settings.json
            if settings_backup_path.exists() {
                if let Ok(orig) = fs::read_to_string(&settings_backup_path) {
                    let _ = atomic_write(&settings_path, &orig);
                }
                let _ = fs::remove_file(&settings_backup_path);
            } else if settings_path.exists() {
                if let Ok(content) = fs::read_to_string(&settings_path) {
                    if let Ok(mut v) = serde_json::from_str::<Value>(&content) {
                        if let Some(obj) = v.as_object_mut() {
                            obj.remove("anthropicBaseUrl");
                            if let Some(env_obj) =
                                obj.get_mut("env").and_then(|e| e.as_object_mut())
                            {
                                env_obj.remove("ANTHROPIC_BASE_URL");
                            }
                        }
                        if let Ok(updated) = serde_json::to_string_pretty(&v) {
                            let _ = atomic_write(&settings_path, &updated);
                        }
                    }
                }
            }

            // 恢复 ~/.claude.json
            if backup_path.exists() {
                if let Ok(orig) = fs::read_to_string(&backup_path) {
                    let _ = atomic_write(&config_path, &orig);
                }
                let _ = fs::remove_file(&backup_path);
            } else if config_path.exists() {
                if let Ok(content) = fs::read_to_string(&config_path) {
                    if let Ok(mut v) = serde_json::from_str::<Value>(&content) {
                        if let Some(obj) = v.as_object_mut() {
                            obj.remove("anthropicBaseUrl");
                            if let Some(env_obj) =
                                obj.get_mut("env").and_then(|e| e.as_object_mut())
                            {
                                env_obj.remove("ANTHROPIC_BASE_URL");
                            }
                        }
                        if let Ok(updated) = serde_json::to_string_pretty(&v) {
                            let _ = atomic_write(&config_path, &updated);
                        }
                    }
                }
            }

            let _ = self
                .db
                .save_interceptor_state("claude_code", false, None, None);
        }

        Ok(self.get_claude_code_status())
    }

    // ================= Codex CLI =================

    fn codex_config_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".codex")
            .join("config.toml")
    }

    fn codex_backup_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".codex")
            .join("config.toml.omniswitch.bak")
    }

    pub fn get_codex_status(&self) -> InterceptorInfo {
        let config_path = Self::codex_config_path();
        let backup_path = Self::codex_backup_path();
        let installed = Self::is_binary_installed("codex")
            || config_path.exists()
            || Self::home_dir()
                .map(|h| h.join(".codex").exists())
                .unwrap_or(false);

        let mut db_is_intercepted = false;
        let mut original_endpoint = None;

        if let Ok(Some((db_intercepted, _, orig_ep))) = self.db.get_interceptor_state("codex_cli") {
            db_is_intercepted = db_intercepted;
            original_endpoint = orig_ep;
        }

        let mut file_intercepted = false;
        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                let target_endpoint = format!("{}/v1", self.gateway_url);
                if content.contains(&self.gateway_url) || content.contains(&target_endpoint) {
                    file_intercepted = true;
                }
            }
        }

        let is_intercepted = if config_path.exists() {
            file_intercepted
        } else {
            db_is_intercepted
        };

        let is_overwritten = db_is_intercepted && !file_intercepted;

        if is_intercepted {
            let catalog_path = Self::codex_catalog_path();
            let missing_catalog = !catalog_path.exists();
            let missing_config_entry = config_path.exists()
                && fs::read_to_string(&config_path)
                    .map(|c| !c.contains(Self::CODEX_CATALOG_FILENAME))
                    .unwrap_or(false);
            if missing_catalog || missing_config_entry {
                let _ = self.do_sync_codex_catalog();
            }
        }

        let third_party = self.get_third_party_config("codex_cli");
        let original_endpoint = third_party.as_ref().map(|tp| tp.endpoint.clone()).or(original_endpoint);

        InterceptorInfo {
            id: "codex_cli".to_string(),
            name: "OpenAI Codex CLI".to_string(),
            installed,
            config_path: config_path.to_string_lossy().to_string(),
            is_intercepted,
            backup_path: if backup_path.exists() {
                Some(backup_path.to_string_lossy().to_string())
            } else {
                None
            },
            original_endpoint,
            conflict: self.check_third_party_conflict("codex_cli", is_overwritten),
            third_party,
            is_overwritten,
        }
    }

    pub const CODEX_CATALOG_FILENAME: &'static str = "omniswitch-model-catalog.json";

    fn codex_catalog_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".codex")
            .join(Self::CODEX_CATALOG_FILENAME)
    }

    fn do_sync_codex_catalog(&self) -> Result<(), String> {
        let providers = self.db.get_providers().map_err(|e| e.to_string())?;
        let catalog = generate_codex_catalog_json(&providers);
        let catalog_path = Self::codex_catalog_path();
        if let Ok(catalog_str) = serde_json::to_string_pretty(&catalog) {
            let _ = atomic_write(&catalog_path, &catalog_str);
        }

        let config_path = Self::codex_config_path();
        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                let target_endpoint = format!("{}/v1", self.gateway_url);
                if let Ok(updated) = update_codex_config_toml(
                    &content,
                    &target_endpoint,
                    Some(Self::CODEX_CATALOG_FILENAME),
                    None,
                ) {
                    let _ = atomic_write(&config_path, &updated);
                }
            }
        }

        Ok(())
    }

    pub fn sync_codex_catalog_if_intercepted(&self) -> Result<(), String> {
        let status = self.get_codex_status();
        if status.is_intercepted {
            self.do_sync_codex_catalog()?;
        }
        Ok(())
    }

    pub fn toggle_codex(&self, enable: bool) -> Result<InterceptorInfo, String> {
        let config_path = Self::codex_config_path();
        let backup_path = Self::codex_backup_path();
        let catalog_path = Self::codex_catalog_path();
        let target_endpoint = format!("{}/v1", self.gateway_url);

        if enable {
            let mut original_content = String::new();
            if config_path.exists() {
                original_content = fs::read_to_string(&config_path).unwrap_or_default();
            } else if let Some(p) = config_path.parent() {
                let _ = fs::create_dir_all(p);
            }
            let saved_state = self
                .db
                .get_interceptor_state("codex_cli")
                .ok()
                .flatten();
            let already_intercepted = backup_path.exists()
                || saved_state.as_ref().map(|state| state.0).unwrap_or(false)
                || original_content.contains(&target_endpoint);

            if !already_intercepted && config_path.exists() {
                atomic_write(&backup_path, &original_content).map_err(|e| e.to_string())?;
            }

            // 生成并写入 catalog 文件
            let providers = self.db.get_providers().unwrap_or_default();
            let catalog = generate_codex_catalog_json(&providers);
            if let Ok(catalog_str) = serde_json::to_string_pretty(&catalog) {
                let _ = atomic_write(&catalog_path, &catalog_str);
            }

            let preferred_model = providers
                .iter()
                .find(|p| p.is_default && p.enabled)
                .and_then(|p| {
                    let prefix = p.name.to_lowercase().replace(' ', "-");
                    let m = p.default_model.as_deref().or_else(|| p.models.first().map(|s| s.as_str()))?;
                    Some(if m.to_lowercase().starts_with(&format!("{}/", prefix)) {
                        m.to_string()
                    } else {
                        format!("{}/{}", prefix, m)
                    })
                })
                .or_else(|| {
                    catalog["models"]
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|m| m["slug"].as_str().map(|s| s.to_string()))
                });

            let new_content = update_codex_config_toml(
                &original_content,
                &target_endpoint,
                Some(Self::CODEX_CATALOG_FILENAME),
                preferred_model.as_deref(),
            )?;
            atomic_write(&config_path, &new_content).map_err(|e| e.to_string())?;

            if !already_intercepted {
                let _ = self
                    .db
                    .save_interceptor_state("codex_cli", true, Some(&original_content), None);
            }
        } else {
            if backup_path.exists() {
                if let Ok(orig) = fs::read_to_string(&backup_path) {
                    let _ = atomic_write(&config_path, &orig);
                }
                let _ = fs::remove_file(&backup_path);
            } else if let Ok(Some((_, Some(backup_content), _))) =
                self.db.get_interceptor_state("codex_cli")
            {
                let _ = atomic_write(&config_path, &backup_content);
            } else if config_path.exists() {
                if let Ok(content) = fs::read_to_string(&config_path) {
                    if let Ok(cleaned) = update_codex_config_toml(
                        &content,
                        "https://api.openai.com/v1",
                        None,
                        None,
                    ) {
                        let _ = atomic_write(&config_path, &cleaned);
                    }
                }
            }

            if catalog_path.exists() {
                let _ = fs::remove_file(&catalog_path);
            }

            let _ = self
                .db
                .save_interceptor_state("codex_cli", false, None, None);
        }

        Ok(self.get_codex_status())
    }

    // ================= Gemini CLI =================

    fn gemini_config_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gemini")
            .join("config.json")
    }

    fn gemini_backup_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gemini")
            .join("config.json.omniswitch.bak")
    }

    fn gemini_env_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gemini")
            .join(".env")
    }

    fn gemini_env_backup_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gemini")
            .join(".env.omniswitch.bak")
    }

    pub fn get_gemini_cli_status(&self) -> InterceptorInfo {
        let env_path = Self::gemini_env_path();
        let config_path = Self::gemini_config_path();
        let backup_path = Self::gemini_backup_path();
        let installed = Self::is_binary_installed("gemini")
            || env_path.exists()
            || config_path.exists()
            || Self::home_dir()
                .map(|h| h.join(".gemini").exists())
                .unwrap_or(false);

        let mut db_is_intercepted = false;
        let mut original_endpoint = None;

        if let Ok(Some((db_intercepted, _, orig_ep))) = self.db.get_interceptor_state("gemini_cli")
        {
            db_is_intercepted = db_intercepted;
            original_endpoint = orig_ep;
        }

        let mut file_intercepted = false;
        if env_path.exists() {
            if let Ok(content) = fs::read_to_string(&env_path) {
                if content.contains(&self.gateway_url) {
                    file_intercepted = true;
                }
            }
        }
        if !file_intercepted && config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if content.contains(&self.gateway_url) {
                    file_intercepted = true;
                }
            }
        }

        let is_intercepted = if env_path.exists() || config_path.exists() {
            file_intercepted
        } else {
            db_is_intercepted
        };

        let is_overwritten = db_is_intercepted && !file_intercepted;

        let third_party = self.get_third_party_config("gemini_cli");
        let original_endpoint = third_party.as_ref().map(|tp| tp.endpoint.clone()).or(original_endpoint);

        InterceptorInfo {
            id: "gemini_cli".to_string(),
            name: "Gemini CLI".to_string(),
            installed,
            config_path: if env_path.exists() {
                env_path.to_string_lossy().to_string()
            } else {
                config_path.to_string_lossy().to_string()
            },
            is_intercepted,
            backup_path: if backup_path.exists() {
                Some(backup_path.to_string_lossy().to_string())
            } else {
                None
            },
            original_endpoint,
            conflict: self.check_third_party_conflict("gemini_cli", is_overwritten),
            third_party,
            is_overwritten,
        }
    }

    pub fn toggle_gemini_cli(&self, enable: bool) -> Result<InterceptorInfo, String> {
        let config_path = Self::gemini_config_path();
        let backup_path = Self::gemini_backup_path();
        let env_path = Self::gemini_env_path();
        let env_backup_path = Self::gemini_env_backup_path();

        if enable {
            // 1. 接管 ~/.gemini/.env (Gemini CLI 官方真实环境变量文件)
            let mut orig_env = String::new();
            if env_path.exists() {
                orig_env = fs::read_to_string(&env_path).unwrap_or_default();
            } else if let Some(p) = env_path.parent() {
                let _ = fs::create_dir_all(p);
            }
            if !env_backup_path.exists() && env_path.exists() {
                let _ = atomic_write(&env_backup_path, &orig_env);
            }
            let new_env = update_gemini_env(&orig_env, &self.gateway_url);
            let _ = atomic_write(&env_path, &new_env);

            // 2. 同时维护 config.json (兼容旧配置和测试用例)
            let mut original_content = String::new();
            let mut parsed: Value = json!({});
            if config_path.exists() {
                original_content = fs::read_to_string(&config_path).unwrap_or_default();
                if let Ok(v) = serde_json::from_str::<Value>(&original_content) {
                    parsed = v;
                }
            } else if let Some(p) = config_path.parent() {
                let _ = fs::create_dir_all(p);
            }
            if !backup_path.exists() && config_path.exists() {
                let _ = atomic_write(&backup_path, &original_content);
            }
            parsed["baseUrl"] = json!(self.gateway_url);
            if let Ok(new_content) = serde_json::to_string_pretty(&parsed) {
                let _ = atomic_write(&config_path, &new_content);
            }

            let _ = self
                .db
                .save_interceptor_state("gemini_cli", true, Some(&orig_env), None);
        } else {
            // 恢复 ~/.gemini/.env
            if env_backup_path.exists() {
                if let Ok(orig) = fs::read_to_string(&env_backup_path) {
                    let _ = atomic_write(&env_path, &orig);
                }
                let _ = fs::remove_file(&env_backup_path);
            } else if env_path.exists() {
                if let Ok(content) = fs::read_to_string(&env_path) {
                    let cleaned: Vec<&str> = content
                        .lines()
                        .filter(|l| {
                            !l.contains(&self.gateway_url)
                                && !l.starts_with("GOOGLE_GEMINI_BASE_URL=")
                                && !l.starts_with("GEMINI_BASE_URL=")
                        })
                        .collect();
                    let _ = atomic_write(&env_path, &(cleaned.join("\n") + "\n"));
                }
            }

            // 恢复 config.json
            if backup_path.exists() {
                if let Ok(orig) = fs::read_to_string(&backup_path) {
                    let _ = atomic_write(&config_path, &orig);
                }
                let _ = fs::remove_file(&backup_path);
            }

            let _ = self
                .db
                .save_interceptor_state("gemini_cli", false, None, None);
        }

        Ok(self.get_gemini_cli_status())
    }

    pub fn toggle_tool(&self, id: &str, enable: bool) -> Result<InterceptorInfo, String> {
        match id {
            "claude_code" => self.toggle_claude_code(enable),
            "codex_cli" => self.toggle_codex(enable),
            "gemini_cli" => self.toggle_gemini_cli(enable),
            _ => Err(format!("Unknown tool id: {}", id)),
        }
    }
}

/// Update Codex config.toml while preserving all sections, comments, and modern model_providers tables
fn update_codex_config_toml(
    original: &str,
    target_endpoint: &str,
    catalog_filename: Option<&str>,
    preferred_model: Option<&str>,
) -> Result<String, String> {
    let mut doc: toml::Value = if original.trim().is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        toml::from_str(original).map_err(|e| format!("解析 Codex config.toml 失败: {}", e))?
    };

    if let toml::Value::Table(ref mut map) = doc {
        // 1. 设置根级 base_url 兼容旧版 CLI 与工具
        map.insert(
            "base_url".to_string(),
            toml::Value::String(target_endpoint.to_string()),
        );

        // 2. 注入或移除 model_catalog_json (必须处于 TOML 根级)
        if let Some(catalog) = catalog_filename {
            map.insert(
                "model_catalog_json".to_string(),
                toml::Value::String(catalog.to_string()),
            );
        } else {
            map.remove("model_catalog_json");
        }

        // 3. 设定优先模型（如果有指定）
        if let Some(model) = preferred_model {
            map.insert(
                "model".to_string(),
                toml::Value::String(model.to_string()),
            );
        }

        // 4. 兼容现代多 Provider 模式 (如 model_providers.custom)
        let current_provider = map
            .get("model_provider")
            .and_then(|v| v.as_str())
            .unwrap_or("custom")
            .to_string();

        map.insert(
            "model_provider".to_string(),
            toml::Value::String(current_provider.clone()),
        );

        let model_providers = map
            .entry("model_providers".to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

        if let toml::Value::Table(ref mut providers_map) = model_providers {
            let provider_table = providers_map
                .entry(current_provider)
                .or_insert_with(|| {
                    let mut p = toml::map::Map::new();
                    p.insert("name".to_string(), toml::Value::String("OmniSwitch".to_string()));
                    p.insert("wire_api".to_string(), toml::Value::String("responses".to_string()));
                    p.insert("requires_openai_auth".to_string(), toml::Value::Boolean(true));
                    toml::Value::Table(p)
                });

            if let toml::Value::Table(ref mut p_map) = provider_table {
                p_map.insert(
                    "base_url".to_string(),
                    toml::Value::String(target_endpoint.to_string()),
                );
            }
        }
    }

    toml::to_string_pretty(&doc).map_err(|e| format!("序列化 Codex config.toml 失败: {}", e))
}

fn load_native_codex_models_cache() -> HashMap<String, Value> {
    let mut map = HashMap::new();
    let cache_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
        .join("models_cache.json");

    if let Ok(content) = fs::read_to_string(&cache_path) {
        if let Ok(val) = serde_json::from_str::<Value>(&content) {
            if let Some(models) = val.get("models").and_then(Value::as_array) {
                for item in models {
                    if let Some(slug) = item.get("slug").and_then(Value::as_str) {
                        map.insert(slug.to_lowercase(), item.clone());
                    }
                }
            }
        }
    }
    map
}

pub fn resolve_model_context_window(model_name: &str) -> (u64, u64) {
    let lower = model_name.to_lowercase();
    let name = lower.split('/').last().unwrap_or(&lower);

    if name.starts_with("gpt-5")
        || name.starts_with("gpt-6")
        || name.starts_with("gpt-reserve")
        || name.contains("codex-auto-review")
    {
        (272000, 872000)
    } else if name.contains("gemini") {
        (1000000, 2000000)
    } else if name.contains("claude") {
        (200000, 200000)
    } else if name.contains("deepseek") {
        (64000, 64000)
    } else {
        (128000, 128000)
    }
}

pub fn generate_codex_catalog_json(providers: &[Provider]) -> Value {
    let native_cache = load_native_codex_models_cache();
    let mut models = Vec::new();
    let mut seen_slugs = HashSet::new();
    let mut priority = 1000;

    for provider in providers.iter().filter(|p| p.enabled) {
        let mut all_models = Vec::new();
        if let Some(ref dm) = provider.default_model {
            let dm_trimmed = dm.trim();
            if !dm_trimmed.is_empty() {
                all_models.push(dm_trimmed.to_string());
            }
        }
        for m in &provider.models {
            let trimmed = m.trim().to_string();
            if !trimmed.is_empty() && !all_models.contains(&trimmed) {
                all_models.push(trimmed);
            }
        }

        let provider_prefix = provider.name.to_lowercase().replace(' ', "-");
        for model_name in all_models {
            let slug = if model_name.to_lowercase().starts_with(&format!("{}/", provider_prefix)) {
                model_name.clone()
            } else {
                format!("{}/{}", provider_prefix, model_name)
            };

            if seen_slugs.contains(&slug) {
                continue;
            }
            seen_slugs.insert(slug.clone());

            let display_name = slug.clone();
            let description = format!("{} via {}", slug, provider.name);

            let lower = model_name.to_lowercase();
            let base_name = lower.split('/').last().unwrap_or(&lower);

            let model_entry = if let Some(native_item) = native_cache.get(base_name) {
                let mut entry = native_item.clone();
                entry["slug"] = json!(slug);
                entry["display_name"] = json!(display_name);
                entry["description"] = json!(description);
                entry["priority"] = json!(priority);
                entry["visibility"] = json!("list");
                entry["supported_in_api"] = json!(true);
                entry
            } else {
                let (supported_reasoning_levels, default_reasoning_level) =
                    resolve_model_reasoning_spec(&model_name);
                let (context_window, max_context_window) =
                    resolve_model_context_window(&model_name);

                json!({
                    "slug": slug,
                    "display_name": display_name,
                    "description": description,
                    "priority": priority,
                    "visibility": "list",
                    "supported_in_api": true,
                    "context_window": context_window,
                    "max_context_window": max_context_window,
                    "effective_context_window_percent": 95,
                    "input_modalities": ["text", "image"],
                    "shell_type": "shell_command",
                    "base_instructions": "You are Codex, a coding agent. You and the user share the same workspace and collaborate to achieve the user's goals.",
                    "default_reasoning_level": default_reasoning_level,
                    "default_reasoning_summary": "none",
                    "supported_reasoning_levels": supported_reasoning_levels,
                    "supports_image_detail_original": false,
                    "supports_parallel_tool_calls": false,
                    "supports_reasoning_summaries": true,
                    "supports_search_tool": false,
                    "truncation_policy": {
                        "limit": 10000,
                        "mode": "bytes"
                    },
                    "additional_speed_tiers": [],
                    "service_tiers": [],
                    "experimental_supported_tools": [],
                    "support_verbosity": false,
                    "availability_nux": Value::Null,
                    "upgrade": Value::Null
                })
            };

            models.push(model_entry);
            priority += 1;
        }
    }

    json!({ "models": models })
}

/// Accurately resolves supported reasoning levels and default effort for a model
pub fn resolve_model_reasoning_spec(model_name: &str) -> (Vec<Value>, &'static str) {
    let lower = model_name.to_lowercase();
    let name = lower.split('/').last().unwrap_or(&lower);

    // 0. Non-text / Non-Reasoning Modalities (生图、图像生成、语音、嵌入等辅助模型)
    let is_media_or_aux = name.contains("image")
        || name.contains("imagen")
        || name.contains("dall-e")
        || name.contains("flux")
        || name.contains("embedding")
        || name.contains("embed")
        || name.contains("tts")
        || name.contains("whisper")
        || name.contains("audio")
        || name.contains("realtime");

    if is_media_or_aux {
        return (
            vec![json!({ "description": "Default", "effort": "none" })],
            "none",
        );
    }

    // 1. Explicit Non-Reasoning Models (纯经典非推理模型，不支持思考深度调节)
    let is_openai_chat = name.starts_with("gpt-4")
        || name.starts_with("gpt-3")
        || name.starts_with("chatgpt-4");

    let is_claude_non_thinking = (name.contains("claude-3-5")
        || name.contains("claude-3-opus")
        || name.contains("claude-3-haiku")
        || name.starts_with("claude-2"))
        && !name.contains("thinking");

    let is_gemini_non_thinking = (name.starts_with("gemini-1.")
        || name.starts_with("gemini-pro")
        || name.starts_with("gemini-flash"))
        && !name.contains("thinking")
        && !name.contains("2.")
        && !name.contains("3.");

    let is_deepseek_chat = (name == "deepseek-chat" || name == "deepseek-coder")
        && !name.contains("r1")
        && !name.contains("reason")
        && !name.contains("v4");

    let is_llama_or_qwen_chat = (name.starts_with("llama") || (name.starts_with("qwen") && !name.contains("qwq")))
        && !name.contains("thinking")
        && !name.contains("r1");

    if is_openai_chat
        || is_claude_non_thinking
        || is_gemini_non_thinking
        || is_deepseek_chat
        || is_llama_or_qwen_chat
    {
        return (
            vec![json!({ "description": "Default", "effort": "none" })],
            "none",
        );
    }

    // 2. OpenAI Reasoning Models (o1, o3, gpt-5, gpt-6)
    if name.starts_with("o1") || name.starts_with("o3") {
        return (
            vec![
                json!({ "description": "Low Thinking", "effort": "low" }),
                json!({ "description": "Medium Thinking", "effort": "medium" }),
                json!({ "description": "High Thinking", "effort": "high" }),
            ],
            "medium",
        );
    }

    if name.starts_with("gpt-5") || name.starts_with("gpt-6") {
        return (
            vec![
                json!({ "description": "Fast responses with lighter reasoning", "effort": "low" }),
                json!({ "description": "Balances speed and reasoning depth for everyday tasks", "effort": "medium" }),
                json!({ "description": "Greater reasoning depth for complex problems", "effort": "high" }),
                json!({ "description": "Extra high reasoning depth for complex problems", "effort": "xhigh" }),
                json!({ "description": "Maximum reasoning depth for the hardest problems", "effort": "max" }),
                json!({ "description": "Maximum reasoning with automatic task delegation", "effort": "ultra" }),
            ],
            if name.contains("sol") { "low" } else { "medium" },
        );
    }

    // 3. Claude Extended Thinking (Claude 3.7 Sonnet, Claude 4.x, or explicitly named thinking)
    if name.contains("claude-3-7")
        || name.contains("opus-4")
        || name.contains("sonnet-4")
        || name.contains("thinking")
    {
        return (
            vec![
                json!({ "description": "Disable Thinking", "effort": "none" }),
                json!({ "description": "Low Thinking", "effort": "low" }),
                json!({ "description": "Medium Thinking", "effort": "medium" }),
                json!({ "description": "High Thinking", "effort": "high" }),
            ],
            if name.contains("thinking") {
                "high"
            } else {
                "medium"
            },
        );
    }

    // 4. DeepSeek Reasoning Models (R1, Reasoner, V4)
    if name.contains("reason") || name.contains("r1") || name.contains("deepseek-v4") {
        return (
            vec![
                json!({ "description": "Disable Thinking", "effort": "none" }),
                json!({ "description": "Low Thinking", "effort": "low" }),
                json!({ "description": "Medium Thinking", "effort": "medium" }),
                json!({ "description": "High Thinking", "effort": "high" }),
            ],
            "high",
        );
    }

    // 5. Gemini Reasoning Models (2.5, 3.x, High, Thinking)
    if name.contains("gemini-2.5")
        || name.contains("gemini-3")
        || name.contains("high")
        || name.contains("thinking")
    {
        return (
            vec![
                json!({ "description": "Disable Thinking", "effort": "none" }),
                json!({ "description": "Low Thinking", "effort": "low" }),
                json!({ "description": "Medium Thinking", "effort": "medium" }),
                json!({ "description": "High Thinking", "effort": "high" }),
            ],
            "medium",
        );
    }

    // 6. Other Reasoning Models (QwQ, etc.)
    if name.contains("qwq") {
        return (
            vec![
                json!({ "description": "Disable Thinking", "effort": "none" }),
                json!({ "description": "Low Thinking", "effort": "low" }),
                json!({ "description": "Medium Thinking", "effort": "medium" }),
                json!({ "description": "High Thinking", "effort": "high" }),
            ],
            "medium",
        );
    }

    // 7. General Custom / Unknown Models: Default to none with option to configure
    (
        vec![
            json!({ "description": "Default", "effort": "none" }),
            json!({ "description": "Low Thinking", "effort": "low" }),
            json!({ "description": "Medium Thinking", "effort": "medium" }),
            json!({ "description": "High Thinking", "effort": "high" }),
        ],
        "none",
    )
}

/// Update Gemini CLI official .env file with gateway base URLs
fn update_gemini_env(original: &str, gateway_url: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut found_google = false;
    let mut found_gemini = false;

    for line in original.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("GOOGLE_GEMINI_BASE_URL=") {
            lines.push(format!("GOOGLE_GEMINI_BASE_URL={}", gateway_url));
            found_google = true;
        } else if trimmed.starts_with("GEMINI_BASE_URL=") {
            lines.push(format!("GEMINI_BASE_URL={}", gateway_url));
            found_gemini = true;
        } else {
            lines.push(line.to_string());
        }
    }

    if !found_google {
        lines.push(format!("GOOGLE_GEMINI_BASE_URL={}", gateway_url));
    }
    if !found_gemini {
        lines.push(format!("GEMINI_BASE_URL={}", gateway_url));
    }

    lines.join("\n") + "\n"
}

/// Safely write to file via a temporary file + rename to guarantee atomicity and crash-safety
fn atomic_write<P: AsRef<Path>>(path: P, content: &str) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
    fs::write(&temp_path, content)?;
    fs::rename(temp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static HOME_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn repeated_enable_preserves_original_cli_configuration() {
        let _lock = HOME_LOCK.lock().unwrap();
        let home = std::env::temp_dir().join(format!(
            "omniswitch-interceptor-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&home).unwrap();
        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &home);

        let result = (|| -> Result<(), String> {
            let db_path = home.join("state.db");
            let db = Database::new(db_path).map_err(|e| e.to_string())?;
            let manager = InterceptorManager::new(db, 16888);

            let claude_original = r#"{"env":{"ANTHROPIC_BASE_URL":"https://claude.example"},"theme":"dark"}"#;
            fs::write(home.join(".claude.json"), claude_original).map_err(|e| e.to_string())?;
            manager.toggle_claude_code(true)?;
            manager.toggle_claude_code(true)?;
            manager.toggle_claude_code(false)?;
            assert_eq!(
                fs::read_to_string(home.join(".claude.json")).map_err(|e| e.to_string())?,
                claude_original
            );

            let codex_dir = home.join(".codex");
            fs::create_dir_all(&codex_dir).map_err(|e| e.to_string())?;
            let codex_original = "model = \"gpt-4o\"\n\n";
            fs::write(codex_dir.join("config.toml"), codex_original)
                .map_err(|e| e.to_string())?;
            manager.toggle_codex(true)?;
            manager.toggle_codex(true)?;
            manager.toggle_codex(false)?;
            assert_eq!(
                fs::read_to_string(codex_dir.join("config.toml")).map_err(|e| e.to_string())?,
                codex_original
            );

            let gemini_dir = home.join(".gemini");
            fs::create_dir_all(&gemini_dir).map_err(|e| e.to_string())?;
            let gemini_original = "{\"theme\":\"dark\",\"baseUrl\":\"https://gemini.example\"}";
            fs::write(gemini_dir.join("config.json"), gemini_original)
                .map_err(|e| e.to_string())?;
            manager.toggle_gemini_cli(true)?;
            manager.toggle_gemini_cli(true)?;
            manager.toggle_gemini_cli(false)?;
            assert_eq!(
                fs::read_to_string(gemini_dir.join("config.json"))
                    .map_err(|e| e.to_string())?,
                gemini_original
            );
            Ok(())
        })();

        if let Some(home) = old_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
            let _ = fs::remove_dir_all(&home);
        result.unwrap();
    }

    #[test]
    fn test_update_codex_config_toml_modern_format() {
        let original = r#"model_provider = "custom"
model = "gpt-6-astra"

[model_providers.custom]
name = "OpenAI"
base_url = "https://sub2api.vonote.com"
requires_openai_auth = true
wire_api = "responses"

[desktop]
followUpQueueMode = "queue"
"#;
        let updated = update_codex_config_toml(
            original,
            "http://127.0.0.1:16888/v1",
            Some("omniswitch-model-catalog.json"),
            None,
        )
        .unwrap();
        assert!(updated.contains("http://127.0.0.1:16888/v1"));
        assert!(updated.contains("model_catalog_json = \"omniswitch-model-catalog.json\""));
        assert!(updated.contains("[desktop]"));
        assert!(updated.contains("followUpQueueMode = \"queue\""));
        assert!(!updated.contains("https://sub2api.vonote.com"));
    }

    #[test]
    fn test_generate_codex_catalog_json() {
        let provider1 = Provider {
            id: "p1".to_string(),
            name: "DeepSeek".to_string(),
            protocol: "openai".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            preset_id: None,
            api_keys: vec!["sk-test".to_string()],
            models: vec!["deepseek-v4-pro".to_string(), "deepseek-reasoner".to_string()],
            default_model: None,
            is_default: false,
            enabled: true,
            priority: 10,
            weight: 1,
            timeout_secs: 30,
            latency_ms: None,
            created_at: None,
            updated_at: None,
        };
        let provider2 = Provider {
            id: "p2".to_string(),
            name: "Work".to_string(),
            protocol: "openai".to_string(),
            base_url: "https://work.example.com".to_string(),
            preset_id: None,
            api_keys: vec!["sk-work".to_string()],
            models: vec!["gpt-6-astra".to_string()],
            default_model: Some("gpt-6-astra".to_string()),
            is_default: true,
            enabled: true,
            priority: 20,
            weight: 1,
            timeout_secs: 30,
            latency_ms: None,
            created_at: None,
            updated_at: None,
        };

        let catalog = generate_codex_catalog_json(&[provider1, provider2]);
        let models = catalog["models"].as_array().expect("models must be array");
        assert_eq!(models.len(), 3);
        assert_eq!(models[0]["slug"], "deepseek/deepseek-v4-pro");
        assert_eq!(models[0]["display_name"], "deepseek/deepseek-v4-pro");
        assert_eq!(models[1]["slug"], "deepseek/deepseek-reasoner");
        assert_eq!(models[1]["default_reasoning_level"], "high");
        assert_eq!(models[2]["slug"], "work/gpt-6-astra");
        assert_eq!(models[2]["display_name"], "work/gpt-6-astra");
        assert_eq!(models[2]["context_window"], 272000);
        assert_eq!(models[2]["max_context_window"], 872000);
        let levels = models[2]["supported_reasoning_levels"].as_array().unwrap();
        assert_eq!(levels.len(), 6);
        assert_eq!(levels[5]["effort"], "ultra");
    }

    #[test]
    fn test_resolve_model_reasoning_spec() {
        // Non-reasoning models
        let (levels, default_lvl) = resolve_model_reasoning_spec("gpt-4o");
        assert_eq!(default_lvl, "none");
        assert_eq!(levels.len(), 1);

        let (levels, default_lvl) = resolve_model_reasoning_spec("claude-3-5-sonnet-20241022");
        assert_eq!(default_lvl, "none");
        assert_eq!(levels.len(), 1);

        let (levels, default_lvl) = resolve_model_reasoning_spec("gemini-1.5-flash");
        assert_eq!(default_lvl, "none");
        assert_eq!(levels.len(), 1);

        let (levels, default_lvl) = resolve_model_reasoning_spec("deepseek-chat");
        assert_eq!(default_lvl, "none");
        assert_eq!(levels.len(), 1);

        let (levels, default_lvl) = resolve_model_reasoning_spec("gemini-3.1-flash-image");
        assert_eq!(default_lvl, "none");
        assert_eq!(levels.len(), 1);

        let (levels, default_lvl) = resolve_model_reasoning_spec("dall-e-3");
        assert_eq!(default_lvl, "none");
        assert_eq!(levels.len(), 1);

        // Reasoning models
        let (levels, default_lvl) = resolve_model_reasoning_spec("claude-3-7-sonnet-20250219");
        assert_eq!(default_lvl, "medium");
        assert!(levels.len() > 1);

        let (levels, default_lvl) = resolve_model_reasoning_spec("claude-opus-4-6-thinking");
        assert_eq!(default_lvl, "high");
        assert!(levels.len() > 1);

        let (levels, default_lvl) = resolve_model_reasoning_spec("deepseek-reasoner");
        assert_eq!(default_lvl, "high");
        assert!(levels.len() > 1);

        let (levels, default_lvl) = resolve_model_reasoning_spec("gemini-3.8-flash-high");
        assert_eq!(default_lvl, "medium");
        assert!(levels.len() > 1);

        let (levels, default_lvl) = resolve_model_reasoning_spec("gpt-5.6-sol");
        assert_eq!(default_lvl, "low");
        assert_eq!(levels.len(), 6);
        assert_eq!(levels[5]["effort"], "ultra");

        let (levels, default_lvl) = resolve_model_reasoning_spec("gpt-6-astra");
        assert_eq!(default_lvl, "medium");
        assert_eq!(levels.len(), 6);
        assert_eq!(levels[0]["effort"], "low");
        assert_eq!(levels[4]["effort"], "max");
        assert_eq!(levels[5]["effort"], "ultra");

        let (cw, mcw) = resolve_model_context_window("gpt-6-astra");
        assert_eq!(cw, 272000);
        assert_eq!(mcw, 872000);
    }

    #[test]
    fn test_update_gemini_env() {
        let original = "GEMINI_API_KEY=sk-test1234\nGOOGLE_GEMINI_BASE_URL=https://old.proxy.com\n";
        let updated = update_gemini_env(original, "http://127.0.0.1:16888");
        assert!(updated.contains("GOOGLE_GEMINI_BASE_URL=http://127.0.0.1:16888"));
        assert!(updated.contains("GEMINI_BASE_URL=http://127.0.0.1:16888"));
        assert!(updated.contains("GEMINI_API_KEY=sk-test1234"));
    }

    #[test]
    fn test_detect_third_party_endpoint_and_conflict() {
        let _lock = HOME_LOCK.lock().unwrap();
        let home = std::env::temp_dir().join(format!(
            "omniswitch-conflict-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&home).unwrap();
        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &home);

        let result = (|| -> Result<(), String> {
            let db_path = home.join("state.db");
            let db = Database::new(db_path).map_err(|e| e.to_string())?;
            let manager = InterceptorManager::new(db, 16888);

            // 1. Claude Code
            // 官方地址不应报警
            fs::write(
                home.join(".claude.json"),
                r#"{"anthropicBaseUrl":"https://api.anthropic.com"}"#,
            )
            .map_err(|e| e.to_string())?;
            assert_eq!(manager.detect_third_party_endpoint("claude_code"), None);
            assert_eq!(manager.check_third_party_conflict("claude_code", false), None);

            // 第三方代理地址应当被检出并给出提示
            fs::write(
                home.join(".claude.json"),
                r#"{"anthropicBaseUrl":"https://my-proxy.example.com"}"#,
            )
            .map_err(|e| e.to_string())?;
            assert_eq!(
                manager.detect_third_party_endpoint("claude_code"),
                Some("https://my-proxy.example.com".to_string())
            );
            assert!(manager
                .check_third_party_conflict("claude_code", false)
                .unwrap()
                .contains("https://my-proxy.example.com"));

            // 验证现代 ~/.claude/settings.json 中的第三方端点优先检出
            let claude_dir = home.join(".claude");
            fs::create_dir_all(&claude_dir).map_err(|e| e.to_string())?;
            fs::write(
                claude_dir.join("settings.json"),
                r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:8317"}}"#,
            )
            .map_err(|e| e.to_string())?;
            assert_eq!(
                manager.detect_third_party_endpoint("claude_code"),
                Some("http://127.0.0.1:8317".to_string())
            );
            assert!(manager
                .check_third_party_conflict("claude_code", false)
                .unwrap()
                .contains("http://127.0.0.1:8317"));
            // 验证争夺冲突提示 (is_overwritten = true)
            assert!(manager
                .check_third_party_conflict("claude_code", true)
                .unwrap()
                .contains("停止自动覆盖"));
            let claude_tp = manager.get_third_party_config("claude_code").unwrap();
            assert_eq!(claude_tp.endpoint, "http://127.0.0.1:8317");
            assert!(claude_tp.suggested_name.contains("本地代理"));
            assert_eq!(claude_tp.protocol, "anthropic");

            // 2. Codex
            let codex_dir = home.join(".codex");
            fs::create_dir_all(&codex_dir).map_err(|e| e.to_string())?;
            fs::write(
                codex_dir.join("config.toml"),
                "base_url = \"https://api.openai.com/v1\"\n",
            )
            .map_err(|e| e.to_string())?;
            assert_eq!(manager.detect_third_party_endpoint("codex_cli"), None);
            assert_eq!(manager.check_third_party_conflict("codex_cli", false), None);

            fs::write(
                codex_dir.join("config.toml"),
                "base_url = \"https://sub2api.vonote.com\"\n",
            )
            .map_err(|e| e.to_string())?;
            assert_eq!(
                manager.detect_third_party_endpoint("codex_cli"),
                Some("https://sub2api.vonote.com".to_string())
            );
            assert!(manager
                .check_third_party_conflict("codex_cli", false)
                .unwrap()
                .contains("https://sub2api.vonote.com"));
            let codex_tp = manager.get_third_party_config("codex_cli").unwrap();
            assert_eq!(codex_tp.endpoint, "https://sub2api.vonote.com");
            assert_eq!(codex_tp.protocol, "openai");

            // 3. Gemini CLI
            let gemini_dir = home.join(".gemini");
            fs::create_dir_all(&gemini_dir).map_err(|e| e.to_string())?;
            fs::write(
                gemini_dir.join(".env"),
                "GOOGLE_GEMINI_BASE_URL=\"https://gemini-proxy.com\"\n",
            )
            .map_err(|e| e.to_string())?;
            assert_eq!(
                manager.detect_third_party_endpoint("gemini_cli"),
                Some("https://gemini-proxy.com".to_string())
            );
            assert!(manager
                .check_third_party_conflict("gemini_cli", false)
                .unwrap()
                .contains("https://gemini-proxy.com"));
            let gemini_tp = manager.get_third_party_config("gemini_cli").unwrap();
            assert_eq!(gemini_tp.endpoint, "https://gemini-proxy.com");
            assert_eq!(gemini_tp.protocol, "gemini");

            Ok(())
        })();

        if let Some(h) = old_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
        let _ = fs::remove_dir_all(&home);
        result.unwrap();
    }
}

