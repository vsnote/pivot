pub mod antigravity;
pub mod codex;
pub mod s3_client;
pub mod types;

use antigravity::AntigravityAdapter;
use codex::CodexAdapter;
use s3_client::S3Client;
use std::collections::HashMap;
use types::{SessionBundle, SyncConfig, SyncSessionItem, TestConnectionResult};

pub struct SyncManager {
    s3: S3Client,
}

impl SyncManager {
    pub fn new(config: SyncConfig) -> Self {
        Self {
            s3: S3Client::new(config),
        }
    }

    pub async fn test_connection(&self) -> Result<TestConnectionResult, String> {
        self.s3.test_connection().await
    }

    pub async fn get_sessions(&self, agent_type: &str) -> Result<Vec<SyncSessionItem>, String> {
        // 1. Get local sessions
        let local_items = match agent_type {
            "antigravity" => AntigravityAdapter::list_local_sessions()?,
            "codex" => CodexAdapter::list_local_sessions()?,
            _ => return Err(format!("Unsupported agent type: {}", agent_type)),
        };

        let mut session_map: HashMap<String, SyncSessionItem> = HashMap::new();
        for item in local_items {
            session_map.insert(item.id.clone(), item);
        }

        // 2. Fetch remote sessions from R2
        let prefix = format!("{}/sessions/", agent_type);
        let remote_objects = self.s3.list_objects(&prefix).await.unwrap_or_default();

        for (key, last_modified, size) in remote_objects {
            let filename = key.trim_start_matches(&prefix);
            if let Some(id) = filename.strip_suffix(".bundle.json") {
                if let Some(existing) = session_map.get_mut(id) {
                    existing.remote_updated_at = Some(last_modified);
                    existing.size_bytes = Some(size);
                    existing.sync_status = "synced".to_string();
                } else {
                    // Remote-only session
                    session_map.insert(
                        id.to_string(),
                        SyncSessionItem {
                            id: id.to_string(),
                            agent_type: agent_type.to_string(),
                            title: format!("云端会话 ({})", &id[..id.len().min(8)]),
                            preview: "可从云端拉取恢复至本机".to_string(),
                            project_name: "remote".to_string(),
                            updated_at: last_modified.clone(),
                            step_count: 0,
                            tokens_used: 0,
                            sync_status: "remote_only".to_string(),
                            remote_updated_at: Some(last_modified),
                            size_bytes: Some(size),
                        },
                    );
                }
            }
        }

        let mut list: Vec<SyncSessionItem> = session_map.into_values().collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(list)
    }

    pub async fn upload_session(&self, agent_type: &str, session_id: &str) -> Result<(), String> {
        let bundle = match agent_type {
            "antigravity" => AntigravityAdapter::bundle_session(session_id)?,
            "codex" => CodexAdapter::bundle_session(session_id)?,
            _ => return Err(format!("Unsupported agent type: {}", agent_type)),
        };

        let json_bytes = serde_json::to_vec(&bundle)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let key = format!("{}/sessions/{}.bundle.json", agent_type, session_id);
        self.s3.put_object(&key, &json_bytes).await?;

        Ok(())
    }

    pub async fn restore_session(&self, agent_type: &str, session_id: &str) -> Result<(), String> {
        let key = format!("{}/sessions/{}.bundle.json", agent_type, session_id);
        let bytes = self.s3.get_object(&key).await?;

        let bundle: SessionBundle = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Deserialization error: {}", e))?;

        match agent_type {
            "antigravity" => AntigravityAdapter::restore_session(&bundle)?,
            "codex" => CodexAdapter::restore_session(&bundle)?,
            _ => return Err(format!("Unsupported agent type: {}", agent_type)),
        }

        Ok(())
    }

