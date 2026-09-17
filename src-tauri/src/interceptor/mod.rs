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
            original_endpoint,
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
            original_endpoint,
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
                format!(
                    "# Managed by OmniSwitch\nbase_url = \"{}\"\napi_endpoint = \"{}\"\n\n{}",
                    target_endpoint, target_endpoint, original_content
                )
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

    pub fn get_gemini_cli_status(&self) -> InterceptorInfo {
        let config_path = Self::gemini_config_path();
        let backup_path = Self::gemini_backup_path();
        let installed = Self::is_binary_installed("gemini")
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
            config_path: config_path.to_string_lossy().to_string(),
            is_intercepted,
            backup_path: if backup_path.exists() {
                Some(backup_path.to_string_lossy().to_string())
            } else {
                None
            },
            original_endpoint,
        }
    }

    pub fn toggle_gemini_cli(&self, enable: bool) -> Result<InterceptorInfo, String> {
        let config_path = Self::gemini_config_path();
        let backup_path = Self::gemini_backup_path();

        if enable {
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
            let saved_state = self
                .db
                .get_interceptor_state("gemini_cli")
                .ok()
                .flatten();
            let already_intercepted = backup_path.exists()
                || saved_state.as_ref().map(|state| state.0).unwrap_or(false)
                || original_content.contains(&self.gateway_url);

            if !already_intercepted && config_path.exists() {
                atomic_write(&backup_path, &original_content).map_err(|e| e.to_string())?;
            }

            parsed["baseUrl"] = json!(self.gateway_url);
            let new_content = serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())?;
            atomic_write(&config_path, &new_content).map_err(|e| e.to_string())?;

            if !already_intercepted {
                let _ = self
                    .db
                    .save_interceptor_state("gemini_cli", true, Some(&original_content), None);
            }
        } else {
            if backup_path.exists() {
                if let Ok(orig) = fs::read_to_string(&backup_path) {
                    let _ = atomic_write(&config_path, &orig);
                }
                let _ = fs::remove_file(&backup_path);
            } else if let Ok(Some((_, Some(backup_content), _))) =
                self.db.get_interceptor_state("gemini_cli")
            {
                let _ = atomic_write(&config_path, &backup_content);
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
}
