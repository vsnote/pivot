use crate::db::Database;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

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

    /// Extracts the current custom endpoint from the tool's configuration if it points
    /// to a third-party service (i.e. neither official upstream nor this OmniSwitch gateway).
    fn detect_third_party_endpoint(&self, tool_id: &str) -> Option<String> {
        match tool_id {
            "claude_code" => {
                let config_path = Self::claude_config_path();
                if config_path.exists() {
                    if let Ok(content) = fs::read_to_string(&config_path) {
                        if !content.contains(&self.gateway_url) {
                            if let Ok(v) = serde_json::from_str::<Value>(&content) {
                                let ep = v
                                    .get("anthropicBaseUrl")
                                    .and_then(|u| u.as_str())
                                    .or_else(|| {
                                        v.get("env")
                                            .and_then(|e| e.get("ANTHROPIC_BASE_URL"))
                                            .and_then(|u| u.as_str())
                                    });
                                if let Some(url) = ep {
                                    let trimmed = url.trim().trim_end_matches('/');
                                    if !trimmed.is_empty() && trimmed != "https://api.anthropic.com" {
                                        return Some(url.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "codex_cli" => {
                let config_path = Self::codex_config_path();
                if config_path.exists() {
                    if let Ok(content) = fs::read_to_string(&config_path) {
                        if !content.contains(&self.gateway_url) {
                            if let Ok(doc) = toml::from_str::<toml::Value>(&content) {
                                let current_provider = doc
                                    .get("model_provider")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("custom");

                                let ep = doc
                                    .get("model_providers")
                                    .and_then(|mp| mp.get(current_provider))
                                    .and_then(|p| p.get("base_url"))
                                    .and_then(|u| u.as_str())
                                    .or_else(|| doc.get("base_url").and_then(|u| u.as_str()));

                                if let Some(url) = ep {
                                    let trimmed = url.trim().trim_end_matches('/');
                                    if !trimmed.is_empty()
                                        && trimmed != "https://api.openai.com"
                                        && trimmed != "https://api.openai.com/v1"
                                    {
                                        return Some(url.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "gemini_cli" => {
                let env_path = Self::gemini_env_path();
                if env_path.exists() {
                    if let Ok(content) = fs::read_to_string(&env_path) {
                        if !content.contains(&self.gateway_url) {
                            for line in content.lines() {
                                let trimmed = line.trim();
                                let val = if let Some(stripped) = trimmed.strip_prefix("GOOGLE_GEMINI_BASE_URL=") {
                                    Some(stripped)
                                } else if let Some(stripped) = trimmed.strip_prefix("GEMINI_BASE_URL=") {
                                    Some(stripped)
                                } else {
                                    None
                                };
                                if let Some(url) = val {
                                    let clean_url = url
                                        .trim()
                                        .trim_matches('"')
                                        .trim_matches('\'')
                                        .trim_end_matches('/');
                                    if !clean_url.is_empty() && !clean_url.contains("googleapis.com") {
                                        return Some(clean_url.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                let config_path = Self::gemini_config_path();
                if config_path.exists() {
                    if let Ok(content) = fs::read_to_string(&config_path) {
                        if !content.contains(&self.gateway_url) {
                            if let Ok(v) = serde_json::from_str::<Value>(&content) {
                                if let Some(url) = v.get("baseUrl").and_then(|u| u.as_str()) {
                                    let clean_url = url.trim().trim_end_matches('/');
                                    if !clean_url.is_empty() && !clean_url.contains("googleapis.com") {
                                        return Some(clean_url.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    /// Generate user-facing conflict warning message if a third-party endpoint is detected.
    fn check_third_party_conflict(&self, tool_id: &str) -> Option<String> {
        let ep = self.detect_third_party_endpoint(tool_id)?;
        Some(format!(
            "检测到当前配置了第三方端点（{}）。开启 OmniSwitch 将会接管此配置，原配置会自动备份并在恢复官方直连时还原。",
            ep
        ))
    }

    // ================= Claude Code =================

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
        let config_path = Self::claude_config_path();
        let backup_path = Self::claude_backup_path();
        let installed = Self::is_binary_installed("claude")
            || config_path.exists()
            || Self::home_dir()
                .map(|h| h.join(".claude").exists())
                .unwrap_or(false);

        let mut is_intercepted = false;
        let mut original_endpoint = None;

        if let Ok(Some((db_intercepted, _, orig_ep))) = self.db.get_interceptor_state("claude_code")
        {
            is_intercepted = db_intercepted;
            original_endpoint = orig_ep;
        }

        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if content.contains(&self.gateway_url) {
                    is_intercepted = true;
                }
            }
        }

        InterceptorInfo {
            id: "claude_code".to_string(),
            name: "Claude Code CLI".to_string(),
            installed,
            config_path: config_path.to_string_lossy().to_string(),
            is_intercepted,
            backup_path: if backup_path.exists() {
                Some(backup_path.to_string_lossy().to_string())
            } else {
                None
            },
            original_endpoint: self.detect_third_party_endpoint("claude_code").or(original_endpoint),
            conflict: self.check_third_party_conflict("claude_code"),
        }
    }

    pub fn toggle_claude_code(&self, enable: bool) -> Result<InterceptorInfo, String> {
        let config_path = Self::claude_config_path();
        let backup_path = Self::claude_backup_path();

        if enable {
            let mut original_content = String::new();
            let mut parsed: Value = json!({});
            if config_path.exists() {
                original_content = fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
                if let Ok(v) = serde_json::from_str::<Value>(&original_content) {
                    parsed = v;
                }
            }
            let saved_state = self
                .db
                .get_interceptor_state("claude_code")
                .ok()
                .flatten();
            let already_intercepted = backup_path.exists()
                || saved_state.as_ref().map(|state| state.0).unwrap_or(false)
                || original_content.contains(&self.gateway_url);

            // Keep the first backup. Re-enabling must never replace the user's
            // original configuration with an already-intercepted snapshot.
            if !already_intercepted && config_path.exists() {
                atomic_write(&backup_path, &original_content).map_err(|e| e.to_string())?;
            }

            // Save original endpoint if present
            let orig_ep = parsed
                .get("anthropicBaseUrl")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Set OmniSwitch gateway endpoint
            parsed["anthropicBaseUrl"] = json!(self.gateway_url);
            if !parsed["env"].is_object() {
                parsed["env"] = json!({});
            }
            parsed["env"]["ANTHROPIC_BASE_URL"] = json!(self.gateway_url);

            let new_content = serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())?;
            atomic_write(&config_path, &new_content).map_err(|e| e.to_string())?;

            if !already_intercepted {
                let _ = self.db.save_interceptor_state(
                    "claude_code",
                    true,
                    Some(&original_content),
                    orig_ep.as_deref(),
                );
            }
        } else {
            // Restore
            if backup_path.exists() {
                if let Ok(orig) = fs::read_to_string(&backup_path) {
                    let _ = atomic_write(&config_path, &orig);
                }
                let _ = fs::remove_file(&backup_path);
            } else if let Ok(Some((_, Some(backup_content), _))) =
                self.db.get_interceptor_state("claude_code")
            {
                let _ = atomic_write(&config_path, &backup_content);
            } else if config_path.exists() {
                // Remove custom baseUrl keys
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
                        let updated = serde_json::to_string_pretty(&v).unwrap_or_default();
                        let _ = atomic_write(&config_path, &updated);
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

        let mut is_intercepted = false;
        let mut original_endpoint = None;

        if let Ok(Some((db_intercepted, _, orig_ep))) = self.db.get_interceptor_state("codex_cli") {
            is_intercepted = db_intercepted;
            original_endpoint = orig_ep;
        }

        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if content.contains(&self.gateway_url) {
                    is_intercepted = true;
                }
            }
        }

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
            original_endpoint: self.detect_third_party_endpoint("codex_cli").or(original_endpoint),
            conflict: self.check_third_party_conflict("codex_cli"),
        }
    }

    pub fn toggle_codex(&self, enable: bool) -> Result<InterceptorInfo, String> {
        let config_path = Self::codex_config_path();
        let backup_path = Self::codex_backup_path();
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

            let new_content = if original_content.contains(&target_endpoint) {
                original_content.clone()
            } else {
                update_codex_config_toml(&original_content, &target_endpoint)?
            };
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
                // If it was created purely by us, delete or remove base_url lines
                let _ = fs::remove_file(&config_path);
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

        let mut is_intercepted = false;
        let mut original_endpoint = None;

        if let Ok(Some((db_intercepted, _, orig_ep))) = self.db.get_interceptor_state("gemini_cli")
        {
            is_intercepted = db_intercepted;
            original_endpoint = orig_ep;
        }

        if env_path.exists() {
            if let Ok(content) = fs::read_to_string(&env_path) {
                if content.contains(&self.gateway_url) {
                    is_intercepted = true;
                }
            }
        }
        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if content.contains(&self.gateway_url) {
                    is_intercepted = true;
                }
            }
        }

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
            original_endpoint: self.detect_third_party_endpoint("gemini_cli").or(original_endpoint),
            conflict: self.check_third_party_conflict("gemini_cli"),
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
fn update_codex_config_toml(original: &str, target_endpoint: &str) -> Result<String, String> {
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

        // 2. 兼容现代多 Provider 模式 (如 model_providers.custom)
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
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn repeated_enable_preserves_original_cli_configuration() {
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
        let updated = update_codex_config_toml(original, "http://127.0.0.1:16888/v1").unwrap();
        assert!(updated.contains("http://127.0.0.1:16888/v1"));
        assert!(updated.contains("[desktop]"));
        assert!(updated.contains("followUpQueueMode = \"queue\""));
        assert!(!updated.contains("https://sub2api.vonote.com"));
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
            assert_eq!(manager.check_third_party_conflict("claude_code"), None);

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
                .check_third_party_conflict("claude_code")
                .unwrap()
                .contains("https://my-proxy.example.com"));

            // 2. Codex
            let codex_dir = home.join(".codex");
            fs::create_dir_all(&codex_dir).map_err(|e| e.to_string())?;
            fs::write(
                codex_dir.join("config.toml"),
                "base_url = \"https://api.openai.com/v1\"\n",
            )
            .map_err(|e| e.to_string())?;
            assert_eq!(manager.detect_third_party_endpoint("codex_cli"), None);
            assert_eq!(manager.check_third_party_conflict("codex_cli"), None);

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
                .check_third_party_conflict("codex_cli")
                .unwrap()
                .contains("https://sub2api.vonote.com"));

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
                .check_third_party_conflict("gemini_cli")
                .unwrap()
                .contains("https://gemini-proxy.com"));

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