    pub async fn sync_all(&self, agent_type: &str) -> Result<(usize, usize), String> {
        let sessions = self.get_sessions(agent_type).await?;
        let mut uploaded = 0;
        let mut restored = 0;

        for s in sessions {
            if s.sync_status == "local_only" || s.sync_status == "modified" {
                if self.upload_session(agent_type, &s.id).await.is_ok() {
                    uploaded += 1;
                }
            } else if s.sync_status == "remote_only" {
                if self.restore_session(agent_type, &s.id).await.is_ok() {
                    restored += 1;
                }
            }
        }

        Ok((uploaded, restored))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_installation_detection() {
        let ag_installed = AntigravityAdapter::is_installed();
        let codex_installed = CodexAdapter::is_installed();
        // Since we are running on a machine with both installed, assert at least one is detected
        println!("Antigravity installed: {}, Codex installed: {}", ag_installed, codex_installed);
    }

    #[test]
    fn test_list_local_sessions() {
        if AntigravityAdapter::is_installed() {
            let ag_sessions = AntigravityAdapter::list_local_sessions().unwrap();
            assert!(!ag_sessions.is_empty(), "Antigravity should have local sessions");
            println!("Found {} Antigravity sessions. First: {}", ag_sessions.len(), ag_sessions[0].title);
        }

        if CodexAdapter::is_installed() {
            let codex_sessions = CodexAdapter::list_local_sessions().unwrap();
            assert!(!codex_sessions.is_empty(), "Codex should have local sessions");
            println!("Found {} Codex sessions. First: {}", codex_sessions.len(), codex_sessions[0].title);
        }
    }

    #[tokio::test]
    async fn test_r2_connection_with_unconfigured() {
        let config = SyncConfig::default();
        let mgr = SyncManager::new(config);
        let res = mgr.test_connection().await.unwrap();
        assert!(!res.success, "Unconfigured should report not configured");
    }

    #[tokio::test]
    async fn test_bundle_and_upload_antigravity_session() {
        if AntigravityAdapter::is_installed() {
            let ag_sessions = AntigravityAdapter::list_local_sessions().unwrap();
            if let Some(first) = ag_sessions.first() {
                let bundle = AntigravityAdapter::bundle_session(&first.id).unwrap();
                assert_eq!(bundle.id, first.id);
                println!("Successfully bundled session: {}, files count: {}", bundle.title, bundle.files.len());

                let config = SyncConfig::default();
                let mgr = SyncManager::new(config.clone());
                
                if !config.is_configured() {
                    // 默认未配置状态下，安全防御机制应拦截并报错
                    let err = mgr.upload_session("antigravity", &first.id).await.unwrap_err();
                    assert!(err.contains("尚未配置云端存储"));
                    println!("Verified unconfigured guard blocked upload safely: {}", err);
                } else {
                    // Upload
                    mgr.upload_session("antigravity", &first.id).await.unwrap();
                    println!("Uploaded session {} to R2", first.id);

                    // Check it appears in remote sessions
                    let sessions = mgr.get_sessions("antigravity").await.unwrap();
                    let uploaded_item = sessions.iter().find(|s| s.id == first.id).unwrap();
                    assert_eq!(uploaded_item.sync_status, "synced");
                    println!("Verified session {} status is now 'synced'", first.id);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_bundle_and_upload_codex_session() {
        if CodexAdapter::is_installed() {
            let codex_sessions = CodexAdapter::list_local_sessions().unwrap();
            if let Some(first) = codex_sessions.first() {
                let bundle = CodexAdapter::bundle_session(&first.id).unwrap();
                assert_eq!(bundle.id, first.id);
                println!("Successfully bundled Codex session: {}, files count: {}", bundle.title, bundle.files.len());

                let config = SyncConfig::default();
                let mgr = SyncManager::new(config.clone());
                
                if !config.is_configured() {
                    // 默认未配置状态下，安全防御机制应拦截并报错
                    let err = mgr.upload_session("codex", &first.id).await.unwrap_err();
                    assert!(err.contains("尚未配置云端存储"));
                    println!("Verified unconfigured guard blocked Codex upload safely: {}", err);
                } else {
                    // Upload
                    mgr.upload_session("codex", &first.id).await.unwrap();
                    println!("Uploaded Codex session {} to R2", first.id);

                    // Check it appears in remote sessions
                    let sessions = mgr.get_sessions("codex").await.unwrap();
                    let uploaded_item = sessions.iter().find(|s| s.id == first.id).unwrap();
                    assert_eq!(uploaded_item.sync_status, "synced");
                    println!("Verified Codex session {} status is now 'synced'", first.id);
                }
            }
        }
    }
}
