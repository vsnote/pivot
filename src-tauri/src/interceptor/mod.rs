use crate::db::{Database, Provider, RouteRule};
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
            self.get_pi_status(),
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
            "pi" => {
                let models_path = Self::pi_models_path();
                if !models_path.exists() {
                    return None;
                }
                let content = fs::read_to_string(&models_path).ok()?;
                let val = serde_json::from_str::<Value>(&content).ok()?;
                let providers_obj = val.get("providers").and_then(|p| p.as_object())?;

                for (p_id, p_val) in providers_obj {
                    if p_id == "pivot" {
                        continue;
                    }
                    let ep = p_val.get("baseUrl").and_then(|u| u.as_str())?;
                    let clean_ep = ep.trim().trim_end_matches('/').to_string();
                    if clean_ep.is_empty() || clean_ep.contains(&self.gateway_url) {
                        continue;
                    }

                    let token = p_val
                        .get("apiKey")
                        .and_then(|k| k.as_str())
                        .map(|s| s.to_string());
                    let api = p_val
                        .get("api")
                        .and_then(|a| a.as_str())
                        .unwrap_or("openai-completions");
                    let protocol = if api == "anthropic-messages" {
                        "anthropic".to_string()
                    } else {
                        "openai".to_string()
                    };

                    let host = if clean_ep.contains("127.0.0.1") || clean_ep.contains("localhost") {
                        let port = clean_ep.rsplit(':').next().unwrap_or("代理");
                        format!("本地代理 ({})", port)
                    } else {
                        let h = clean_ep
                            .trim_start_matches("http://")
                            .trim_start_matches("https://")
                            .split('/')
                            .next()
                            .unwrap_or(p_id);
                        format!("Pi 渠道 ({})", h)
                    };

                    let mut models = Vec::new();
                    if let Some(m_arr) = p_val.get("models").and_then(|m| m.as_array()) {
                        for item in m_arr {
                            if let Some(mid) = item.get("id").and_then(|i| i.as_str()) {
                                models.push(mid.to_string());
                            }
                        }
                    }
                    if models.is_empty() {
                        models = vec!["gpt-4o".to_string(), "claude-3-7-sonnet".to_string()];
                    }

                    return Some(ThirdPartyConfig {
                        endpoint: clean_ep,
                        auth_token: token,
                        suggested_name: host,
                        protocol,
                        models,
                    });
                }
                None
            }
            _ => None,
        }
    }

    /// Extracts the current custom endpoint from the tool's configuration if it points
    /// to a third-party service (i.e. neither official upstream nor this Pivot gateway).
    fn detect_third_party_endpoint(&self, tool_id: &str) -> Option<String> {
        self.get_third_party_config(tool_id).map(|tp| tp.endpoint)
    }

    /// Generate user-facing conflict warning message if a third-party endpoint is detected.
    fn check_third_party_conflict(&self, tool_id: &str, is_overwritten: bool) -> Option<String> {
        let ep = self.detect_third_party_endpoint(tool_id)?;
        if is_overwritten {
            Some(format!(
                "检测到当前配置已被第三方工具覆盖（{}）！为避免持续争夺权限，Pivot 已停止自动覆盖。建议在外部工具中关闭自动同步，然后重新接管或导入为上游服务商。",
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
        let base = Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude");
        let new_bak = base.join("settings.json.pivot.bak");
        let legacy_bak = base.join("settings.json.omniswitch.bak");
        if !new_bak.exists() && legacy_bak.exists() {
            legacy_bak
        } else {
            new_bak
        }
    }

    fn claude_config_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude.json")
    }

    fn claude_backup_path() -> PathBuf {
        let base = Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."));
        let new_bak = base.join(".claude.json.pivot.bak");
        let legacy_bak = base.join(".claude.json.omniswitch.bak");
        if !new_bak.exists() && legacy_bak.exists() {
            legacy_bak
        } else {
            new_bak
        }
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

        if is_intercepted {
            let missing_picker = settings_path.exists()
                && fs::read_to_string(&settings_path)
                    .map(|c| !c.contains("replaceBuiltInOptions"))
                    .unwrap_or(false);
            if missing_picker {
                let _ = self.do_sync_claude_settings();
            }
        }

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
            // 获取之前已保存的 orig_ep，或提取当前配置的第三方端点
            let saved_state = self.db.get_interceptor_state("claude_code").ok().flatten();
            let orig_ep = saved_state
                .and_then(|s| s.2)
                .or_else(|| self.detect_third_party_endpoint("claude_code"));

            // 1. 接管 ~/.claude/settings.json (现代 Claude Code 全局官方环境变量配置)
            let mut orig_settings = String::new();
            if settings_path.exists() {
                orig_settings = fs::read_to_string(&settings_path).unwrap_or_default();
            } else if let Some(p) = settings_path.parent() {
                let _ = fs::create_dir_all(p);
            }

            let mut settings_val: Value =
                serde_json::from_str(&orig_settings).unwrap_or_else(|_| json!({}));
            if !settings_val.is_object() {
                settings_val = json!({});
            }
            if !settings_val["env"].is_object() {
                settings_val["env"] = json!({});
            }
            // 仅对 ANTHROPIC_BASE_URL 进行打补丁，其余 permissions / hooks / model / env 完整保留
            settings_val["env"]["ANTHROPIC_BASE_URL"] = json!(self.gateway_url);

            // 确保有有效 token，防止 Claude Code 抛出登录弹窗
            if settings_val["env"]
                .get("ANTHROPIC_AUTH_TOKEN")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .is_empty()
            {
                settings_val["env"]["ANTHROPIC_AUTH_TOKEN"] = json!("pivot");
            }

            // 清理第三方代理遗留的默认模型重定向环境变量，使 Claude Code 的请求正常流转到 Pivot 的 Agent 槽位与动态路由
            if let Some(env_obj) = settings_val["env"].as_object_mut() {
                env_obj.remove("ANTHROPIC_DEFAULT_HAIKU_MODEL");
                env_obj.remove("ANTHROPIC_DEFAULT_OPUS_MODEL");
                env_obj.remove("ANTHROPIC_DEFAULT_SONNET_MODEL");
                env_obj.remove("ANTHROPIC_SMALL_FAST_MODEL");
            }

            // 同步生成并注入 modelPicker，使得 Claude Code 的 /model 能够原生列出所有启用的服务商模型
            if let Ok(providers) = self.db.get_providers() {
                let routes = self.db.get_routes().unwrap_or_default();
                let picker = generate_claude_model_picker(&providers, &routes);
                settings_val["modelPicker"] = picker;
            }

            if let Ok(new_settings) = serde_json::to_string_pretty(&settings_val) {
                let _ = atomic_write(&settings_path, &new_settings);
            }

            // 2. 同步微创接管 ~/.claude.json (如果文件存在)
            if config_path.exists() {
                let orig_config = fs::read_to_string(&config_path).unwrap_or_default();
                if let Ok(mut config_val) = serde_json::from_str::<Value>(&orig_config) {
                    config_val["anthropicBaseUrl"] = json!(self.gateway_url);
                    if let Some(env_obj) = config_val.get_mut("env").and_then(|e| e.as_object_mut()) {
                        env_obj.insert("ANTHROPIC_BASE_URL".to_string(), json!(self.gateway_url));
                    }
                    if let Ok(new_config) = serde_json::to_string_pretty(&config_val) {
                        let _ = atomic_write(&config_path, &new_config);
                    }
                }
            }

            // 清理旧版全量备份文件，避免日后污染
            let _ = fs::remove_file(&settings_backup_path);
            let _ = fs::remove_file(&backup_path);

            let _ = self.db.save_interceptor_state(
                "claude_code",
                true,
                None,
                orig_ep.as_deref(),
            );
        } else {
            let orig_ep = self
                .db
                .get_interceptor_state("claude_code")
                .ok()
                .flatten()
                .and_then(|s| s.2);

            // 方案 B：微创增量还原 ~/.claude/settings.json
            // 重新读取磁盘当前最新内容，绝不覆盖回滚用户在接管期间新授权的命令、hooks 或修改的配置
            if settings_path.exists() {
                if let Ok(content) = fs::read_to_string(&settings_path) {
                    if let Ok(mut v) = serde_json::from_str::<Value>(&content) {
                        if let Some(obj) = v.as_object_mut() {
                            obj.remove("anthropicBaseUrl");
                            obj.remove("modelPicker");
                            if let Some(env_obj) =
                                obj.get_mut("env").and_then(|e| e.as_object_mut())
                            {
                                if let Some(ref ep) = orig_ep.as_deref().filter(|s| !s.is_empty() && !s.contains(&self.gateway_url)) {
                                    env_obj.insert("ANTHROPIC_BASE_URL".to_string(), json!(ep));
                                } else {
                                    env_obj.remove("ANTHROPIC_BASE_URL");
                                }
                            }
                        }
                        if let Ok(updated) = serde_json::to_string_pretty(&v) {
                            let _ = atomic_write(&settings_path, &updated);
                        }
                    }
                }
            }

            // 同样微创还原 ~/.claude.json
            if config_path.exists() {
                if let Ok(content) = fs::read_to_string(&config_path) {
                    if let Ok(mut v) = serde_json::from_str::<Value>(&content) {
                        if let Some(obj) = v.as_object_mut() {
                            let has_env = obj.get("env").and_then(|e| e.as_object()).is_some();
                            if !has_env {
                                if let Some(ref ep) = orig_ep.as_deref().filter(|s| !s.is_empty() && !s.contains(&self.gateway_url)) {
                                    obj.insert("anthropicBaseUrl".to_string(), json!(ep));
                                } else {
                                    obj.remove("anthropicBaseUrl");
                                }
                            } else {
                                obj.remove("anthropicBaseUrl");
                            }

                            if let Some(env_obj) =
                                obj.get_mut("env").and_then(|e| e.as_object_mut())
                            {
                                if let Some(ref ep) = orig_ep.as_deref().filter(|s| !s.is_empty() && !s.contains(&self.gateway_url)) {
                                    env_obj.insert("ANTHROPIC_BASE_URL".to_string(), json!(ep));
                                } else {
                                    env_obj.remove("ANTHROPIC_BASE_URL");
                                }
                            }
                        }
                        if let Ok(updated) = serde_json::to_string_pretty(&v) {
                            let _ = atomic_write(&config_path, &updated);
                        }
                    }
                }
            }

            let _ = fs::remove_file(&settings_backup_path);
            let _ = fs::remove_file(&backup_path);

            let _ = self
                .db
                .save_interceptor_state("claude_code", false, None, None);
        }

        Ok(self.get_claude_code_status())
    }

    fn do_sync_claude_settings(&self) -> Result<(), String> {
        let settings_path = Self::claude_settings_path();
        if !settings_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&settings_path).unwrap_or_default();
        let mut v: Value = serde_json::from_str(&content).unwrap_or_else(|_| json!({}));
        if !v.is_object() {
            return Ok(());
        }
        let providers = self.db.get_providers().map_err(|e| e.to_string())?;
        let routes = self.db.get_routes().unwrap_or_default();
        v["modelPicker"] = generate_claude_model_picker(&providers, &routes);
        if let Some(env_obj) = v.get_mut("env").and_then(|e| e.as_object_mut()) {
            env_obj.remove("ANTHROPIC_DEFAULT_HAIKU_MODEL");
            env_obj.remove("ANTHROPIC_DEFAULT_OPUS_MODEL");
            env_obj.remove("ANTHROPIC_DEFAULT_SONNET_MODEL");
            env_obj.remove("ANTHROPIC_SMALL_FAST_MODEL");
        }
        if let Ok(updated) = serde_json::to_string_pretty(&v) {
            let _ = atomic_write(&settings_path, &updated);
        }
        Ok(())
    }

    pub fn sync_claude_settings_if_intercepted(&self) -> Result<(), String> {
        let status = self.get_claude_code_status();
        if status.is_intercepted {
            self.do_sync_claude_settings()?;
        }
        Ok(())
    }

    // ================= Codex CLI =================

    fn codex_config_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".codex")
            .join("config.toml")
    }

    fn codex_backup_path() -> PathBuf {
        let base = Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".codex");
        let new_bak = base.join("config.toml.pivot.bak");
        let legacy_bak = base.join("config.toml.omniswitch.bak");
        if !new_bak.exists() && legacy_bak.exists() {
            legacy_bak
        } else {
            new_bak
        }
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

    pub const CODEX_CATALOG_FILENAME: &'static str = "pivot-model-catalog.json";

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
            let saved_state = self.db.get_interceptor_state("codex_cli").ok().flatten();
            let orig_ep = saved_state
                .and_then(|s| s.2)
                .or_else(|| self.detect_third_party_endpoint("codex_cli"));

            let mut original_content = String::new();
            if config_path.exists() {
                original_content = fs::read_to_string(&config_path).unwrap_or_default();
            } else if let Some(p) = config_path.parent() {
                let _ = fs::create_dir_all(p);
            }

            // 生成并写入 catalog 文件
            let providers = self.db.get_providers().unwrap_or_default();
            let catalog = generate_codex_catalog_json(&providers);
            if let Ok(catalog_str) = serde_json::to_string_pretty(&catalog) {
                let _ = atomic_write(&catalog_path, &catalog_str);
            }

            let model_provider_count = count_model_providers(&providers);
            let preferred_model = providers
                .iter()
                .find(|p| p.is_default && p.enabled)
                .and_then(|p| {
                    let prefix = p.name.to_lowercase().replace(' ', "-");
                    let m = p.default_model.as_deref().or_else(|| p.models.first().map(|s| s.as_str()))?;
                    let is_duplicate = model_provider_count.get(m).copied().unwrap_or(0) > 1;
                    Some(if m.to_lowercase().starts_with(&format!("{}/", prefix)) {
                        m.to_string()
                    } else if is_duplicate {
                        format!("{}/{}", prefix, m)
                    } else {
                        m.to_string()
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

            let _ = fs::remove_file(&backup_path);
            let _ = self
                .db
                .save_interceptor_state("codex_cli", true, None, orig_ep.as_deref());
        } else {
            let orig_ep = self
                .db
                .get_interceptor_state("codex_cli")
                .ok()
                .flatten()
                .and_then(|s| s.2);

            // 方案 B：微创增量还原 ~/.codex/config.toml
            if config_path.exists() {
                if let Ok(content) = fs::read_to_string(&config_path) {
                    if let Ok(cleaned) = restore_codex_config_toml(
                        &content,
                        orig_ep.as_deref(),
                        Self::CODEX_CATALOG_FILENAME,
                    ) {
                        let _ = atomic_write(&config_path, &cleaned);
                    }
                }
            }

            let _ = fs::remove_file(&backup_path);
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
        let base = Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gemini");
        let new_bak = base.join("config.json.pivot.bak");
        let legacy_bak = base.join("config.json.omniswitch.bak");
        if !new_bak.exists() && legacy_bak.exists() {
            legacy_bak
        } else {
            new_bak
        }
    }

    fn gemini_env_path() -> PathBuf {
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gemini")
            .join(".env")
    }

    fn gemini_env_backup_path() -> PathBuf {
        let base = Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gemini");
        let new_bak = base.join(".env.pivot.bak");
        let legacy_bak = base.join(".env.omniswitch.bak");
        if !new_bak.exists() && legacy_bak.exists() {
            legacy_bak
        } else {
            new_bak
        }
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
            let saved_state = self.db.get_interceptor_state("gemini_cli").ok().flatten();
            let orig_ep = saved_state
                .and_then(|s| s.2)
                .or_else(|| self.detect_third_party_endpoint("gemini_cli"));

            // 1. 接管 ~/.gemini/.env (Gemini CLI 官方真实环境变量文件)
            let mut orig_env = String::new();
            if env_path.exists() {
                orig_env = fs::read_to_string(&env_path).unwrap_or_default();
            } else if let Some(p) = env_path.parent() {
                let _ = fs::create_dir_all(p);
            }
            let new_env = update_gemini_env(&orig_env, &self.gateway_url);
            let _ = atomic_write(&env_path, &new_env);

            // 2. 同时维护 config.json (兼容旧配置和测试用例)
            let mut parsed: Value = json!({});
            if config_path.exists() {
                let original_content = fs::read_to_string(&config_path).unwrap_or_default();
                if let Ok(v) = serde_json::from_str::<Value>(&original_content) {
                    parsed = v;
                }
            } else if let Some(p) = config_path.parent() {
                let _ = fs::create_dir_all(p);
            }
            parsed["baseUrl"] = json!(self.gateway_url);
            if let Ok(new_content) = serde_json::to_string_pretty(&parsed) {
                let _ = atomic_write(&config_path, &new_content);
            }

            let _ = fs::remove_file(&env_backup_path);
            let _ = fs::remove_file(&backup_path);

            let _ = self
                .db
                .save_interceptor_state("gemini_cli", true, None, orig_ep.as_deref());
        } else {
            let orig_ep = self
                .db
                .get_interceptor_state("gemini_cli")
                .ok()
                .flatten()
                .and_then(|s| s.2);

            // 方案 B：微创增量还原 ~/.gemini/.env
            if env_path.exists() {
                if let Ok(content) = fs::read_to_string(&env_path) {
                    let cleaned = restore_gemini_env(&content, orig_ep.as_deref(), &self.gateway_url);
                    let _ = atomic_write(&env_path, &cleaned);
                }
            }

            // 恢复 config.json
            if config_path.exists() {
                if let Ok(content) = fs::read_to_string(&config_path) {
                    if let Ok(mut v) = serde_json::from_str::<Value>(&content) {
                        if let Some(obj) = v.as_object_mut() {
                            if let Some(ref ep) = orig_ep.as_deref().filter(|s| !s.is_empty() && !s.contains(&self.gateway_url)) {
                                obj.insert("baseUrl".to_string(), json!(ep));
                            } else {
                                obj.remove("baseUrl");
                            }
                        }
                        if let Ok(updated) = serde_json::to_string_pretty(&v) {
                            let _ = atomic_write(&config_path, &updated);
                        }
                    }
                }
            }

            let _ = fs::remove_file(&env_backup_path);
            let _ = fs::remove_file(&backup_path);

            let _ = self
                .db
                .save_interceptor_state("gemini_cli", false, None, None);
        }

        Ok(self.get_gemini_cli_status())
    }

    // ================= Pi Agent =================

    fn pi_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
            let p = PathBuf::from(dir);
            if !p.as_os_str().is_empty() {
                return p;
            }
        }
        Self::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".pi")
            .join("agent")
    }

    fn pi_models_path() -> PathBuf {
        Self::pi_dir().join("models.json")
    }

    fn pi_models_backup_path() -> PathBuf {
        let base = Self::pi_dir();
        let new_bak = base.join("models.json.pivot.bak");
        let legacy_bak = base.join("models.json.omniswitch.bak");
        if !new_bak.exists() && legacy_bak.exists() {
            legacy_bak
        } else {
            new_bak
        }
    }

    fn pi_settings_path() -> PathBuf {
        Self::pi_dir().join("settings.json")
    }

    pub fn get_pi_status(&self) -> InterceptorInfo {
        let models_path = Self::pi_models_path();
        let models_backup_path = Self::pi_models_backup_path();
        let settings_path = Self::pi_settings_path();

        let installed = Self::is_binary_installed("pi")
            || models_path.exists()
            || settings_path.exists()
            || Self::pi_dir().exists()
            || Self::home_dir()
                .map(|h| h.join(".pi").exists())
                .unwrap_or(false);

        let mut db_is_intercepted = false;
        let mut original_endpoint = None;

        if let Ok(Some((db_intercepted, _, orig_ep))) = self.db.get_interceptor_state("pi") {
            db_is_intercepted = db_intercepted;
            original_endpoint = orig_ep;
        }

        let mut file_intercepted = false;
        if models_path.exists() {
            if let Ok(content) = fs::read_to_string(&models_path) {
                if content.contains(&self.gateway_url) {
                    file_intercepted = true;
                }
            }
        }

        let is_intercepted = if models_path.exists() {
            file_intercepted
        } else {
            db_is_intercepted
        };

        let is_overwritten = db_is_intercepted && !file_intercepted;

        if is_intercepted {
            let _ = self.do_sync_pi_models();
        }

        let third_party = self.get_third_party_config("pi");
        let original_endpoint = third_party.as_ref().map(|tp| tp.endpoint.clone()).or(original_endpoint);

        InterceptorInfo {
            id: "pi".to_string(),
            name: "Pi Agent".to_string(),
            installed,
            config_path: models_path.to_string_lossy().to_string(),
            is_intercepted,
            backup_path: if models_backup_path.exists() {
                Some(models_backup_path.to_string_lossy().to_string())
            } else {
                None
            },
            original_endpoint,
            conflict: self.check_third_party_conflict("pi", is_overwritten),
            third_party,
            is_overwritten,
        }
    }

    fn do_sync_pi_models(&self) -> Result<(), String> {
        let models_path = Self::pi_models_path();
        if !models_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&models_path).unwrap_or_default();
        let mut v: Value = serde_json::from_str(&content).unwrap_or_else(|_| json!({}));
        if !v.is_object() || !v["providers"].is_object() {
            return Ok(());
        }
        if v["providers"].get("pivot").is_none() {
            return Ok(());
        }

        let providers = self.db.get_providers().map_err(|e| e.to_string())?;
        let routes = self.db.get_routes().unwrap_or_default();
        let (pi_models, preferred_model) = generate_pi_models_list(&providers, &routes);

        let target_endpoint = format!("{}/v1", self.gateway_url);
        v["providers"]["pivot"] = json!({
            "baseUrl": target_endpoint,
            "api": "openai-completions",
            "apiKey": "pivot",
            "models": pi_models,
        });

        if let Ok(serialized) = serde_json::to_string_pretty(&v) {
            let _ = atomic_write(&models_path, &serialized);
        }

        let settings_path = Self::pi_settings_path();
        if settings_path.exists() {
            if let Ok(content) = fs::read_to_string(&settings_path) {
                if let Ok(mut sv) = serde_json::from_str::<Value>(&content) {
                    if sv.get("defaultProvider").and_then(|p| p.as_str()) == Some("pivot") {
                        if let Some(ref pm) = preferred_model {
                            sv["defaultModel"] = json!(pm);
                            if let Ok(serialized) = serde_json::to_string_pretty(&sv) {
                                let _ = atomic_write(&settings_path, &serialized);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn sync_pi_models_if_intercepted(&self) -> Result<(), String> {
        let status = self.get_pi_status();
        if status.is_intercepted {
            self.do_sync_pi_models()?;
        }
        Ok(())
    }

    pub fn toggle_pi(&self, enable: bool) -> Result<InterceptorInfo, String> {
        let models_path = Self::pi_models_path();
        let models_backup_path = Self::pi_models_backup_path();
        let settings_path = Self::pi_settings_path();
        let target_endpoint = format!("{}/v1", self.gateway_url);

        if enable {
            let saved_state = self.db.get_interceptor_state("pi").ok().flatten();
            let orig_ep = saved_state
                .as_ref()
                .and_then(|s| s.2.clone())
                .or_else(|| self.detect_third_party_endpoint("pi"));

            if let Some(p) = models_path.parent() {
                let _ = fs::create_dir_all(p);
            }

            // 1. 读取或初始化 models.json
            let mut models_val: Value = if models_path.exists() {
                let content = fs::read_to_string(&models_path).unwrap_or_default();
                serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
            } else {
                json!({})
            };
            if !models_val.is_object() {
                models_val = json!({});
            }
            if !models_val["providers"].is_object() {
                models_val["providers"] = json!({});
            }

            // 2. 生成 pivot provider 的 models 列表
            let providers = self.db.get_providers().unwrap_or_default();
            let routes = self.db.get_routes().unwrap_or_default();
            let (pi_models, preferred_model) = generate_pi_models_list(&providers, &routes);

            models_val["providers"]["pivot"] = json!({
                "baseUrl": target_endpoint,
                "api": "openai-completions",
                "apiKey": "pivot",
                "models": pi_models,
            });

            if let Ok(serialized) = serde_json::to_string_pretty(&models_val) {
                atomic_write(&models_path, &serialized).map_err(|e| e.to_string())?;
            }

            // 3. 更新 settings.json 的 defaultProvider 和 defaultModel
            let prev_backup = saved_state
                .as_ref()
                .and_then(|s| s.1.as_deref())
                .and_then(|b| serde_json::from_str::<Value>(b).ok());

            let mut orig_default_provider = prev_backup
                .as_ref()
                .and_then(|b| b.get("orig_default_provider"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let mut orig_default_model = prev_backup
                .as_ref()
                .and_then(|b| b.get("orig_default_model"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut settings_val: Value = if settings_path.exists() {
                let content = fs::read_to_string(&settings_path).unwrap_or_default();
                serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
            } else {
                json!({})
            };
            if !settings_val.is_object() {
                settings_val = json!({});
            }
            if orig_default_provider.is_none() {
                if let Some(dp) = settings_val.get("defaultProvider").and_then(|v| v.as_str()) {
                    if dp != "pivot" {
                        orig_default_provider = Some(dp.to_string());
                    }
                }
            }
            if orig_default_model.is_none() {
                if let Some(dm) = settings_val.get("defaultModel").and_then(|v| v.as_str()) {
                    orig_default_model = Some(dm.to_string());
                }
            }

            settings_val["defaultProvider"] = json!("pivot");
            if let Some(ref pm) = preferred_model {
                settings_val["defaultModel"] = json!(pm);
            }
            if let Ok(serialized) = serde_json::to_string_pretty(&settings_val) {
                let _ = atomic_write(&settings_path, &serialized);
            }

            let _ = fs::remove_file(&models_backup_path);

            let backup_info = json!({
                "orig_default_provider": orig_default_provider,
                "orig_default_model": orig_default_model,
            });
            let _ = self.db.save_interceptor_state(
                "pi",
                true,
                Some(&backup_info.to_string()),
                orig_ep.as_deref(),
            );
        } else {
            let saved_state = self.db.get_interceptor_state("pi").ok().flatten();
            let orig_ep = saved_state.as_ref().and_then(|s| s.2.clone());
            let backup_val = saved_state
                .as_ref()
                .and_then(|s| s.1.as_deref())
                .and_then(|b| serde_json::from_str::<Value>(b).ok());

            // 1. 还原 models.json
            if models_path.exists() {
                if let Ok(content) = fs::read_to_string(&models_path) {
                    if let Ok(mut v) = serde_json::from_str::<Value>(&content) {
                        if let Some(providers_obj) = v.get_mut("providers").and_then(|p| p.as_object_mut()) {
                            providers_obj.remove("pivot");
                        }
                        let remaining_keys = v.as_object().map(|o| o.keys().count()).unwrap_or(0);
                        let providers_empty = v.get("providers")
                            .and_then(|p| p.as_object())
                            .map(|p| p.is_empty())
                            .unwrap_or(false);

                        if remaining_keys == 1 && providers_empty {
                            let _ = fs::remove_file(&models_path);
                        } else if let Ok(serialized) = serde_json::to_string_pretty(&v) {
                            let _ = atomic_write(&models_path, &serialized);
                        }
                    }
                }
            }

            // 2. 还原 settings.json
            if settings_path.exists() {
                if let Ok(content) = fs::read_to_string(&settings_path) {
                    if let Ok(mut v) = serde_json::from_str::<Value>(&content) {
                        if v.get("defaultProvider").and_then(|p| p.as_str()) == Some("pivot") {
                            if let Some(ref b) = backup_val {
                                if let Some(orig_p) = b.get("orig_default_provider").and_then(|p| p.as_str()) {
                                    v["defaultProvider"] = json!(orig_p);
                                } else if let Some(obj) = v.as_object_mut() {
                                    obj.remove("defaultProvider");
                                }
                                if let Some(orig_m) = b.get("orig_default_model").and_then(|m| m.as_str()) {
                                    v["defaultModel"] = json!(orig_m);
                                } else if let Some(obj) = v.as_object_mut() {
                                    obj.remove("defaultModel");
                                }
                            } else if let Some(obj) = v.as_object_mut() {
                                obj.remove("defaultProvider");
                                obj.remove("defaultModel");
                            }
                        }
                        if let Ok(serialized) = serde_json::to_string_pretty(&v) {
                            let _ = atomic_write(&settings_path, &serialized);
                        }
                    }
                }
            }

            let _ = fs::remove_file(&models_backup_path);
            let _ = self.db.save_interceptor_state("pi", false, None, orig_ep.as_deref());
        }

        Ok(self.get_pi_status())
    }

    pub fn toggle_tool(&self, id: &str, enable: bool) -> Result<InterceptorInfo, String> {
        match id {
            "claude_code" => self.toggle_claude_code(enable),
            "codex_cli" => self.toggle_codex(enable),
            "gemini_cli" => self.toggle_gemini_cli(enable),
            "pi" => self.toggle_pi(enable),
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
            for (p_key, p_val) in providers_map.iter_mut() {
                if let toml::Value::Table(ref mut p_tbl) = p_val {
                    if p_key == &current_provider {
                        p_tbl.insert(
                            "base_url".to_string(),
                            toml::Value::String(target_endpoint.to_string()),
                        );
                        p_tbl.insert(
                            "supports_websockets".to_string(),
                            toml::Value::Boolean(false),
                        );
                    } else if let Some(bu) = p_tbl.get("base_url").and_then(|v| v.as_str()) {
                        if bu.contains("127.0.0.1:16888") || bu.contains("localhost:16888") {
                            p_tbl.insert(
                                "supports_websockets".to_string(),
                                toml::Value::Boolean(false),
                            );
                        }
                    }
                }
            }

            if !providers_map.contains_key(&current_provider) {
                let mut p = toml::map::Map::new();
                p.insert("name".to_string(), toml::Value::String("Pivot".to_string()));
                p.insert("base_url".to_string(), toml::Value::String(target_endpoint.to_string()));
                p.insert("wire_api".to_string(), toml::Value::String("responses".to_string()));
                p.insert("requires_openai_auth".to_string(), toml::Value::Boolean(true));
                p.insert("supports_websockets".to_string(), toml::Value::Boolean(false));
                providers_map.insert(current_provider, toml::Value::Table(p));
            }
        }
    }

    toml::to_string_pretty(&doc).map_err(|e| format!("序列化 Codex config.toml 失败: {}", e))
}

/// Restore Codex config.toml while preserving all other user modifications
fn restore_codex_config_toml(
    content: &str,
    original_endpoint: Option<&str>,
    catalog_filename: &str,
) -> Result<String, String> {
    if content.trim().is_empty() {
        return Ok(String::new());
    }
    let mut doc: toml::Value =
        toml::from_str(content).map_err(|e| format!("解析 Codex config.toml 失败: {}", e))?;

    if let toml::Value::Table(ref mut map) = doc {
        // 1. 移除注入的 model_catalog_json
        if let Some(cat) = map.get("model_catalog_json").and_then(|v| v.as_str()) {
            if cat == catalog_filename {
                map.remove("model_catalog_json");
            }
        }

        // 2. 还原或移除 base_url
        if let Some(ep) = original_endpoint.filter(|s| !s.trim().is_empty()) {
            map.insert("base_url".to_string(), toml::Value::String(ep.to_string()));
        } else {
            map.remove("base_url");
        }

        // 3. 还原 model_providers
        let current_provider = map
            .get("model_provider")
            .and_then(|v| v.as_str())
            .unwrap_or("custom")
            .to_string();

        if let Some(toml::Value::Table(ref mut providers_map)) = map.get_mut("model_providers") {
            let mut remove_provider = false;
            if let Some(toml::Value::Table(ref mut p_map)) = providers_map.get_mut(&current_provider) {
                if let Some(ep) = original_endpoint.filter(|s| !s.trim().is_empty()) {
                    p_map.insert("base_url".to_string(), toml::Value::String(ep.to_string()));
                } else {
                    p_map.remove("base_url");
                }
                // 如果这是 Pivot 自动创建的临时 custom provider 且没有 base_url，清理它
                if p_map.get("name").and_then(|v| v.as_str()) == Some("Pivot") {
                    remove_provider = true;
                }
            }
            if remove_provider {
                providers_map.remove(&current_provider);
            }
            if providers_map.is_empty() {
                map.remove("model_providers");
                if map.get("model_provider").and_then(|v| v.as_str()) == Some("custom") {
                    map.remove("model_provider");
                }
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
    // 目录命中优先：models.dev 的上下文与输出上限比名称猜测准确
    if let Some(m) = crate::model_catalog::lookup(model_name, None) {
        if let Some(ctx) = m.context_window {
            return (ctx, m.max_output.unwrap_or(ctx));
        }
    }

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

/// 按模型名推断可接受的输入模态。
/// 默认只声明 text：漏标多模态的代价只是用户少贴一张图，而误标成 image
/// 会让不支持图片的上游直接返回 400，所以只有明确识别为视觉模型时才放开。
pub fn resolve_model_input_modalities(model_name: &str) -> Vec<&'static str> {
    let lower = model_name.to_lowercase();
    let name = lower.split('/').last().unwrap_or(&lower);

    // 图像生成 / 嵌入 / 语音等辅助模型不接收图片输入。
    // 这条判断先于目录：目录把生图模型的 text+image 理解为「输出/处理」而非对话输入
    let is_aux = name.contains("image")
        || name.contains("imagen")
        || name.contains("dall-e")
        || name.contains("flux")
        || name.contains("embed")
        || name.contains("tts")
        || name.contains("whisper")
        || name.contains("audio")
        || name.contains("realtime");
    if is_aux {
        return vec!["text"];
    }

    // 目录命中优先，未命中或目录没写模态时回落到名称启发式
    if let Some(m) = crate::model_catalog::lookup(model_name, None) {
        let mods = m.client_input_modalities();
        if !mods.is_empty() {
            return mods;
        }
    }

    let is_vision = name.starts_with("gpt-4o")
        || name.starts_with("gpt-4.1")
        || name.starts_with("gpt-4-turbo")
        || name.starts_with("gpt-5")
        || name.starts_with("gpt-6")
        || name.starts_with("chatgpt-4o")
        || name.starts_with("o1")
        || name.starts_with("o3")
        || name.starts_with("o4")
        || name.starts_with("claude-3")
        || name.starts_with("claude-4")
        || name.starts_with("claude-sonnet")
        || name.starts_with("claude-opus")
        || name.starts_with("claude-haiku")
        || (name.starts_with("gemini")
            && !name.starts_with("gemini-1.0")
            && !name.starts_with("gemini-pro"))
        || name.starts_with("grok-4")
        || name.contains("-vl")
        || name.contains("vision")
        || name.contains("llava")
        || name.contains("pixtral")
        || name.contains("internvl")
        || name.contains("glm-4v")
        || name.contains("glm-4.1v")
        || name.contains("step-1v")
        || name.contains("llama-4");

    if is_vision {
        vec!["text", "image"]
    } else {
        vec!["text"]
    }
}

/// 获取服务商已声明的全部模型（包含 default_model 并去重）
fn provider_declared_models(provider: &Provider) -> Vec<String> {
    let mut models = Vec::new();
    if let Some(ref dm) = provider.default_model {
        let dm_trimmed = dm.trim();
        if !dm_trimmed.is_empty() {
            models.push(dm_trimmed.to_string());
        }
    }
    for m in &provider.models {
        let trimmed = m.trim().to_string();
        if !trimmed.is_empty() && !models.contains(&trimmed) {
            models.push(trimmed);
        }
    }
    models
}

/// 统计每个模型在已启用的服务商中被多少个服务商所声明，用于同名冲突加前缀消歧
fn count_model_providers(providers: &[Provider]) -> HashMap<String, usize> {
    let mut count = HashMap::new();
    for provider in providers.iter().filter(|p| p.enabled) {
        for m in provider_declared_models(provider) {
            *count.entry(m).or_insert(0) += 1;
        }
    }
    count
}

pub fn generate_codex_catalog_json(providers: &[Provider]) -> Value {
    let native_cache = load_native_codex_models_cache();
    let mut models = Vec::new();
    let mut seen_slugs = HashSet::new();
    let mut priority = 1000;

    let model_provider_count = count_model_providers(providers);

    for provider in providers.iter().filter(|p| p.enabled) {
        let all_models = provider_declared_models(provider);
        let provider_prefix = provider.name.to_lowercase().replace(' ', "-");
        for model_name in all_models {
            let is_duplicate = model_provider_count
                .get(&model_name)
                .copied()
                .unwrap_or(0) > 1;

            let slug = if model_name.to_lowercase().starts_with(&format!("{}/", provider_prefix)) {
                model_name.clone()
            } else if is_duplicate {
                format!("{}/{}", provider_prefix, model_name)
            } else {
                model_name.clone()
            };

            if seen_slugs.contains(&slug) {
                continue;
            }
            seen_slugs.insert(slug.clone());

            let display_name = format!("{} · {}", model_name, provider.name);
            let description = format!("via Pivot ({})", provider.name);

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
                    "input_modalities": resolve_model_input_modalities(&model_name),
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

/// Resolve the behavesAs model ID for Claude Code client-side capability mapping
pub fn resolve_claude_behaves_as(model_name: &str) -> &'static str {
    let lower = model_name.to_lowercase();
    let name = lower.split('/').last().unwrap_or(&lower);
    if name.contains("opus")
        || name.contains("thinking")
        || name.contains("reason")
        || name.contains("r1")
        || name.contains("o1")
        || name.contains("o3")
    {
        "claude-3-opus"
    } else if name.contains("haiku")
        || name.contains("flash")
        || name.contains("mini")
        || name.contains("small")
        || name.contains("instant")
    {
        "claude-3-5-haiku"
    } else {
        "claude-3-7-sonnet"
    }
}

/// Generates the modelPicker configuration for ~/.claude/settings.json
pub fn generate_claude_model_picker(
    providers: &[Provider],
    routes: &[RouteRule],
) -> Value {
    let mut options = Vec::new();
    let mut seen_models = HashSet::new();

    // 1. Agent 槽位映射行 (Main / Reasoning / Fast)
    let tier_specs = [
        (
            "tier-main",
            "主力模型 · Main",
            "claude-3-7-sonnet",
            "主力槽位动态路由",
        ),
        (
            "tier-reasoning",
            "推理模型 · Reasoning",
            "claude-3-opus",
            "推理槽位动态路由",
        ),
        (
            "tier-fast",
            "快速模型 · Fast",
            "claude-3-5-haiku",
            "辅助槽位动态路由",
        ),
    ];

    for (tier_id, label, default_model, base_desc) in tier_specs {
        if let Some(rule) = routes.iter().find(|r| r.id == tier_id && r.enabled) {
            let target_prov = providers
                .iter()
                .find(|p| p.id == rule.target_provider_id)
                .map(|p| p.name.as_str())
                .unwrap_or("未指定服务商");
            let target_m = rule.target_model.as_deref().unwrap_or(default_model);
            let desc = format!("{} (当前: {} via {})", base_desc, target_m, target_prov);
            options.push(json!({
                "model": default_model,
                "label": label,
                "description": desc,
                "behavesAs": default_model,
            }));
            seen_models.insert(default_model.to_string());
        }
    }

    // 2. 统计各模型在不同服务商中的出现频次以判断是否需要前缀以防同名冲突
    let model_provider_count = count_model_providers(providers);

    // 3. 所有已启用服务商的所有可用模型
    for provider in providers.iter().filter(|p| p.enabled) {
        let provider_models = provider_declared_models(provider);
        let provider_prefix = provider.name.to_lowercase().replace(' ', "-");

        for model_name in provider_models {
            let behaves_as = resolve_claude_behaves_as(&model_name);
            let is_duplicate = model_provider_count
                .get(&model_name)
                .copied()
                .unwrap_or(0) > 1;

            if is_duplicate {
                let prefixed = format!("{}/{}", provider_prefix, model_name);
                if !seen_models.contains(&prefixed) {
                    seen_models.insert(prefixed.clone());
                    options.push(json!({
                        "model": prefixed,
                        "label": format!("{} · {}", model_name, provider.name),
                        "description": format!("via Pivot ({})", provider.name),
                        "behavesAs": behaves_as,
                    }));
                }
            } else {
                if !seen_models.contains(&model_name) {
                    seen_models.insert(model_name.clone());
                    options.push(json!({
                        "model": model_name,
                        "label": format!("{} · {}", model_name, provider.name),
                        "description": format!("via Pivot ({})", provider.name),
                        "behavesAs": behaves_as,
                    }));
                }
            }
        }
    }

    json!({
        "options": options,
        "replaceBuiltInOptions": true
    })
}

/// Generate Pi models list and preferred model from configured providers and routes
pub fn generate_pi_models_list(
    providers: &[Provider],
    routes: &[RouteRule],
) -> (Vec<Value>, Option<String>) {
    let mut models = Vec::new();
    let mut seen_ids = HashSet::new();

    // 1. Tier 槽位模型
    let tier_specs = [
        ("tier-main", "主力模型 (Main)", "claude-3-7-sonnet"),
        ("tier-reasoning", "推理模型 (Reasoning)", "claude-3-opus"),
        ("tier-fast", "快速模型 (Fast)", "claude-3-5-haiku"),
    ];

    for (tier_id, label, default_model) in tier_specs {
        if let Some(rule) = routes.iter().find(|r| r.id == tier_id && r.enabled) {
            let target_m = rule.target_model.as_deref().unwrap_or(default_model);
            let id = default_model.to_string();
            if !seen_ids.contains(&id) {
                seen_ids.insert(id.clone());
                let (levels, default_lvl) = resolve_model_reasoning_spec(target_m);
                let is_reasoning = !levels.is_empty() && default_lvl != "none";
                let (ctx, _) = resolve_model_context_window(target_m);
                models.push(json!({
                    "id": id,
                    "name": format!("{} · Pivot 动态路由", label),
                    "reasoning": is_reasoning,
                    "input": resolve_model_input_modalities(target_m),
                    "contextWindow": ctx,
                    "maxTokens": 16384,
                }));
            }
        }
    }

    // 2. 统计各模型在不同服务商中的出现频次以判断是否需要前缀以防同名冲突
    let model_provider_count = count_model_providers(providers);

    // 3. 所有已启用服务商的所有可用模型
    // 默认服务商的兜底模型先记下来，等清单生成后按同一套 id 规则回填，避免 defaultModel 指向不存在的 id
    let default_target = providers
        .iter()
        .find(|p| p.is_default && p.enabled)
        .and_then(|p| {
            let m = p
                .default_model
                .as_deref()
                .or_else(|| p.models.first().map(|s| s.as_str()))?
                .trim()
                .to_string();
            if m.is_empty() {
                None
            } else {
                Some((p.id.clone(), m))
            }
        });

    let mut preferred_model = None;

    for provider in providers.iter().filter(|p| p.enabled) {
        let provider_models = provider_declared_models(provider);
        let provider_prefix = provider.name.to_lowercase().replace(' ', "-");

        for model_name in provider_models {
            let is_duplicate = model_provider_count
                .get(&model_name)
                .copied()
                .unwrap_or(0) > 1;

            // 跨服务商重名才带前缀，否则用模型原名：每个模型在列表里只出现一次
            let id = if model_name.to_lowercase().starts_with(&format!("{}/", provider_prefix)) {
                model_name.clone()
            } else if is_duplicate {
                format!("{}/{}", provider_prefix, model_name)
            } else {
                model_name.clone()
            };

            if !seen_ids.insert(id.clone()) {
                continue;
            }

            let (levels, default_lvl) = resolve_model_reasoning_spec(&model_name);
            let is_reasoning = !levels.is_empty() && default_lvl != "none";
            let (ctx, _) = resolve_model_context_window(&model_name);
            models.push(json!({
                "id": id,
                "name": format!("{} · {}", model_name, provider.name),
                "reasoning": is_reasoning,
                "input": resolve_model_input_modalities(&model_name),
                "contextWindow": ctx,
                "maxTokens": 16384,
            }));

            if preferred_model.is_none() {
                if let Some((ref def_provider, ref def_model)) = default_target {
                    if *def_provider == provider.id && *def_model == model_name {
                        preferred_model = Some(id);
                    }
                }
            }
        }
    }

    if preferred_model.is_none() {
        preferred_model = models.first().and_then(|m| m.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()));
    }

    (models, preferred_model)
}

/// Accurately resolves supported reasoning levels and default effort for a model
pub fn resolve_model_reasoning_spec(model_name: &str) -> (Vec<Value>, &'static str) {
    // 目录只用来补强：聚合商条目的 reasoning 标记并不可靠，
    // 因此目录说 true 时给未知模型补一组通用档位，说 false 时不推翻名称启发式
    let catalog_reasoning = crate::model_catalog::lookup(model_name, None).map(|m| m.reasoning);

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
    // 目录已知是推理模型但名称规则不认识时，默认给一组通用档位
    (
        vec![
            json!({ "description": "Default", "effort": "none" }),
            json!({ "description": "Low Thinking", "effort": "low" }),
            json!({ "description": "Medium Thinking", "effort": "medium" }),
            json!({ "description": "High Thinking", "effort": "high" }),
        ],
        if catalog_reasoning == Some(true) {
            "medium"
        } else {
            "none"
        },
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

/// Restore Gemini CLI official .env file while preserving user API keys and all other variables
fn restore_gemini_env(content: &str, original_endpoint: Option<&str>, gateway_url: &str) -> String {
    let mut lines = Vec::new();
    let clean_orig = original_endpoint.filter(|s| !s.trim().is_empty() && !s.contains(gateway_url));

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("GOOGLE_GEMINI_BASE_URL=") || trimmed.starts_with("GEMINI_BASE_URL=") {
            if let Some(orig) = clean_orig {
                if trimmed.starts_with("GOOGLE_GEMINI_BASE_URL=") {
                    lines.push(format!("GOOGLE_GEMINI_BASE_URL={}", orig));
                } else {
                    lines.push(format!("GEMINI_BASE_URL={}", orig));
                }
            }
            // 若原先没有配置，则直接移除，恢复为官方默认直连
        } else {
            lines.push(line.to_string());
        }
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
        let _lock = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let home = std::env::temp_dir().join(format!(
            "pivot-interceptor-{}",
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
            let claude_actual: Value = serde_json::from_str(&fs::read_to_string(home.join(".claude.json")).map_err(|e| e.to_string())?).unwrap();
            let claude_expected: Value = serde_json::from_str(claude_original).unwrap();
            assert_eq!(claude_actual, claude_expected);

            let codex_dir = home.join(".codex");
            fs::create_dir_all(&codex_dir).map_err(|e| e.to_string())?;
            let codex_original = "model = \"gpt-4o\"\n\n";
            fs::write(codex_dir.join("config.toml"), codex_original)
                .map_err(|e| e.to_string())?;
            manager.toggle_codex(true)?;
            manager.toggle_codex(true)?;
            manager.toggle_codex(false)?;
            let codex_actual: toml::Value = toml::from_str(&fs::read_to_string(codex_dir.join("config.toml")).map_err(|e| e.to_string())?).unwrap();
            let codex_expected: toml::Value = toml::from_str(codex_original).unwrap();
            assert_eq!(codex_actual, codex_expected);

            let gemini_dir = home.join(".gemini");
            fs::create_dir_all(&gemini_dir).map_err(|e| e.to_string())?;
            let gemini_original = "{\"theme\":\"dark\",\"baseUrl\":\"https://gemini.example\"}";
            fs::write(gemini_dir.join("config.json"), gemini_original)
                .map_err(|e| e.to_string())?;
            manager.toggle_gemini_cli(true)?;
            manager.toggle_gemini_cli(true)?;
            manager.toggle_gemini_cli(false)?;
            let gemini_actual: Value = serde_json::from_str(&fs::read_to_string(gemini_dir.join("config.json")).map_err(|e| e.to_string())?).unwrap();
            let gemini_expected: Value = serde_json::from_str(gemini_original).unwrap();
            assert_eq!(gemini_actual, gemini_expected);

            let pi_dir = home.join(".pi").join("agent");
            fs::create_dir_all(&pi_dir).map_err(|e| e.to_string())?;
            let pi_models_original = r#"{"providers":{"other":{"baseUrl":"https://pi.example","api":"openai-completions","apiKey":"sk-other"}}}"#;
            let pi_settings_original = r#"{"defaultProvider":"other","defaultModel":"gpt-4o"}"#;
            fs::write(pi_dir.join("models.json"), pi_models_original).map_err(|e| e.to_string())?;
            fs::write(pi_dir.join("settings.json"), pi_settings_original).map_err(|e| e.to_string())?;

            manager.toggle_pi(true)?;
            let pi_intercepted = manager.get_pi_status();
            assert!(pi_intercepted.is_intercepted);
            let models_content = fs::read_to_string(pi_dir.join("models.json")).unwrap();
            assert!(models_content.contains("http://127.0.0.1:16888/v1"));
            let settings_content = fs::read_to_string(pi_dir.join("settings.json")).unwrap();
            assert!(settings_content.contains("\"defaultProvider\": \"pivot\""));

            manager.toggle_pi(true)?;
            manager.toggle_pi(false)?;

            let pi_models_actual: Value = serde_json::from_str(&fs::read_to_string(pi_dir.join("models.json")).unwrap()).unwrap();
            let pi_models_expected: Value = serde_json::from_str(pi_models_original).unwrap();
            assert_eq!(pi_models_actual, pi_models_expected);

            let pi_settings_actual: Value = serde_json::from_str(&fs::read_to_string(pi_dir.join("settings.json")).unwrap()).unwrap();
            let pi_settings_expected: Value = serde_json::from_str(pi_settings_original).unwrap();
            assert_eq!(pi_settings_actual, pi_settings_expected);
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
            Some("pivot-model-catalog.json"),
            None,
        )
        .unwrap();
        assert!(updated.contains("http://127.0.0.1:16888/v1"));
        assert!(updated.contains("model_catalog_json = \"pivot-model-catalog.json\""));
        assert!(updated.contains("[desktop]"));
        assert!(updated.contains("followUpQueueMode = \"queue\""));
        assert!(!updated.contains("https://sub2api.vonote.com"));
        assert!(updated.contains("supports_websockets = false"));
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
        assert_eq!(models[0]["slug"], "deepseek-v4-pro");
        assert_eq!(models[0]["display_name"], "deepseek-v4-pro · DeepSeek");
        assert_eq!(models[1]["slug"], "deepseek-reasoner");
        assert_eq!(models[1]["display_name"], "deepseek-reasoner · DeepSeek");
        assert_eq!(models[1]["default_reasoning_level"], "high");
        assert_eq!(models[2]["slug"], "gpt-6-astra");
        assert_eq!(models[2]["display_name"], "gpt-6-astra · Work");
        assert_eq!(models[2]["context_window"], 272000);
        assert_eq!(models[2]["max_context_window"], 872000);
        let levels = models[2]["supported_reasoning_levels"].as_array().unwrap();
        assert_eq!(levels.len(), 6);
        assert_eq!(levels[5]["effort"], "ultra");
    }

    #[test]
    fn test_generate_pi_models_list() {
        let provider1 = Provider {
            id: "p1".to_string(),
            name: "Anthropic".to_string(),
            protocol: "anthropic".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            preset_id: None,
            api_keys: vec!["sk-ant".to_string()],
            models: vec!["claude-3-7-sonnet".to_string()],
            default_model: Some("claude-3-7-sonnet".to_string()),
            is_default: true,
            enabled: true,
            priority: 10,
            weight: 1,
            timeout_secs: 30,
            latency_ms: None,
            created_at: None,
            updated_at: None,
        };

        let (models, preferred_model) = generate_pi_models_list(&[provider1], &[]);
        assert!(!models.is_empty());
        // 模型不重名时用原名，且 defaultModel 必须指向清单里真实存在的 id
        assert_eq!(preferred_model, Some("claude-3-7-sonnet".to_string()));
        let sonnet_count = models.iter().filter(|m| m["id"] == "claude-3-7-sonnet").count();
        assert_eq!(sonnet_count, 1, "每个模型在清单里只能出现一次");
    }

    #[test]
    fn test_resolve_model_input_modalities() {
        assert_eq!(resolve_model_input_modalities("gpt-4o-mini"), vec!["text", "image"]);
        assert_eq!(resolve_model_input_modalities("claude-3-7-sonnet"), vec!["text", "image"]);
        assert_eq!(resolve_model_input_modalities("gemini-2.5-pro"), vec!["text", "image"]);
        assert_eq!(resolve_model_input_modalities("qwen2.5-vl-72b"), vec!["text", "image"]);
        // 未识别的模型保守只声明 text，避免上游 400
        assert_eq!(resolve_model_input_modalities("deepseek-v4-pro"), vec!["text"]);
        assert_eq!(resolve_model_input_modalities("gpt-3.5-turbo"), vec!["text"]);
        // 辅助模型不接收图片输入
        assert_eq!(resolve_model_input_modalities("gpt-image-1"), vec!["text"]);
        assert_eq!(resolve_model_input_modalities("text-embedding-3-large"), vec!["text"]);
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
        let _lock = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let home = std::env::temp_dir().join(format!(
            "pivot-conflict-test-{}",
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

            // 4. Pi Agent
            let pi_dir = home.join(".pi").join("agent");
            fs::create_dir_all(&pi_dir).map_err(|e| e.to_string())?;
            fs::write(
                pi_dir.join("models.json"),
                r#"{"providers":{"thirdparty":{"baseUrl":"https://pi-proxy.example.com/v1","apiKey":"sk-pi-123"}}}"#,
            )
            .map_err(|e| e.to_string())?;
            assert_eq!(
                manager.detect_third_party_endpoint("pi"),
                Some("https://pi-proxy.example.com/v1".to_string())
            );
            assert!(manager
                .check_third_party_conflict("pi", false)
                .unwrap()
                .contains("https://pi-proxy.example.com/v1"));
            let pi_tp = manager.get_third_party_config("pi").unwrap();
            assert_eq!(pi_tp.endpoint, "https://pi-proxy.example.com/v1");
            assert_eq!(pi_tp.auth_token, Some("sk-pi-123".to_string()));
            assert_eq!(pi_tp.protocol, "openai");

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

    #[test]
    fn test_claude_settings_incremental_patch_preserves_new_user_configs() {
        let _lock = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let home = std::env::temp_dir().join(format!(
            "pivot-inc-patch-test-{}",
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

            let claude_dir = home.join(".claude");
            fs::create_dir_all(&claude_dir).map_err(|e| e.to_string())?;
            let settings_file = claude_dir.join("settings.json");

            // 1. 用户最初的配置
            let initial = r#"{
  "permissions": {
    "allow": ["Bash(git status*)"]
  },
  "theme": "dark"
}"#;
            fs::write(&settings_file, initial).map_err(|e| e.to_string())?;

            // 2. 开启接管
            manager.toggle_claude_code(true)?;
            let intercepted_content = fs::read_to_string(&settings_file).map_err(|e| e.to_string())?;
            assert!(intercepted_content.contains("http://127.0.0.1:16888"));

            // 3. 模拟用户在接管期间使用 Claude Code，授权了新命令，并新增了 hook
            let mut v: Value = serde_json::from_str(&intercepted_content).map_err(|e| e.to_string())?;
            v["permissions"]["allow"] = json!(["Bash(git status*)", "Bash(cargo test*)"]);
            v["hooks"] = json!({ "PostToolUse": [{ "matcher": "*" }] });
            fs::write(&settings_file, serde_json::to_string_pretty(&v).unwrap()).map_err(|e| e.to_string())?;

            // 4. 关闭接管
            manager.toggle_claude_code(false)?;
            let restored_content = fs::read_to_string(&settings_file).map_err(|e| e.to_string())?;
            let restored: Value = serde_json::from_str(&restored_content).map_err(|e| e.to_string())?;

            // 验证：网关地址被安全移除
            assert_eq!(restored.get("env").and_then(|e| e.get("ANTHROPIC_BASE_URL")), None);
            // 验证：用户在接管期间新授权的命令和新设置的 hooks 100% 完整保留！
            let allowed = restored["permissions"]["allow"].as_array().unwrap();
            assert_eq!(allowed.len(), 2);
            assert_eq!(allowed[1], "Bash(cargo test*)");
            assert!(restored.get("hooks").is_some());
            assert_eq!(restored["theme"], "dark");

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

    #[test]
    fn test_generate_claude_model_picker() {
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

        let route1 = RouteRule {
            id: "tier-main".to_string(),
            pattern: ".*(sonnet|gpt-4o).*".to_string(),
            target_provider_id: "p2".to_string(),
            target_model: Some("gpt-6-astra".to_string()),
            fallback_provider_ids: vec![],
            enabled: true,
            priority: 500,
            description: Some("Agent Main Tier".to_string()),
        };

        let picker = generate_claude_model_picker(&[provider1, provider2], &[route1]);
        assert_eq!(picker["replaceBuiltInOptions"], true);
        let options = picker["options"].as_array().expect("options must be array");

        // Tier slot row
        assert_eq!(options[0]["model"], "claude-3-7-sonnet");
        assert_eq!(options[0]["behavesAs"], "claude-3-7-sonnet");
        assert!(options[0]["description"].as_str().unwrap().contains("gpt-6-astra via Work"));

        // Models
        assert_eq!(options[1]["model"], "deepseek-v4-pro");
        assert_eq!(options[1]["behavesAs"], "claude-3-7-sonnet");
        assert_eq!(options[2]["model"], "deepseek-reasoner");
        assert_eq!(options[2]["behavesAs"], "claude-3-opus");
        assert_eq!(options[3]["model"], "gpt-6-astra");
        assert_eq!(options[3]["behavesAs"], "claude-3-7-sonnet");
    }
}

