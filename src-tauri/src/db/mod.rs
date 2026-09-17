pub mod presets;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub preset_id: Option<String>,
    pub api_keys: Vec<String>,
    pub models: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub weight: i32,
    pub timeout_secs: i32,
    pub latency_ms: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteRule {
    pub id: String,
    pub pattern: String,
    pub target_provider_id: String,
    pub target_model: Option<String>,
    pub fallback_provider_ids: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLog {
    pub id: String,
    pub timestamp: String,
    pub method: String,
    pub path: String,
    pub client_type: String,
    pub incoming_model: String,
    pub routed_provider_id: String,
    pub routed_provider_name: Option<String>,
    pub routed_model: String,
    pub status_code: i32,
    pub latency_ms: i64,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub error_message: Option<String>,
    pub is_stream: bool,
    pub fallback_used: bool,
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(db_path: PathBuf) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.init_tables()?;
        db.seed_defaults_if_empty()?;

        Ok(db)
    }

    fn init_tables(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS providers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                protocol TEXT NOT NULL,
                base_url TEXT NOT NULL,
                api_keys TEXT NOT NULL,
                models TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                priority INTEGER NOT NULL DEFAULT 0,
                weight INTEGER NOT NULL DEFAULT 1,
                timeout_secs INTEGER NOT NULL DEFAULT 60,
                latency_ms INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS routes (
                id TEXT PRIMARY KEY,
                pattern TEXT NOT NULL,
                target_provider_id TEXT NOT NULL,
                target_model TEXT,
                fallback_provider_ids TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                priority INTEGER NOT NULL DEFAULT 0,
                description TEXT
            );

            CREATE TABLE IF NOT EXISTS request_logs (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                method TEXT NOT NULL,
                path TEXT NOT NULL,
                client_type TEXT NOT NULL,
                incoming_model TEXT NOT NULL,
                routed_provider_id TEXT NOT NULL,
                routed_provider_name TEXT,
                routed_model TEXT NOT NULL,
                status_code INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL,
                prompt_tokens INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                error_message TEXT,
                is_stream INTEGER NOT NULL DEFAULT 0,
                fallback_used INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON request_logs(timestamp DESC);

            CREATE TABLE IF NOT EXISTS interceptor_state (
                client_id TEXT PRIMARY KEY,
                is_intercepted INTEGER NOT NULL DEFAULT 0,
                backup_config TEXT,
                original_endpoint TEXT,
                updated_at TEXT NOT NULL
            );",
        )?;
        let has_preset_id = conn.prepare("PRAGMA table_info(providers)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?.iter().any(|name| name == "preset_id");
        if !has_preset_id {
            conn.execute("ALTER TABLE providers ADD COLUMN preset_id TEXT", [])?;
            for preset in presets::get_default_presets() {
                conn.execute("UPDATE providers SET preset_id = ?1 WHERE id = ?1 AND protocol = ?2 AND rtrim(base_url, '/') = rtrim(?3, '/')",
                    params![preset.id, preset.protocol, preset.base_url])?;
            }
        }
        Ok(())
    }

    fn seed_defaults_if_empty(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let provider_count: i64 =
            conn.query_row("SELECT count(*) FROM providers", [], |r| r.get(0))?;

        if provider_count == 0 {
            let now = chrono::Utc::now().to_rfc3339();
            // Seed default Anthropic, OpenAI, DeepSeek, OpenRouter, and Ollama providers
            let presets = presets::get_default_presets();
            for preset in presets.iter().take(6) {
                let api_keys_json = serde_json::to_string(&Vec::<String>::new()).unwrap();
                let models_json = serde_json::to_string(&preset.default_models).unwrap();
                let enabled = if preset.id == "anthropic" || preset.id == "openai" {
                    1
                } else {
                    0
                };

                conn.execute(
                    "INSERT INTO providers (id, name, protocol, base_url, api_keys, models, enabled, priority, weight, timeout_secs, latency_ms, created_at, updated_at, preset_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, ?11, ?12, ?1)",
                    params![
                        preset.id,
                        preset.name,
                        preset.protocol,
                        preset.base_url,
                        api_keys_json,
                        models_json,
                        enabled,
                        10,
                        1,
                        60,
                        now,
                        now,
                    ],
                )?;
            }

            // Seed default routing rules
            let default_routes = vec![
                RouteRule {
                    id: "route_claude".to_string(),
                    pattern: "claude*".to_string(),
                    target_provider_id: "anthropic".to_string(),
                    target_model: None,
                    fallback_provider_ids: vec!["openrouter".to_string(), "deepseek".to_string()],
                    enabled: true,
                    priority: 100,
                    description: Some(
                        "Dispatch Claude models to Anthropic with OpenRouter fallback".to_string(),
                    ),
                },
                RouteRule {
                    id: "route_gpt".to_string(),
                    pattern: "gpt*".to_string(),
                    target_provider_id: "openai".to_string(),
                    target_model: None,
                    fallback_provider_ids: vec!["openrouter".to_string()],
                    enabled: true,
                    priority: 90,
                    description: Some("Dispatch GPT models to OpenAI".to_string()),
                },
                RouteRule {
                    id: "route_o_series".to_string(),
                    pattern: "o[13]*".to_string(),
                    target_provider_id: "openai".to_string(),
                    target_model: None,
                    fallback_provider_ids: vec!["openrouter".to_string()],
                    enabled: true,
                    priority: 90,
                    description: Some("Dispatch o1 and o3 models to OpenAI".to_string()),
                },
                RouteRule {
                    id: "route_deepseek".to_string(),
                    pattern: "deepseek*".to_string(),
                    target_provider_id: "deepseek".to_string(),
                    target_model: None,
                    fallback_provider_ids: vec!["openrouter".to_string()],
                    enabled: true,
                    priority: 80,
                    description: Some(
                        "Dispatch DeepSeek requests to DeepSeek official".to_string(),
                    ),
                },
                RouteRule {
                    id: "route_gemini".to_string(),
                    pattern: "gemini*".to_string(),
                    target_provider_id: "gemini".to_string(),
                    target_model: None,
                    fallback_provider_ids: vec!["openrouter".to_string()],
                    enabled: true,
                    priority: 70,
                    description: Some("Dispatch Gemini requests to Google Gemini".to_string()),
                },
                RouteRule {
                    id: "route_fallback".to_string(),
                    pattern: "*".to_string(),
                    target_provider_id: "anthropic".to_string(),
                    target_model: None,
                    fallback_provider_ids: vec!["openai".to_string(), "openrouter".to_string()],
                    enabled: true,
                    priority: 0,
                    description: Some("Catch-all default route".to_string()),
                },
            ];

            for route in default_routes {
                let fallbacks_json = serde_json::to_string(&route.fallback_provider_ids).unwrap();
                conn.execute(
                    "INSERT INTO routes (id, pattern, target_provider_id, target_model, fallback_provider_ids, enabled, priority, description)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        route.id,
                        route.pattern,
                        route.target_provider_id,
                        route.target_model,
                        fallbacks_json,
                        if route.enabled { 1 } else { 0 },
                        route.priority,
                        route.description,
                    ],
                )?;
            }
        }
        Ok(())
    }

    pub fn get_providers(&self) -> Result<Vec<Provider>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, protocol, base_url, api_keys, models, enabled, priority, weight, timeout_secs, latency_ms, created_at, updated_at, preset_id
             FROM providers ORDER BY priority DESC, name ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let api_keys_str: String = row.get(4)?;
            let models_str: String = row.get(5)?;
            let api_keys: Vec<String> = serde_json::from_str(&api_keys_str).unwrap_or_default();
            let models: Vec<String> = serde_json::from_str(&models_str).unwrap_or_default();
            let enabled_int: i32 = row.get(6)?;

            Ok(Provider {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol: row.get(2)?,
                base_url: row.get(3)?,
                api_keys,
                models,
                enabled: enabled_int == 1,
                priority: row.get(7)?,
                weight: row.get(8)?,
                timeout_secs: row.get(9)?,
                latency_ms: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
                preset_id: row.get(13)?,
            })
        })?;

        let mut providers = Vec::new();
        for r in rows {
            providers.push(r?);
        }
        Ok(providers)
    }

    pub fn save_provider(&self, provider: &Provider) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let api_keys_json =
            serde_json::to_string(&provider.api_keys).unwrap_or_else(|_| "[]".to_string());
        let models_json =
            serde_json::to_string(&provider.models).unwrap_or_else(|_| "[]".to_string());
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO providers (id, name, protocol, base_url, api_keys, models, enabled, priority, weight, timeout_secs, latency_ms, created_at, updated_at, preset_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                protocol = excluded.protocol,
                base_url = excluded.base_url,
                api_keys = excluded.api_keys,
                models = excluded.models,
                enabled = excluded.enabled,
                priority = excluded.priority,
                weight = excluded.weight,
                timeout_secs = excluded.timeout_secs,
                latency_ms = excluded.latency_ms,
                updated_at = excluded.updated_at,
                preset_id = excluded.preset_id;",
            params![
                provider.id,
                provider.name,
                provider.protocol,
                provider.base_url,
                api_keys_json,
                models_json,
                if provider.enabled { 1 } else { 0 },
                provider.priority,
                provider.weight,
                provider.timeout_secs,
                provider.latency_ms,
                provider.created_at.clone().unwrap_or_else(|| now.clone()),
                now,
                provider.preset_id,
            ],
        )?;
        Ok(())
    }

    pub fn set_provider_enabled(&self, id: &str, enabled: bool) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let updated = conn.execute("UPDATE providers SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![enabled, chrono::Utc::now().to_rfc3339(), id])?;
        if updated == 0 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        Ok(())
    }

    pub fn delete_provider(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM providers WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update_provider_latency(
        &self,
        id: &str,
        latency_ms: i64,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE providers SET latency_ms = ?1 WHERE id = ?2",
            params![latency_ms, id],
        )?;
        Ok(())
    }

    pub fn get_routes(&self) -> Result<Vec<RouteRule>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, pattern, target_provider_id, target_model, fallback_provider_ids, enabled, priority, description
             FROM routes ORDER BY priority DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let fallbacks_str: String = row.get(4)?;
            let fallback_provider_ids: Vec<String> =
                serde_json::from_str(&fallbacks_str).unwrap_or_default();
            let enabled_int: i32 = row.get(5)?;

            Ok(RouteRule {
                id: row.get(0)?,
                pattern: row.get(1)?,
                target_provider_id: row.get(2)?,
                target_model: row.get(3)?,
                fallback_provider_ids,
                enabled: enabled_int == 1,
                priority: row.get(6)?,
                description: row.get(7)?,
            })
        })?;

        let mut routes = Vec::new();
        for r in rows {
            routes.push(r?);
        }
        Ok(routes)
    }

    pub fn save_routes(&self, routes: &[RouteRule]) -> Result<(), rusqlite::Error> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM routes", [])?;

        for route in routes {
            let fallbacks_json = serde_json::to_string(&route.fallback_provider_ids)
                .unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "INSERT INTO routes (id, pattern, target_provider_id, target_model, fallback_provider_ids, enabled, priority, description)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    route.id,
                    route.pattern,
                    route.target_provider_id,
                    route.target_model,
                    fallbacks_json,
                    if route.enabled { 1 } else { 0 },
                    route.priority,
                    route.description,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_log(&self, log: &RequestLog) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO request_logs (id, timestamp, method, path, client_type, incoming_model, routed_provider_id, routed_provider_name, routed_model, status_code, latency_ms, prompt_tokens, completion_tokens, error_message, is_stream, fallback_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                log.id,
                log.timestamp,
                log.method,
                log.path,
                log.client_type,
                log.incoming_model,
                log.routed_provider_id,
                log.routed_provider_name,
                log.routed_model,
                log.status_code,
                log.latency_ms,
                log.prompt_tokens,
                log.completion_tokens,
                log.error_message,
                if log.is_stream { 1 } else { 0 },
                if log.fallback_used { 1 } else { 0 },
            ],
        )?;
        Ok(())
    }

    pub fn get_logs(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<RequestLog>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, method, path, client_type, incoming_model, routed_provider_id, routed_provider_name, routed_model, status_code, latency_ms, prompt_tokens, completion_tokens, error_message, is_stream, fallback_used
             FROM request_logs ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            let is_stream_int: i32 = row.get(14)?;
            let fallback_int: i32 = row.get(15)?;

            Ok(RequestLog {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                method: row.get(2)?,
                path: row.get(3)?,
                client_type: row.get(4)?,
                incoming_model: row.get(5)?,
                routed_provider_id: row.get(6)?,
                routed_provider_name: row.get(7)?,
                routed_model: row.get(8)?,
                status_code: row.get(9)?,
                latency_ms: row.get(10)?,
                prompt_tokens: row.get(11)?,
                completion_tokens: row.get(12)?,
                error_message: row.get(13)?,
                is_stream: is_stream_int == 1,
                fallback_used: fallback_int == 1,
            })
        })?;

        let mut logs = Vec::new();
        for r in rows {
            logs.push(r?);
        }
        Ok(logs)
    }

    pub fn clear_logs(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM request_logs", [])?;
        Ok(())
    }

    pub fn save_interceptor_state(
        &self,
        client_id: &str,
        is_intercepted: bool,
        backup: Option<&str>,
        original_endpoint: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO interceptor_state (client_id, is_intercepted, backup_config, original_endpoint, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(client_id) DO UPDATE SET
                is_intercepted = excluded.is_intercepted,
                backup_config = excluded.backup_config,
                original_endpoint = excluded.original_endpoint,
                updated_at = excluded.updated_at",
            params![
                client_id,
                if is_intercepted { 1 } else { 0 },
                backup,
                original_endpoint,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn get_interceptor_state(
        &self,
        client_id: &str,
    ) -> Result<Option<(bool, Option<String>, Option<String>)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT is_intercepted, backup_config, original_endpoint FROM interceptor_state WHERE client_id = ?1")?;
        let mut rows = stmt.query(params![client_id])?;

        if let Some(row) = rows.next()? {
            let intercepted_int: i32 = row.get(0)?;
            let backup: Option<String> = row.get(1)?;
            let endpoint: Option<String> = row.get(2)?;
            Ok(Some((intercepted_int == 1, backup, endpoint)))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    #[test]
    fn preset_migration_and_roundtrip_preserve_existing_configuration() {
        let path = std::env::temp_dir().join(format!("omni-provider-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(path.clone()).unwrap();
        let mut existing = db.get_providers().unwrap().into_iter().find(|p| p.id == "openai").unwrap();
        existing.api_keys = vec!["keep-test-key".into()];
        existing.priority = 72;
        existing.timeout_secs = 180;
        existing.enabled = false;
        db.save_provider(&existing).unwrap();
        // Simulate the previous schema, then run the additive migration twice.
        db.conn.lock().unwrap().execute("ALTER TABLE providers DROP COLUMN preset_id", []).unwrap();
        db.init_tables().unwrap();
        db.init_tables().unwrap();
        let migrated = db.get_providers().unwrap().into_iter().find(|p| p.id == "openai").unwrap();
        assert_eq!(migrated.preset_id.as_deref(), Some("openai"));
        assert_eq!(migrated.api_keys, existing.api_keys);
        assert_eq!(migrated.timeout_secs, 180);
        assert_eq!(migrated.priority, 72);
        assert!(!migrated.enabled);
        let mut copy = migrated.clone();
        copy.id = format!("prov_{}", uuid::Uuid::new_v4());
        copy.api_keys.clear();
        db.save_provider(&copy).unwrap();
        db.set_provider_enabled(&copy.id, true).unwrap();
        let original = db.get_providers().unwrap().into_iter().find(|p| p.id == "openai").unwrap();
        assert_eq!(original.api_keys, existing.api_keys);
        assert!(!original.enabled);
        drop(db);
        let _ = std::fs::remove_file(path);
    }
}
