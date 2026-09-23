use crate::sync::types::{SessionBundle, SyncSessionItem};
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub struct CodexAdapter;

impl CodexAdapter {
    pub fn get_base_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".codex"))
    }

    pub fn is_installed() -> bool {
        Self::get_base_dir()
            .map(|p| p.join("state_5.sqlite").exists() || p.join("session_index.jsonl").exists())
            .unwrap_or(false)
    }

    pub fn list_local_sessions() -> Result<Vec<SyncSessionItem>, String> {
        let base_dir = Self::get_base_dir().ok_or("Failed to locate home directory")?;
        let db_path = base_dir.join("state_5.sqlite");

        if db_path.exists() {
            if let Ok(conn) = Connection::open(&db_path) {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, title, cwd, tokens_used, updated_at, preview
                         FROM threads
                         ORDER BY updated_at DESC",
                    )
                    .map_err(|e| e.to_string())?;

                let rows = stmt
                    .query_map([], |row| {
                        let id: String = row.get(0)?;
                        let title: String = row.get(1)?;
                        let cwd: String = row.get(2)?;
                        let tokens_used: i64 = row.get(3)?;
                        let updated_at_raw: i64 = row.get(4)?;
                        let preview: String = row.get(5).unwrap_or_default();

                        let project_name = cwd
                            .trim_end_matches('/')
                            .split('/')
                            .last()
                            .unwrap_or("project")
                            .to_string();

                        // Convert timestamp (secs or ms) to ISO
                        let ts_sec = if updated_at_raw > 10_000_000_000 {
                            updated_at_raw / 1000
                        } else {
                            updated_at_raw
                        };
                        let dt = DateTime::from_timestamp(ts_sec, 0).unwrap_or_else(Utc::now);
                        let updated_at = dt.to_rfc3339();

                        Ok(SyncSessionItem {
                            id,
                            agent_type: "codex".to_string(),
                            title: if title.is_empty() { "未命名线程".to_string() } else { title },
                            preview,
                            project_name,
                            updated_at,
                            step_count: 0,
                            tokens_used,
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

                if !items.is_empty() {
                    return Ok(items);
                }
            }
        }

        // Fallback: parse session_index.jsonl
        let index_path = base_dir.join("session_index.jsonl");
        if index_path.exists() {
            if let Ok(content) = fs::read_to_string(&index_path) {
                let mut items = Vec::new();
                for line in content.lines().rev() {
                    if let Ok(v) = serde_json::from_str::<Value>(line) {
                        if let (Some(id), Some(title), Some(updated_at)) = (
                            v.get("id").and_then(|x| x.as_str()),
                            v.get("thread_name").and_then(|x| x.as_str()),
                            v.get("updated_at").and_then(|x| x.as_str()),
                        ) {
                            items.push(SyncSessionItem {
                                id: id.to_string(),
                                agent_type: "codex".to_string(),
                                title: title.to_string(),
                                preview: "".to_string(),
                                project_name: "default".to_string(),
                                updated_at: updated_at.to_string(),
                                step_count: 0,
                                tokens_used: 0,
                                sync_status: "local_only".to_string(),
                                remote_updated_at: None,
                                size_bytes: None,
                            });
                        }
                    }
                }
                return Ok(items);
            }
        }

        Ok(Vec::new())
    }

    pub fn bundle_session(session_id: &str) -> Result<SessionBundle, String> {
        let base_dir = Self::get_base_dir().ok_or("Failed to locate home directory")?;
        let db_path = base_dir.join("state_5.sqlite");

        let mut rollout_path_str = "".to_string();
        let mut title = "".to_string();
        let mut preview = "".to_string();
        let mut cwd = "".to_string();
        let mut _tokens_used = 0i64;
        let mut updated_at_raw = 0i64;
        let mut meta_json = serde_json::json!({});

        if db_path.exists() {
            if let Ok(conn) = Connection::open(&db_path) {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, rollout_path, created_at, updated_at, source, model_provider,
                                cwd, title, tokens_used, preview
                         FROM threads
                         WHERE id = ?1",
                    )
                    .map_err(|e| e.to_string())?;

                if let Ok(row) = stmt.query_row([session_id], |row| {
                    let id: String = row.get(0)?;
                    let rpath: String = row.get(1)?;
                    let created_at: i64 = row.get(2)?;
                    let updated_at: i64 = row.get(3)?;
                    let source: String = row.get(4)?;
                    let model_provider: String = row.get(5)?;
                    let cwd: String = row.get(6)?;
                    let title: String = row.get(7)?;
                    let tokens_used: i64 = row.get(8)?;
                    let preview: String = row.get(9).unwrap_or_default();

                    let meta = serde_json::json!({
                        "id": id,
                        "rollout_path": rpath,
                        "created_at": created_at,
                        "updated_at": updated_at,
                        "source": source,
                        "model_provider": model_provider,
                        "cwd": cwd,
                        "title": title,
                        "tokens_used": tokens_used,
                        "preview": preview,
                    });

                    Ok((rpath, title, preview, cwd, tokens_used, updated_at, meta))
                }) {
                    rollout_path_str = row.0;
                    title = row.1;
                    preview = row.2;
                    cwd = row.3;
                    _tokens_used = row.4;
                    updated_at_raw = row.5;
                    meta_json = row.6;
                }
            }
        }

        // If title is still empty, try session_index.jsonl
        if title.is_empty() {
            let index_path = base_dir.join("session_index.jsonl");
            if index_path.exists() {
                if let Ok(content) = fs::read_to_string(&index_path) {
                    for line in content.lines() {
                        if let Ok(v) = serde_json::from_str::<Value>(line) {
                            if v.get("id").and_then(|x| x.as_str()) == Some(session_id) {
                                if let Some(t) = v.get("thread_name").and_then(|x| x.as_str()) {
                                    title = t.to_string();
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        let project_name = cwd
            .trim_end_matches('/')
            .split('/')
            .last()
            .unwrap_or("project")
            .to_string();

        let ts_sec = if updated_at_raw > 10_000_000_000 {
            updated_at_raw / 1000
        } else if updated_at_raw > 0 {
            updated_at_raw
        } else {
            Utc::now().timestamp()
        };
        let updated_at = DateTime::from_timestamp(ts_sec, 0)
            .unwrap_or_else(Utc::now)
            .to_rfc3339();

        let mut files = HashMap::new();

        // Find rollout jsonl file
        let mut actual_rollout_path = None;
        if !rollout_path_str.is_empty() {
            let p = PathBuf::from(&rollout_path_str);
            if p.exists() {
                actual_rollout_path = Some(p);
            } else {
                let rel = base_dir.join(&rollout_path_str);
                if rel.exists() {
                    actual_rollout_path = Some(rel);
                }
            }
        }

        // If not found from db, search ~/.codex/sessions for file matching session_id
        if actual_rollout_path.is_none() {
            let sessions_dir = base_dir.join("sessions");
            if sessions_dir.exists() {
                actual_rollout_path = Self::find_rollout_file(&sessions_dir, session_id);
            }
        }

        if let Some(rpath) = actual_rollout_path {
            if let Ok(content) = fs::read_to_string(&rpath) {
                let virtualized = if !cwd.is_empty() {
                    content.replace(&cwd, "${WORKSPACE_ROOT}")
                } else {
                    content
                };

                let rel_key = if let Ok(rel) = rpath.strip_prefix(&base_dir) {
                    rel.to_string_lossy().to_string()
                } else {
                    format!("sessions/rollout-{}.jsonl", session_id)
                };

                files.insert(rel_key, virtualized);
            }
        }

        Ok(SessionBundle {
            version: 1,
            agent_type: "codex".to_string(),
            id: session_id.to_string(),
            title: if title.is_empty() { "未命名线程".to_string() } else { title },
            preview,
            project_name,
            updated_at,
            metadata: meta_json,
            files,
        })
    }

    fn find_rollout_file(dir: &PathBuf, session_id: &str) -> Option<PathBuf> {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(found) = Self::find_rollout_file(&path, session_id) {
                        return Some(found);
                    }
                } else if path.is_file() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.contains(session_id) && name.ends_with(".jsonl") {
                        return Some(path);
                    }
                }
            }
        }
        None
    }

    pub fn restore_session(bundle: &SessionBundle) -> Result<(), String> {
        let base_dir = Self::get_base_dir().ok_or("Failed to locate home directory")?;

        // 1. Restore rollout files
        let mut restored_rollout_path = "".to_string();
        for (rel_path, content) in &bundle.files {
            let dest = base_dir.join(rel_path);
            if let Some(parent) = dest.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&dest, content);
            restored_rollout_path = dest.to_string_lossy().to_string();
        }

        // 2. Upsert into state_5.sqlite threads
        let db_path = base_dir.join("state_5.sqlite");
        if db_path.exists() {
            if let Ok(conn) = Connection::open(&db_path) {
                let meta = &bundle.metadata;
                let now_sec = Utc::now().timestamp();
                let created_at = meta.get("created_at").and_then(|v| v.as_i64()).unwrap_or(now_sec);
                let updated_at = meta.get("updated_at").and_then(|v| v.as_i64()).unwrap_or(now_sec);
                let source = meta.get("source").and_then(|v| v.as_str()).unwrap_or("desktop");
                let model_provider = meta.get("model_provider").and_then(|v| v.as_str()).unwrap_or("openai");
                let cwd = meta.get("cwd").and_then(|v| v.as_str()).unwrap_or("");
                let tokens_used = meta.get("tokens_used").and_then(|v| v.as_i64()).unwrap_or(0);
                let preview = meta.get("preview").and_then(|v| v.as_str()).unwrap_or(&bundle.preview);

                let rpath = if !restored_rollout_path.is_empty() {
                    restored_rollout_path
                } else {
                    meta.get("rollout_path").and_then(|v| v.as_str()).unwrap_or("").to_string()
                };

                let _ = conn.execute(
                    "INSERT INTO threads (
                        id, rollout_path, created_at, updated_at, source,
                        model_provider, cwd, title, tokens_used, preview,
                        sandbox_policy, approval_mode
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'full_access', 'manual')
                     ON CONFLICT(id) DO UPDATE SET
                        title=excluded.title,
                        rollout_path=excluded.rollout_path,
                        updated_at=excluded.updated_at,
                        tokens_used=excluded.tokens_used,
                        preview=excluded.preview",
                    rusqlite::params![
                        bundle.id,
                        rpath,
                        created_at,
                        updated_at,
                        source,
                        model_provider,
                        cwd,
                        bundle.title,
                        tokens_used,
                        preview,
                    ],
                );
            }
        }

        // 3. Upsert session_index.jsonl
        let index_path = base_dir.join("session_index.jsonl");
        let new_entry = serde_json::json!({
            "id": bundle.id,
            "thread_name": bundle.title,
            "updated_at": bundle.updated_at
        });

        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(index_path) {
            let _ = writeln!(file, "{}", new_entry);
        }

        Ok(())
    }
}
