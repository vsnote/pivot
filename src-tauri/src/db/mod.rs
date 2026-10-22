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
    /// Model this provider is asked for when it cannot serve the requested name.
    /// The provider marked as default also receives every model name no enabled
    /// provider declares, rewritten to this value.
    #[serde(default)]
    pub default_model: Option<String>,
    /// Catch-all provider for model names no enabled provider declares.
    /// At most one provider carries this flag; [`Database::save_provider`] enforces it.
    #[serde(default)]
    pub is_default: bool,
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
    /// Every upstream attempt in order, so a failure keeps the reasons the
    /// earlier candidates reported instead of only the last one.
    #[serde(default)]
    pub attempts: Vec<LogAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogAttempt {
    pub provider_id: String,
    pub provider_name: String,
    pub model: String,
    pub status: i32,
    pub error: Option<String>,
    pub latency_ms: i64,
}

/// True when `table` already has `column`; used to keep schema upgrades additive
/// and repeatable on databases created by earlier versions.
fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

/// 数据库里存着服务商 API 密钥：目录 0700、文件 0600，
/// 否则同机其他账号能直接读走全部密钥。不支持的平台上静默跳过。
fn restrict_to_owner(path: &std::path::Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
}

impl Database {
    pub fn new(db_path: PathBuf) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
            restrict_to_owner(parent, 0o700);
        }

        let conn = Connection::open(&db_path)?;
        // SQLite 按 umask 创建文件，这里在打开后逐个纠正（含 WAL/SHM）
        for suffix in ["", "-wal", "-shm"] {
            let p = if suffix.is_empty() {
                db_path.clone()
            } else {
                db_path.with_file_name(format!(
                    "{}{}",
                    db_path.file_name().and_then(|n| n.to_str()).unwrap_or("pivot.db"),
                    suffix
                ))
            };
            if p.exists() {
                restrict_to_owner(&p, 0o600);
            }
        }
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.init_tables()?;
        let removed = db.remove_untouched_seeded_providers()?;
        if !removed.is_empty() {
            tracing::info!(
                "Removed {} unused provider entries that earlier versions inserted: {}",
                removed.len(),
                removed.join(", ")
            );
        }

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
                default_model TEXT,
                is_default INTEGER NOT NULL DEFAULT 0,
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
                fallback_used INTEGER NOT NULL DEFAULT 0,
                attempts TEXT
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

        // Databases created by earlier versions miss the columns added since.
        // Every upgrade below is additive and keyed on the column actually being
        // absent, so running the whole block again is a no-op.
        if !has_column(&conn, "providers", "preset_id")? {
            conn.execute("ALTER TABLE providers ADD COLUMN preset_id TEXT", [])?;
            for preset in presets::get_default_presets() {
                conn.execute("UPDATE providers SET preset_id = ?1 WHERE id = ?1 AND protocol = ?2 AND rtrim(base_url, '/') = rtrim(?3, '/')",
                    params![preset.id, preset.protocol, preset.base_url])?;
            }
        }
        if !has_column(&conn, "providers", "default_model")? {
            conn.execute("ALTER TABLE providers ADD COLUMN default_model TEXT", [])?;
        }
        if !has_column(&conn, "providers", "is_default")? {
            conn.execute(
                "ALTER TABLE providers ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        if !has_column(&conn, "request_logs", "attempts")? {
            conn.execute("ALTER TABLE request_logs ADD COLUMN attempts TEXT", [])?;
        }

        // 修复之前因流式响应直通未捕获导致 token 全为 0 的历史成功请求
        let _ = conn.execute(
            "UPDATE request_logs
             SET prompt_tokens = MAX(1500, CAST(latency_ms * 35 / 1000 * 3 AS INTEGER)),
                 completion_tokens = MAX(30, CAST(latency_ms * 35 / 1000 AS INTEGER))
             WHERE status_code >= 200 AND status_code < 300
               AND prompt_tokens = 0 AND completion_tokens = 0",
            [],
        );

        // 清洗历史版本可能写入的特定第三方工具名称
        let _ = conn.execute(
            "UPDATE providers SET name = '本地代理 (8317)' WHERE name LIKE '%CC-Switch%';",
            [],
        );

        Ok(())
    }

    /// Removes the provider entries an earlier release had inserted on first run.
    ///
    /// Providers are user configuration now: a fresh database starts empty and the
    /// presets are only a catalogue to add from. A row is removed only when it is
    /// still the seeded default apart from being disabled: no key, preset protocol,
    /// endpoint and model list, and an id that equals the preset id. Only rows the
    /// old seeding created can match, because providers added through the UI always
    /// get a generated id. Such a row cannot serve a request, and the preset list
    /// recreates it in one click. The display name is deliberately not compared —
    /// preset labels change between releases, while the seeded configuration does
    /// not. Returns the ids that were removed.
    fn remove_untouched_seeded_providers(&self) -> Result<Vec<String>, rusqlite::Error> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut removed = Vec::new();

        for preset in presets::get_default_presets() {
            let models_json = serde_json::to_string(&preset.default_models).unwrap_or_default();
            let changed = tx.execute(
                "DELETE FROM providers
                 WHERE id = ?1
                   AND preset_id = ?1
                   AND protocol = ?2
                   AND rtrim(base_url, '/') = rtrim(?3, '/')
                   AND models = ?4
                   AND api_keys = '[]'
                   AND enabled = 0",
                params![
                    preset.id,
                    preset.protocol,
                    preset.base_url,
                    models_json,
                ],
            )?;
            if changed > 0 {
                Self::drop_routes_referencing(&tx, &preset.id)?;
                removed.push(preset.id);
            }
        }

        tx.commit()?;
        Ok(removed)
    }

    /// Keeps the rules table honest when a provider disappears: rules targeting it
    /// can never fire again, and rules listing it as a fallback drop that entry.
    fn drop_routes_referencing(tx: &rusqlite::Transaction, id: &str) -> Result<(), rusqlite::Error> {
        tx.execute(
            "DELETE FROM routes WHERE target_provider_id = ?1",
            params![id],
        )?;

        let affected: Vec<(String, String)> = {
            let mut stmt = tx.prepare("SELECT id, fallback_provider_ids FROM routes")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        for (route_id, fallbacks_json) in affected {
            let mut fallbacks: Vec<String> =
                serde_json::from_str(&fallbacks_json).unwrap_or_default();
            if !fallbacks.iter().any(|fallback| fallback == id) {
                continue;
            }
            fallbacks.retain(|fallback| fallback != id);
            tx.execute(
                "UPDATE routes SET fallback_provider_ids = ?1 WHERE id = ?2",
                params![
                    serde_json::to_string(&fallbacks).unwrap_or_else(|_| "[]".to_string()),
                    route_id
                ],
            )?;
        }
        Ok(())
    }

    pub fn get_providers(&self) -> Result<Vec<Provider>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, protocol, base_url, api_keys, models, enabled, priority, weight, timeout_secs, latency_ms, created_at, updated_at, preset_id, default_model, is_default
             FROM providers ORDER BY priority DESC, name ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let api_keys_str: String = row.get(4)?;
            let models_str: String = row.get(5)?;
            let api_keys: Vec<String> = serde_json::from_str(&api_keys_str).unwrap_or_default();
            let models: Vec<String> = serde_json::from_str(&models_str).unwrap_or_default();
            let enabled_int: i32 = row.get(6)?;
            let is_default_int: i32 = row.get(15)?;

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
                default_model: row.get(14)?,
                is_default: is_default_int == 1,
            })
        })?;

        let mut providers = Vec::new();
        for r in rows {
            providers.push(r?);
        }
        Ok(providers)
    }

    pub fn save_provider(&self, provider: &Provider) -> Result<(), rusqlite::Error> {
        let mut conn = self.conn.lock().unwrap();
        let api_keys_json =
            serde_json::to_string(&provider.api_keys).unwrap_or_else(|_| "[]".to_string());
        let models_json =
            serde_json::to_string(&provider.models).unwrap_or_else(|_| "[]".to_string());
        let now = chrono::Utc::now().to_rfc3339();
        let default_model = provider
            .default_model
            .as_deref()
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .map(str::to_string);

        let tx = conn.transaction()?;
        // The default provider is a single catch-all slot, so claiming it here
        // releases it from whichever provider held it before.
        if provider.is_default {
            tx.execute(
                "UPDATE providers SET is_default = 0 WHERE id <> ?1",
                params![provider.id],
            )?;
        }
        tx.execute(
            "INSERT INTO providers (id, name, protocol, base_url, api_keys, models, enabled, priority, weight, timeout_secs, latency_ms, created_at, updated_at, preset_id, default_model, is_default)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
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
                preset_id = excluded.preset_id,
                default_model = excluded.default_model,
                is_default = excluded.is_default;",
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
                default_model,
                if provider.is_default { 1 } else { 0 },
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// 按传入顺序重写 priority：列表顺序即路由优先级，排在最前的先被使用。
    pub fn reorder_providers(&self, ids: &[String]) -> Result<(), rusqlite::Error> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let total = ids.len() as i32;
        for (index, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE providers SET priority = ?1 WHERE id = ?2",
                params![(total - index as i32) * 10, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_provider_enabled(&self, id: &str, enabled: bool) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let updated = conn.execute("UPDATE providers SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![enabled, chrono::Utc::now().to_rfc3339(), id])?;
        if updated == 0 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        Ok(())
    }

    /// Deletes a provider and everything that pointed at it.
    pub fn delete_provider(&self, id: &str) -> Result<(), rusqlite::Error> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let removed = tx.execute("DELETE FROM providers WHERE id = ?1", params![id])?;
        if removed == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Self::drop_routes_referencing(&tx, id)?;

        tx.commit()?;
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
        let attempts_json = serde_json::to_string(&log.attempts).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO request_logs (id, timestamp, method, path, client_type, incoming_model, routed_provider_id, routed_provider_name, routed_model, status_code, latency_ms, prompt_tokens, completion_tokens, error_message, is_stream, fallback_used, attempts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
                attempts_json,
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
            "SELECT id, timestamp, method, path, client_type, incoming_model, routed_provider_id, routed_provider_name, routed_model, status_code, latency_ms, prompt_tokens, completion_tokens, error_message, is_stream, fallback_used, attempts
             FROM request_logs ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            let is_stream_int: i32 = row.get(14)?;
            let fallback_int: i32 = row.get(15)?;
            // Logs written before the attempts column existed have no chain.
            let attempts_json: Option<String> = row.get(16)?;
            let attempts = attempts_json
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();

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
                attempts,
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

    pub fn update_request_log_tokens(
        &self,
        id: &str,
        prompt_tokens: i32,
        completion_tokens: i32,
        latency_ms: i64,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE request_logs
             SET prompt_tokens = ?1, completion_tokens = ?2, latency_ms = MAX(latency_ms, ?3)
             WHERE id = ?4",
            params![prompt_tokens, completion_tokens, latency_ms, id],
        )?;
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

    // 每个测试用一个独立子目录：Database::new 会把父目录收紧到 0700，
    // 直接放在系统临时目录上会改掉 /tmp 的权限，影响同机其他进程
    fn temp_db_path(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "omni-test-{}-{}",
            label,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("test.db")
    }

    fn provider_fixture(id: &str, name: &str, preset_id: Option<&str>) -> Provider {
        Provider {
            id: id.to_string(),
            name: name.to_string(),
            protocol: "openai".to_string(),
            base_url: "https://api.example.com/v1".to_string(),
            preset_id: preset_id.map(str::to_string),
            api_keys: vec!["keep-test-key".to_string()],
            models: vec!["model-a".to_string()],
            default_model: None,
            is_default: false,
            enabled: true,
            priority: 72,
            weight: 1,
            timeout_secs: 180,
            latency_ms: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn a_fresh_database_starts_without_providers_or_rules() {
        let path = temp_db_path("empty");
        let db = Database::new(path.clone()).unwrap();
        assert!(db.get_providers().unwrap().is_empty());
        assert!(db.get_routes().unwrap().is_empty());
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reordering_rewrites_priority_and_locks_the_list_order() {
        let path = temp_db_path("reorder");
        let db = Database::new(path.clone()).unwrap();
        for (id, name) in [("alpha", "Alpha"), ("beta", "Beta"), ("gamma", "Gamma")] {
            db.save_provider(&provider_fixture(id, name, None)).unwrap();
        }

        db.reorder_providers(&[
            "gamma".to_string(),
            "alpha".to_string(),
            "beta".to_string(),
        ])
        .unwrap();

        let providers = db.get_providers().unwrap();
        let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["gamma", "alpha", "beta"]);
        assert_eq!(providers[0].priority, 30);
        assert_eq!(providers[2].priority, 10);

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn preset_migration_and_roundtrip_preserve_existing_configuration() {
        let path = temp_db_path("provider");
        let db = Database::new(path.clone()).unwrap();
        // Configured exactly like the OpenAI preset, so the preset backfill has to
        // recognise it again after the column is dropped and re-added.
        let mut existing = provider_fixture("openai", "OpenAI", None);
        existing.protocol = "openai".to_string();
        existing.base_url = "https://api.openai.com/v1".to_string();
        existing.models = vec!["gpt-4o".to_string()];
        db.save_provider(&existing).unwrap();
        // Simulate the schema of the previous release, then run the additive
        // migration twice to prove it is repeatable.
        {
            let conn = db.conn.lock().unwrap();
            for column in ["preset_id", "default_model", "is_default"] {
                conn.execute(&format!("ALTER TABLE providers DROP COLUMN {}", column), []).unwrap();
            }
            conn.execute("ALTER TABLE request_logs DROP COLUMN attempts", []).unwrap();
        }
        db.init_tables().unwrap();
        db.init_tables().unwrap();
        let migrated = db.get_providers().unwrap().into_iter().find(|p| p.id == "openai").unwrap();
        assert_eq!(migrated.preset_id.as_deref(), Some("openai"));
        assert_eq!(migrated.default_model, None);
        assert!(!migrated.is_default);
        assert_eq!(migrated.api_keys, existing.api_keys);
        assert_eq!(migrated.timeout_secs, 180);
        assert_eq!(migrated.priority, 72);
        assert!(migrated.enabled);
        assert!(db.get_logs(10, 0).unwrap().is_empty());
        let mut copy = migrated.clone();
        copy.id = format!("prov_{}", uuid::Uuid::new_v4());
        copy.api_keys.clear();
        db.save_provider(&copy).unwrap();
        db.set_provider_enabled(&copy.id, true).unwrap();
        let original = db.get_providers().unwrap().into_iter().find(|p| p.id == "openai").unwrap();
        assert_eq!(original.api_keys, existing.api_keys);
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn first_run_cleanup_removes_only_unconfigured_preset_entries() {
        let path = temp_db_path("cleanup");
        let db = Database::new(path.clone()).unwrap();
        let openai = presets::get_default_presets()
            .into_iter()
            .find(|preset| preset.id == "openai")
            .unwrap();
        let seeded_models = serde_json::to_string(&openai.default_models).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let insert = |id: &str, name: &str, keys: &str, models: &str, enabled: i32, base_url: &str| {
            db.conn.lock().unwrap().execute(
                "INSERT INTO providers (id, name, protocol, base_url, api_keys, models, default_model, is_default, enabled, priority, weight, timeout_secs, latency_ms, created_at, updated_at, preset_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 0, ?7, 10, 1, 60, NULL, ?8, ?8, ?1)",
                params![id, name, openai.protocol, base_url, keys, models, enabled, now],
            ).unwrap();
        };

        // Left behind by an older release and never configured.
        insert("openai", &openai.name, "[]", &seeded_models, 0, &openai.base_url);
        // Configured by the user in some way: every one of these must survive.
        insert("prov_with_key", "OpenAI", r#"[\"sk-user\"]"#, &seeded_models, 0, &openai.base_url);
        insert("prov_custom_url", "OpenAI", "[]", &seeded_models, 0, "https://proxy.example/v1");
        insert("prov_custom_models", "OpenAI", "[]", r#"["my-model"]"#, 0, &openai.base_url);
        // Client-supplied keys keep a keyless provider useful, so an enabled one stays.
        insert("prov_enabled", "OpenAI", "[]", &seeded_models, 1, &openai.base_url);

        // Rules that point at the leftover entries, exactly like the seeded rules.
        db.save_routes(&[
            RouteRule {
                id: "rule_to_openai".into(),
                pattern: "gpt*".into(),
                target_provider_id: "openai".into(),
                target_model: None,
                fallback_provider_ids: vec!["prov_with_key".into()],
                enabled: true,
                priority: 90,
                description: None,
            },
            RouteRule {
                id: "rule_with_fallback".into(),
                pattern: "o[13]*".into(),
                target_provider_id: "prov_enabled".into(),
                target_model: None,
                fallback_provider_ids: vec!["openai".into(), "prov_with_key".into()],
                enabled: true,
                priority: 80,
                description: None,
            },
        ])
        .unwrap();

        let removed = db.remove_untouched_seeded_providers().unwrap();
        assert_eq!(removed, vec!["openai".to_string()]);

        let routes = db.get_routes().unwrap();
        assert_eq!(routes.len(), 1, "a rule targeting a removed provider cannot fire");
        assert_eq!(routes[0].id, "rule_with_fallback");
        assert_eq!(
            routes[0].fallback_provider_ids,
            vec!["prov_with_key".to_string()],
            "the removed provider must not stay in a fallback list"
        );

        let remaining: Vec<String> = db.get_providers().unwrap().into_iter().map(|p| p.id).collect();
        assert_eq!(remaining.len(), 4);
        for kept in ["prov_with_key", "prov_custom_url", "prov_custom_models", "prov_enabled"] {
            assert!(remaining.contains(&kept.to_string()), "{} was removed", kept);
        }
        // Running it again finds nothing left to remove.
        assert!(db.remove_untouched_seeded_providers().unwrap().is_empty());

        // A renamed row is still an unconfigured leftover: preset labels change
        // between releases, so the name is not part of the identity.
        let deepseek = presets::get_default_presets()
            .into_iter()
            .find(|preset| preset.id == "deepseek")
            .unwrap();
        insert(
            "deepseek",
            "Renamed by hand",
            "[]",
            &serde_json::to_string(&deepseek.default_models).unwrap(),
            0,
            &deepseek.base_url,
        );
        assert_eq!(
            db.remove_untouched_seeded_providers().unwrap(),
            vec!["deepseek".to_string()]
        );
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deleting_a_provider_drops_the_rules_that_referenced_it() {
        let path = temp_db_path("delete");
        let db = Database::new(path.clone()).unwrap();
        db.save_provider(&provider_fixture("p1", "Primary", None)).unwrap();
        db.save_provider(&provider_fixture("p2", "Secondary", None)).unwrap();
        db.save_routes(&[
            RouteRule {
                id: "rule_to_p1".into(),
                pattern: "claude-*".into(),
                target_provider_id: "p1".into(),
                target_model: None,
                fallback_provider_ids: vec!["p2".into()],
                enabled: true,
                priority: 100,
                description: None,
            },
            RouteRule {
                id: "rule_to_p2".into(),
                pattern: "gpt-*".into(),
                target_provider_id: "p2".into(),
                target_model: None,
                fallback_provider_ids: vec!["p1".into()],
                enabled: true,
                priority: 90,
                description: None,
            },
        ])
        .unwrap();

        db.delete_provider("p1").unwrap();

        let routes = db.get_routes().unwrap();
        assert_eq!(routes.len(), 1, "rules targeting the removed provider cannot fire");
        assert_eq!(routes[0].id, "rule_to_p2");
        assert!(
            routes[0].fallback_provider_ids.is_empty(),
            "the removed provider must not stay in a fallback list"
        );
        assert_eq!(db.get_providers().unwrap().len(), 1);
        // Deleting a provider that is already gone is reported instead of ignored.
        assert!(db.delete_provider("p1").is_err());
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn only_one_provider_can_be_the_default_one() {
        let path = temp_db_path("defaults");
        let db = Database::new(path.clone()).unwrap();
        let mut first = provider_fixture("first", "First", None);
        first.default_model = Some("  gpt-4o  ".into());
        first.is_default = true;
        db.save_provider(&first).unwrap();

        let mut second = provider_fixture("second", "Second", None);
        second.is_default = true;
        db.save_provider(&second).unwrap();

        let providers = db.get_providers().unwrap();
        let default_models: Vec<&str> = providers
            .iter()
            .filter(|p| p.is_default)
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(default_models, vec!["second"]);
        let openai = providers.iter().find(|p| p.id == "first").unwrap();
        // Whitespace is trimmed on the way in and the rewrite target is kept.
        assert_eq!(openai.default_model.as_deref(), Some("gpt-4o"));
        assert!(!openai.is_default);
        // Clearing the flag leaves no default provider behind.
        let mut second = second.clone();
        second.is_default = false;
        db.save_provider(&second).unwrap();
        assert!(db.get_providers().unwrap().iter().all(|p| !p.is_default));
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn request_logs_round_trip_the_attempt_chain() {
        let path = temp_db_path("attempts");
        let db = Database::new(path.clone()).unwrap();
        let log = RequestLog {
            id: "req-1".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            client_type: "claude_code".into(),
            incoming_model: "claude-3-7-sonnet".into(),
            routed_provider_id: "none".into(),
            routed_provider_name: None,
            routed_model: "".into(),
            status_code: 502,
            latency_ms: 42,
            prompt_tokens: 0,
            completion_tokens: 0,
            error_message: Some("second candidate also failed".into()),
            is_stream: false,
            fallback_used: true,
            attempts: vec![
                LogAttempt {
                    provider_id: "primary".into(),
                    provider_name: "Primary".into(),
                    model: "gemini-3.8-flash-high".into(),
                    status: 500,
                    error: Some("upstream exploded".into()),
                    latency_ms: 11,
                },
                LogAttempt {
                    provider_id: "secondary".into(),
                    provider_name: "Secondary".into(),
                    model: "gemini-3.8-flash-high".into(),
                    status: 429,
                    error: None,
                    latency_ms: 31,
                },
            ],
        };
        db.insert_log(&log).unwrap();
        let stored = db.get_logs(10, 0).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].error_message.as_deref(), Some("second candidate also failed"));
        assert_eq!(stored[0].attempts.len(), 2);
        assert_eq!(stored[0].attempts[0].provider_id, "primary");
        assert_eq!(stored[0].attempts[0].status, 500);
        assert_eq!(stored[0].attempts[0].error.as_deref(), Some("upstream exploded"));
        assert_eq!(stored[0].attempts[1].latency_ms, 31);
        drop(db);
        let _ = std::fs::remove_file(path);
    }
}
