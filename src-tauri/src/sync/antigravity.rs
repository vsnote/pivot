use crate::sync::types::{SessionBundle, SyncSessionItem};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct AntigravityAdapter;

impl AntigravityAdapter {
    pub fn get_base_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".gemini").join("antigravity"))
    }

    pub fn is_installed() -> bool {
        Self::get_base_dir()
            .map(|p| p.join("conversation_summaries.db").exists())
            .unwrap_or(false)
    }

    pub fn list_local_sessions() -> Result<Vec<SyncSessionItem>, String> {
        let base_dir = Self::get_base_dir().ok_or("Failed to locate home directory")?;
        let db_path = base_dir.join("conversation_summaries.db");

        if !db_path.exists() {
            return Ok(Vec::new());
        }

        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT conversation_id, title, preview, step_count, last_modified_time, workspace_uris
                 FROM conversation_summaries
                 ORDER BY last_modified_time DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let preview: String = row.get(2)?;
                let step_count: i64 = row.get(3)?;
                let updated_at: String = row.get(4)?;
                let workspace_uris: String = row.get(5)?;

                let mut project_name = "default".to_string();
                if let Ok(v) = serde_json::from_str::<Value>(&workspace_uris) {
                    if let Some(arr) = v.as_array() {
                        if let Some(first) = arr.first().and_then(|u| u.as_str()) {
                            project_name = first
                                .trim_end_matches('/')
                                .split('/')
                                .last()
                                .unwrap_or("project")
                                .to_string();
                        }
                    }
                } else if !workspace_uris.is_empty() {
                    project_name = workspace_uris
                        .trim_end_matches('/')
                        .split('/')
                        .last()
                        .unwrap_or("project")
                        .to_string();
                }

                Ok(SyncSessionItem {
                    id,
                    agent_type: "antigravity".to_string(),
                    title: if title.is_empty() { "未命名会话".to_string() } else { title },
                    preview,
                    project_name,
                    updated_at,
                    step_count,
                    tokens_used: 0,
                    sync_status: "local_only".to_string(),
                    remote_updated_at: None,
                    size_bytes: None,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut items = Vec::new();
        for r in rows {
            if let Ok(item) = r {
                items.push(item);
            }
        }

        Ok(items)
    }

    pub fn bundle_session(session_id: &str) -> Result<SessionBundle, String> {
        let base_dir = Self::get_base_dir().ok_or("Failed to locate home directory")?;
        let db_path = base_dir.join("conversation_summaries.db");

        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT conversation_id, title, preview, step_count, last_modified_time, workspace_uris,
                        status, source, project_id, agent_name, last_user_input_time
                 FROM conversation_summaries
                 WHERE conversation_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let (title, preview, _step_count, updated_at, workspace_uris, meta_json) = stmt
            .query_row([session_id], |row| {
                let id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let preview: String = row.get(2)?;
                let step_count: i64 = row.get(3)?;
                let updated_at: String = row.get(4)?;
                let workspace_uris: String = row.get(5)?;
                let status: String = row.get(6)?;
                let source: String = row.get(7)?;
                let project_id: String = row.get(8)?;
                let agent_name: String = row.get(9)?;
                let last_user_input_time: String = row.get(10)?;

                let meta = serde_json::json!({
                    "conversation_id": id,
                    "title": title,
                    "preview": preview,
                    "step_count": step_count,
                    "last_modified_time": updated_at,
                    "workspace_uris": workspace_uris,
                    "status": status,
                    "source": source,
                    "project_id": project_id,
                    "agent_name": agent_name,
                    "last_user_input_time": last_user_input_time
                });

                Ok((title, preview, step_count, updated_at, workspace_uris, meta))
            })
            .map_err(|e| format!("Session {} not found in conversation_summaries: {}", session_id, e))?;

        let mut project_name = "default".to_string();
        let mut workspace_path = "".to_string();
        if let Ok(v) = serde_json::from_str::<Value>(&workspace_uris) {
            if let Some(arr) = v.as_array() {
                if let Some(first) = arr.first().and_then(|u| u.as_str()) {
                    workspace_path = first.to_string();
                    project_name = first
                        .trim_end_matches('/')
                        .split('/')
                        .last()
                        .unwrap_or("project")
                        .to_string();
                }
            }
        }

        let mut files = HashMap::new();

        // 1. Conversation SQLite DB or protobuf
        let conv_db = base_dir.join("conversations").join(format!("{}.db", session_id));
        if conv_db.exists() {
            if let Ok(bytes) = fs::read(&conv_db) {
                files.insert(format!("conversations/{}.db", session_id), BASE64.encode(bytes));
            }
        }
        let conv_pb = base_dir.join("conversations").join(format!("{}.pb", session_id));
        if conv_pb.exists() {
            if let Ok(bytes) = fs::read(&conv_pb) {
                files.insert(format!("conversations/{}.pb", session_id), BASE64.encode(bytes));
            }
        }

        // 2. Brain directory (logs, artifacts)
        let brain_dir = base_dir.join("brain").join(session_id);
        if brain_dir.exists() {
            Self::collect_files_recursive(&brain_dir, &brain_dir, &workspace_path, &mut files)?;
        }

        Ok(SessionBundle {
            version: 1,
            agent_type: "antigravity".to_string(),
            id: session_id.to_string(),
            title: if title.is_empty() { "未命名会话".to_string() } else { title },
            preview,
            project_name,
            updated_at,
            metadata: meta_json,
            files,
        })
    }

    fn collect_files_recursive(
        root: &Path,
        current: &Path,
        workspace_path: &str,
        files: &mut HashMap<String, String>,
    ) -> Result<(), String> {
        if let Ok(entries) = fs::read_dir(current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    Self::collect_files_recursive(root, &path, workspace_path, files)?;
                } else if path.is_file() {
                    if let Ok(rel_path) = path.strip_prefix(root) {
                        let rel_str = format!("brain/{}", rel_path.to_string_lossy());
                        // If file is text/json/markdown, do workspace path replacement
                        if let Ok(content) = fs::read_to_string(&path) {
                            let virtualized = if !workspace_path.is_empty() {
                                content.replace(workspace_path, "${WORKSPACE_ROOT}")
                            } else {
                                content
                            };
                            files.insert(rel_str, virtualized);
                        } else if let Ok(bytes) = fs::read(&path) {
                            files.insert(rel_str, format!("__BASE64__:{}", BASE64.encode(bytes)));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn restore_session(bundle: &SessionBundle) -> Result<(), String> {
        let base_dir = Self::get_base_dir().ok_or("Failed to locate home directory")?;
        let convs_dir = base_dir.join("conversations");
        let brain_base = base_dir.join("brain").join(&bundle.id);

        fs::create_dir_all(&convs_dir).map_err(|e| e.to_string())?;
        fs::create_dir_all(&brain_base).map_err(|e| e.to_string())?;

        // Restore files
        for (rel_path, content) in &bundle.files {
            if rel_path.starts_with("conversations/") {
                let filename = rel_path.trim_start_matches("conversations/");
                let dest = convs_dir.join(filename);
                if let Ok(bytes) = BASE64.decode(content) {
                    let _ = fs::write(dest, bytes);
                }
            } else if rel_path.starts_with("brain/") {
                let sub_rel = rel_path.trim_start_matches("brain/");
                let dest = brain_base.join(sub_rel);
                if let Some(parent) = dest.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                if content.starts_with("__BASE64__:") {
                    let b64_data = &content["__BASE64__:".len()..];
                    if let Ok(bytes) = BASE64.decode(b64_data) {
                        let _ = fs::write(dest, bytes);
                    }
                } else {
                    // Normalize back: keep ${WORKSPACE_ROOT} or replace with local workspace
                    let _ = fs::write(dest, content);
                }
            }
        }

        // Upsert into conversation_summaries.db
        let db_path = base_dir.join("conversation_summaries.db");
        if db_path.exists() {
            let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
            let meta = &bundle.metadata;

            let title = meta.get("title").and_then(|v| v.as_str()).unwrap_or(&bundle.title);
            let preview = meta.get("preview").and_then(|v| v.as_str()).unwrap_or(&bundle.preview);
            let step_count = meta.get("step_count").and_then(|v| v.as_i64()).unwrap_or(0);
            let updated_at = meta.get("last_modified_time").and_then(|v| v.as_str()).unwrap_or(&bundle.updated_at);
            let workspace_uris = meta.get("workspace_uris").and_then(|v| v.as_str()).unwrap_or("[]");
            let status = meta.get("status").and_then(|v| v.as_str()).unwrap_or("DONE");
            let source = meta.get("source").and_then(|v| v.as_str()).unwrap_or("USER_EXPLICIT");
            let project_id = meta.get("project_id").and_then(|v| v.as_str()).unwrap_or("");
            let agent_name = meta.get("agent_name").and_then(|v| v.as_str()).unwrap_or("");
            let last_user_input_time = meta.get("last_user_input_time").and_then(|v| v.as_str()).unwrap_or(updated_at);

            let _ = conn.execute(
                "INSERT INTO conversation_summaries (
                    conversation_id, title, preview, step_count, last_modified_time,
                    workspace_uris, status, source, project_id, agent_name,
                    last_user_input_time
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(conversation_id) DO UPDATE SET
                    title=excluded.title,
                    preview=excluded.preview,
                    step_count=excluded.step_count,
                    last_modified_time=excluded.last_modified_time,
                    workspace_uris=excluded.workspace_uris,
                    status=excluded.status",
                rusqlite::params![
                    bundle.id,
                    title,
                    preview,
                    step_count,
                    updated_at,
                    workspace_uris,
                    status,
                    source,
                    project_id,
                    agent_name,
                    last_user_input_time
                ],
            );
        }

        Ok(())
    }
}
