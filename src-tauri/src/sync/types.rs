use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    #[serde(default)]
    pub provider_type: Option<String>,
    pub endpoint: String,
    #[serde(default)]
    pub region: Option<String>,
    pub account_id: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            provider_type: Some("r2".to_string()),
            endpoint: String::new(),
            region: Some("auto".to_string()),
            account_id: String::new(),
            bucket: String::new(),
            access_key_id: String::new(),
            secret_access_key: String::new(),
        }
    }
}

impl SyncConfig {
    pub fn is_configured(&self) -> bool {
        !self.endpoint.trim().is_empty()
            && !self.bucket.trim().is_empty()
            && !self.access_key_id.trim().is_empty()
            && !self.secret_access_key.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionItem {
    pub id: String,
    pub agent_type: String, // "antigravity" | "codex"
    pub title: String,
    pub preview: String,
    pub project_name: String,
    pub updated_at: String,
    pub step_count: i64,
    pub tokens_used: i64,
    pub sync_status: String, // "synced" | "local_only" | "remote_only" | "modified"
    pub remote_updated_at: Option<String>,
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBundle {
    pub version: u32,
    pub agent_type: String,
    pub id: String,
    pub title: String,
    pub preview: String,
    pub project_name: String,
    pub updated_at: String,
    pub metadata: serde_json::Value,
    /// Map of relative path -> base64 or text content
    pub files: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: Option<i64>,
}
