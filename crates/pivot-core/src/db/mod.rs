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
    #[serde(default)]
    pub auth_type: Option<String>,
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
pub struct SubscriptionAccount {
    pub id: String,
    pub provider_id: String,
    pub account_type: String,
    pub email: Option<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64,
    pub cooldown_until: i64,
    pub enabled: bool,
    #[serde(default)]
    pub plan_type: Option<String>,
    #[serde(default)]
    pub supported_models: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsBucketPlan {
    pub start_epoch: i64,
    pub step_secs: i64,
    pub count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteModelSlice {
    pub provider_id: String,
    pub provider_name: Option<String>,
    pub model: String,
    pub calls: usize,
    pub ok_calls: usize,
    pub err_calls: usize,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBreakdown {
    pub unrouted: usize,
    pub rate_limited_429: usize,
    pub auth_401_403: usize,
    pub not_found_404: usize,
    pub bad_request_4xx: usize,
    pub upstream_timeout_502_504: usize,
    pub upstream_500: usize,
    pub other_err: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendBucket {
    pub start_epoch: i64,
    pub end_epoch: i64,
    pub ok_calls: usize,
    pub err_calls: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardRawStats {
    pub has_any_logs: bool,
    pub total_reqs: usize,
    pub ok_2xx: usize,
    pub err_4xx: usize,
    pub err_5xx: usize,
    pub unrouted_reqs: usize,
    pub fallback_count: usize,
    pub ok_latency_sum_ms: i64,
    pub ok_latency_samples: usize,
    pub earliest_epoch: Option<i64>,
    pub route_model_slices: Vec<RouteModelSlice>,
    pub client_counts: Vec<(String, usize)>,
    pub errors: ErrorBreakdown,
    pub buckets: Vec<TrendBucket>,
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
                timeout_secs INTEGER NOT NULL DEFAULT 300,
                latency_ms INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
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

            
            CREATE TABLE IF NOT EXISTS subscription_accounts (
                id TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL,
                account_type TEXT NOT NULL,
                email TEXT,
                access_token TEXT NOT NULL,
                refresh_token TEXT,
                expires_at INTEGER NOT NULL,
                cooldown_until INTEGER NOT NULL DEFAULT 0,
                enabled INTEGER NOT NULL DEFAULT 1,
                plan_type TEXT,
                supported_models TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sub_accounts_provider ON subscription_accounts(provider_id);
            CREATE TABLE IF NOT EXISTS interceptor_state (
                client_id TEXT PRIMARY KEY,
                is_intercepted INTEGER NOT NULL DEFAULT 0,
                backup_config TEXT,
                original_endpoint TEXT,
                model TEXT,
                updated_at TEXT NOT NULL
            );",
        )?;

        // Databases created by earlier versions miss the columns added since.
        // Every upgrade below is additive and keyed on the column actually being
        // absent, so running the whole block again is a no-op.
        
        if !has_column(&conn, "providers", "auth_type")? {
            conn.execute("ALTER TABLE providers ADD COLUMN auth_type TEXT", [])?;
        }
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
        if !has_column(&conn, "interceptor_state", "model")? {
            conn.execute("ALTER TABLE interceptor_state ADD COLUMN model TEXT", [])?;
        }
        if !has_column(&conn, "subscription_accounts", "plan_type")? {
            conn.execute("ALTER TABLE subscription_accounts ADD COLUMN plan_type TEXT", [])?;
        }
        if !has_column(&conn, "subscription_accounts", "supported_models")? {
            conn.execute(
                "ALTER TABLE subscription_accounts ADD COLUMN supported_models TEXT",
                [],
            )?;
        }

        // 清理早期版本按延迟公式伪造写入的虚假 Token 估算值，还原为真实 0 值
        let _ = conn.execute(
            "UPDATE request_logs
             SET prompt_tokens = 0,
                 completion_tokens = 0
             WHERE status_code >= 200 AND status_code < 300
               AND prompt_tokens = MAX(1500, CAST(latency_ms * 35 / 1000 * 3 AS INTEGER))
               AND completion_tokens = MAX(30, CAST(latency_ms * 35 / 1000 AS INTEGER))",
            [],
        );

        // 确保历史遗留 60s 默认超时的服务商提升至 300 秒，避免大上下文与深度思考模型（常需 200s+）被中途掐断
        let _ = conn.execute(
            "UPDATE providers SET timeout_secs = 300 WHERE timeout_secs < 180",
            [],
        );

        // 清洗历史版本可能写入的特定第三方工具名称
        let _ = conn.execute(
            "UPDATE providers SET name = '本地代理 (8317)' WHERE name LIKE '%CC-Switch%';",
            [],
        );

        // 清理因历史删除服务商未级联清理而遗留的孤立订阅账号
        let _ = conn.execute(
            "DELETE FROM subscription_accounts WHERE provider_id NOT IN (SELECT id FROM providers);",
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
                removed.push(preset.id);
            }
        }

        tx.commit()?;
        Ok(removed)
    }

    pub fn get_providers(&self) -> Result<Vec<Provider>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, protocol, base_url, api_keys, models, enabled, priority, weight, timeout_secs, latency_ms, created_at, updated_at, preset_id, default_model, is_default, auth_type
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
                latency_ms: None,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
                preset_id: row.get(13)?,
                default_model: row.get(14)?,
                is_default: is_default_int == 1,
                auth_type: row.get(16)?,
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

        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO providers (id, name, protocol, base_url, api_keys, models, enabled, priority, weight, timeout_secs, latency_ms, created_at, updated_at, preset_id, default_model, is_default, auth_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
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
                default_model = NULL,
                is_default = 0,
                auth_type = excluded.auth_type;",
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
                Option::<String>::None,
                0,
                provider.auth_type.clone(),
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

    /// Deletes a provider by id.
    pub fn delete_provider(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let removed = conn.execute("DELETE FROM providers WHERE id = ?1", params![id])?;
        if removed == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        let _ = conn.execute(
            "DELETE FROM subscription_accounts WHERE provider_id = ?1",
            params![id],
        );
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

    fn map_request_log_row(row: &rusqlite::Row<'_>) -> Result<RequestLog, rusqlite::Error> {
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

        let rows = stmt.query_map(
            params![limit as i64, offset as i64],
            Self::map_request_log_row,
        )?;

        let mut logs = Vec::new();
        for r in rows {
            logs.push(r?);
        }
        Ok(logs)
    }

    pub fn get_logs_in_window(
        &self,
        since_epoch: Option<i64>,
        errors_only: bool,
        limit: usize,
    ) -> Result<Vec<RequestLog>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, method, path, client_type, incoming_model, routed_provider_id, routed_provider_name, routed_model, status_code, latency_ms, prompt_tokens, completion_tokens, error_message, is_stream, fallback_used, attempts
             FROM request_logs
             WHERE (?1 IS NULL OR unixepoch(timestamp) >= ?1)
               AND (?2 = 0 OR status_code < 200 OR status_code >= 300)
             ORDER BY timestamp DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![since_epoch, if errors_only { 1 } else { 0 }, limit as i64],
            Self::map_request_log_row,
        )?;
        let mut logs = Vec::new();
        for r in rows {
            logs.push(r?);
        }
        Ok(logs)
    }

    pub fn get_dashboard_stats(
        &self,
        since_epoch: Option<i64>,
        fixed_buckets: Option<StatsBucketPlan>,
        today_start_epoch: i64,
    ) -> Result<DashboardRawStats, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let (any_count, earliest_epoch): (i64, Option<i64>) = conn.query_row(
            "SELECT COUNT(*), MIN(unixepoch(timestamp)) FROM request_logs",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        if any_count == 0 {
            return Ok(DashboardRawStats::default());
        }

        let (
            total_reqs,
            ok_2xx,
            unrouted_reqs,
            err_4xx,
            err_5xx,
            fallback_count,
            ok_latency_sum_ms,
            ok_latency_samples,
            rate_limited_429,
            auth_401_403,
            not_found_404,
            bad_request_4xx,
            upstream_timeout_502_504,
            upstream_500,
        ): (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = conn.query_row(
            "SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN (status_code < 200 OR status_code >= 300) AND (routed_provider_id = 'none' OR trim(routed_provider_id) = '') THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code >= 400 AND status_code < 500 AND routed_provider_id != 'none' AND trim(routed_provider_id) != '' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN (status_code >= 500 OR status_code < 200) AND routed_provider_id != 'none' AND trim(routed_provider_id) != '' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN fallback_used != 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code >= 200 AND status_code < 300 AND latency_ms > 0 THEN latency_ms ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code >= 200 AND status_code < 300 AND latency_ms > 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code = 429 AND routed_provider_id != 'none' AND trim(routed_provider_id) != '' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code IN (401, 403) AND routed_provider_id != 'none' AND trim(routed_provider_id) != '' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code = 404 AND routed_provider_id != 'none' AND trim(routed_provider_id) != '' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code >= 400 AND status_code < 500 AND status_code NOT IN (401, 403, 404, 429) AND routed_provider_id != 'none' AND trim(routed_provider_id) != '' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code IN (502, 503, 504) AND routed_provider_id != 'none' AND trim(routed_provider_id) != '' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code = 500 AND routed_provider_id != 'none' AND trim(routed_provider_id) != '' THEN 1 ELSE 0 END), 0)
             FROM request_logs
             WHERE (?1 IS NULL OR unixepoch(timestamp) >= ?1)",
            params![since_epoch],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    r.get(13)?,
                ))
            },
        )?;

        let total_err = (total_reqs - ok_2xx).max(0) as usize;
        let known_err = (unrouted_reqs
            + rate_limited_429
            + auth_401_403
            + not_found_404
            + bad_request_4xx
            + upstream_timeout_502_504
            + upstream_500)
            .max(0) as usize;
        let other_err = total_err.saturating_sub(known_err);

        let mut slice_stmt = conn.prepare(
            "SELECT
                CASE WHEN trim(routed_provider_id) = '' THEN 'none' ELSE routed_provider_id END AS pid,
                MAX(NULLIF(trim(routed_provider_name), '')) AS pname,
                CASE
                    WHEN trim(routed_model) != '' THEN trim(routed_model)
                    WHEN trim(incoming_model) != '' THEN trim(incoming_model)
                    ELSE 'unknown'
                END AS eff_model,
                COUNT(*),
                COALESCE(SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(MAX(prompt_tokens, 0)), 0),
                COALESCE(SUM(MAX(completion_tokens, 0)), 0)
             FROM request_logs
             WHERE (?1 IS NULL OR unixepoch(timestamp) >= ?1)
             GROUP BY pid, eff_model",
        )?;
        let slice_rows = slice_stmt.query_map(params![since_epoch], |r| {
            let calls: i64 = r.get(3)?;
            let ok_c: i64 = r.get(4)?;
            let err_c: i64 = r.get(5)?;
            let p_tok: i64 = r.get(6)?;
            let c_tok: i64 = r.get(7)?;
            Ok(RouteModelSlice {
                provider_id: r.get(0)?,
                provider_name: r.get(1)?,
                model: r.get(2)?,
                calls: calls.max(0) as usize,
                ok_calls: ok_c.max(0) as usize,
                err_calls: err_c.max(0) as usize,
                prompt_tokens: p_tok.max(0) as u64,
                completion_tokens: c_tok.max(0) as u64,
            })
        })?;
        let mut route_model_slices = Vec::new();
        for s in slice_rows {
            route_model_slices.push(s?);
        }

        let mut client_stmt = conn.prepare(
            "SELECT
                CASE WHEN trim(client_type) = '' THEN 'unknown' ELSE trim(client_type) END AS ctype,
                COUNT(*)
             FROM request_logs
             WHERE (?1 IS NULL OR unixepoch(timestamp) >= ?1)
             GROUP BY ctype
             ORDER BY COUNT(*) DESC",
        )?;
        let client_rows = client_stmt.query_map(params![since_epoch], |r| {
            let c: String = r.get(0)?;
            let cnt: i64 = r.get(1)?;
            Ok((c, cnt.max(0) as usize))
        })?;
        let mut client_counts = Vec::new();
        for c in client_rows {
            client_counts.push(c?);
        }

        let plan = fixed_buckets.unwrap_or_else(|| {
            let tomorrow_start = today_start_epoch + 86_400;
            let earliest = earliest_epoch
                .unwrap_or(today_start_epoch)
                .min(today_start_epoch);
            let raw_days = ((tomorrow_start - earliest + 86_399) / 86_400).max(1);
            if raw_days <= 14 {
                let days = raw_days.max(7) as usize;
                StatsBucketPlan {
                    start_epoch: tomorrow_start - (days as i64) * 86_400,
                    step_secs: 86_400,
                    count: days,
                }
            } else {
                let count = 14usize;
                let days_per_bucket = ((raw_days + count as i64 - 1) / count as i64).max(1);
                let total_days = days_per_bucket * count as i64;
                StatsBucketPlan {
                    start_epoch: tomorrow_start - total_days * 86_400,
                    step_secs: days_per_bucket * 86_400,
                    count,
                }
            }
        });

        let step = plan.step_secs.max(1);
        let bucket_count = plan.count.max(1);
        let mut buckets: Vec<TrendBucket> = (0..bucket_count)
            .map(|i| {
                let s = plan.start_epoch + (i as i64) * step;
                TrendBucket {
                    start_epoch: s,
                    end_epoch: s + step,
                    ok_calls: 0,
                    err_calls: 0,
                }
            })
            .collect();

        let range_end = plan.start_epoch + (bucket_count as i64) * step;
        let mut bucket_stmt = conn.prepare(
            "SELECT
                CAST((unixepoch(timestamp) - ?1) / ?2 AS INTEGER) AS slot,
                COALESCE(SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END), 0)
             FROM request_logs
             WHERE (?3 IS NULL OR unixepoch(timestamp) >= ?3)
               AND unixepoch(timestamp) >= ?1
               AND unixepoch(timestamp) < ?4
             GROUP BY slot",
        )?;
        let b_rows = bucket_stmt.query_map(
            params![plan.start_epoch, step, since_epoch, range_end],
            |r| {
                let slot: i64 = r.get(0)?;
                let ok_c: i64 = r.get(1)?;
                let err_c: i64 = r.get(2)?;
                Ok((slot, ok_c.max(0) as usize, err_c.max(0) as usize))
            },
        )?;
        for b in b_rows {
            let (slot, ok_c, err_c) = b?;
            if slot >= 0 && (slot as usize) < buckets.len() {
                buckets[slot as usize].ok_calls = ok_c;
                buckets[slot as usize].err_calls = err_c;
            }
        }

        Ok(DashboardRawStats {
            has_any_logs: true,
            total_reqs: total_reqs.max(0) as usize,
            ok_2xx: ok_2xx.max(0) as usize,
            err_4xx: err_4xx.max(0) as usize,
            err_5xx: err_5xx.max(0) as usize,
            unrouted_reqs: unrouted_reqs.max(0) as usize,
            fallback_count: fallback_count.max(0) as usize,
            ok_latency_sum_ms: ok_latency_sum_ms.max(0),
            ok_latency_samples: ok_latency_samples.max(0) as usize,
            earliest_epoch,
            route_model_slices,
            client_counts,
            errors: ErrorBreakdown {
                unrouted: unrouted_reqs.max(0) as usize,
                rate_limited_429: rate_limited_429.max(0) as usize,
                auth_401_403: auth_401_403.max(0) as usize,
                not_found_404: not_found_404.max(0) as usize,
                bad_request_4xx: bad_request_4xx.max(0) as usize,
                upstream_timeout_502_504: upstream_timeout_502_504.max(0) as usize,
                upstream_500: upstream_500.max(0) as usize,
                other_err,
            },
            buckets,
        })
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
    ) -> Result<Option<RequestLog>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "UPDATE request_logs
             SET prompt_tokens = ?1, completion_tokens = ?2, latency_ms = MAX(latency_ms, ?3)
             WHERE id = ?4",
            params![prompt_tokens, completion_tokens, latency_ms, id],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, method, path, client_type, incoming_model, routed_provider_id, routed_provider_name, routed_model, status_code, latency_ms, prompt_tokens, completion_tokens, error_message, is_stream, fallback_used, attempts
             FROM request_logs WHERE id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![id], Self::map_request_log_row)?;
        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
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

    pub fn save_interceptor_model(&self, client_id: &str, model: Option<&str>) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO interceptor_state (client_id, is_intercepted, model, updated_at)
             VALUES (?1, 0, ?2, ?3)
             ON CONFLICT(client_id) DO UPDATE SET
                model = excluded.model,
                updated_at = excluded.updated_at",
            params![client_id, model, now],
        )?;
        Ok(())
    }

    pub fn get_interceptor_model(&self, client_id: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT model FROM interceptor_state WHERE client_id = ?1")?;
        let mut rows = stmt.query(params![client_id])?;
        if let Some(row) = rows.next()? {
            Ok(row.get(0)?)
        } else {
            Ok(None)
        }
    }

    fn map_subscription_account_row(row: &rusqlite::Row<'_>) -> Result<SubscriptionAccount, rusqlite::Error> {
        let enabled_int: i32 = row.get(8)?;
        let plan_type: Option<String> = row.get(9)?;
        let supported_models_json: Option<String> = row.get(10)?;
        let supported_models = supported_models_json
            .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok());
        Ok(SubscriptionAccount {
            id: row.get(0)?,
            provider_id: row.get(1)?,
            account_type: row.get(2)?,
            email: row.get(3)?,
            access_token: row.get(4)?,
            refresh_token: row.get(5)?,
            expires_at: row.get(6)?,
            cooldown_until: row.get(7)?,
            enabled: enabled_int == 1,
            plan_type,
            supported_models,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    }

    pub fn get_subscription_accounts(&self, provider_id: &str) -> Result<Vec<SubscriptionAccount>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, provider_id, account_type, email, access_token, refresh_token, expires_at, cooldown_until, enabled, plan_type, supported_models, created_at, updated_at
             FROM subscription_accounts WHERE provider_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![provider_id], Self::map_subscription_account_row)?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_subscription_account_by_id(&self, account_id: &str) -> Result<Option<SubscriptionAccount>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, provider_id, account_type, email, access_token, refresh_token, expires_at, cooldown_until, enabled, plan_type, supported_models, created_at, updated_at
             FROM subscription_accounts WHERE id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![account_id], Self::map_subscription_account_row)?;
        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn save_subscription_account(&self, account: &SubscriptionAccount) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let supported_models_json = account
            .supported_models
            .as_ref()
            .and_then(|list| serde_json::to_string(list).ok());
        conn.execute(
            "INSERT INTO subscription_accounts (id, provider_id, account_type, email, access_token, refresh_token, expires_at, cooldown_until, enabled, plan_type, supported_models, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(id) DO UPDATE SET
                email = excluded.email,
                access_token = excluded.access_token,
                refresh_token = excluded.refresh_token,
                expires_at = excluded.expires_at,
                cooldown_until = excluded.cooldown_until,
                enabled = excluded.enabled,
                plan_type = COALESCE(excluded.plan_type, subscription_accounts.plan_type),
                supported_models = COALESCE(excluded.supported_models, subscription_accounts.supported_models),
                updated_at = excluded.updated_at;",
            params![
                account.id,
                account.provider_id,
                account.account_type,
                account.email,
                account.access_token,
                account.refresh_token,
                account.expires_at,
                account.cooldown_until,
                if account.enabled { 1 } else { 0 },
                account.plan_type,
                supported_models_json,
                account.created_at,
                now,
            ],
        )?;
        Ok(())
    }

    /// 持久化单个订阅账号实测支持的模型列表；传入空切片时不覆盖已有记录
    pub fn update_account_supported_models(
        &self,
        account_id: &str,
        models: &[String],
    ) -> Result<(), rusqlite::Error> {
        let mut seen = std::collections::HashSet::new();
        let cleaned: Vec<String> = models
            .iter()
            .map(|m| m.trim())
            .filter(|m| !m.is_empty() && seen.insert((*m).to_string()))
            .map(ToString::to_string)
            .collect();
        if cleaned.is_empty() {
            return Ok(());
        }
        let json = serde_json::to_string(&cleaned).unwrap_or_else(|_| "[]".to_string());
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE subscription_accounts SET supported_models = ?1, updated_at = ?2 WHERE id = ?3",
            params![json, now, account_id],
        )?;
        Ok(())
    }

    /// 更新订阅账号的套餐等级标识（如 Pro / Plus / Team）
    pub fn update_account_plan_type(
        &self,
        account_id: &str,
        plan_type: &str,
    ) -> Result<(), rusqlite::Error> {
        let trimmed = plan_type.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE subscription_accounts SET plan_type = ?1, updated_at = ?2 WHERE id = ?3",
            params![trimmed, now, account_id],
        )?;
        Ok(())
    }

    /// 运行时从指定账号的 supported_models 中移除上游判定无权调用的模型，避免后续请求误派
    pub fn remove_model_from_account(
        &self,
        account_id: &str,
        model: &str,
    ) -> Result<bool, rusqlite::Error> {
        let target = model.trim();
        if target.is_empty() {
            return Ok(false);
        }
        let conn = self.conn.lock().unwrap();
        let current_json: Option<String> = conn
            .query_row(
                "SELECT supported_models FROM subscription_accounts WHERE id = ?1 LIMIT 1",
                params![account_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();

        let Some(raw) = current_json else {
            return Ok(false);
        };
        let Ok(mut list) = serde_json::from_str::<Vec<String>>(&raw) else {
            return Ok(false);
        };

        let target_candidates = crate::gateway::router::normalize_model_candidates(target);
        let matches_target = |candidate: &str| -> bool {
            if candidate.eq_ignore_ascii_case(target) {
                return true;
            }
            if target_candidates
                .iter()
                .any(|c| c.eq_ignore_ascii_case(candidate))
            {
                return true;
            }
            crate::gateway::router::normalize_model_candidates(candidate)
                .iter()
                .any(|c| c.eq_ignore_ascii_case(target))
        };

        let before_len = list.len();
        list.retain(|m| !matches_target(m));
        if list.len() == before_len {
            return Ok(false);
        }

        let updated_json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE subscription_accounts SET supported_models = ?1, updated_at = ?2 WHERE id = ?3",
            params![updated_json, now, account_id],
        )?;
        Ok(true)
    }

    pub fn update_account_tokens(&self, account_id: &str, access_token: &str, refresh_token: Option<&str>, expires_at: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        if let Some(rf) = refresh_token {
            conn.execute(
                "UPDATE subscription_accounts SET access_token = ?1, refresh_token = ?2, expires_at = ?3, updated_at = ?4 WHERE id = ?5",
                params![access_token, rf, expires_at, now, account_id],
            )?;
        } else {
            conn.execute(
                "UPDATE subscription_accounts SET access_token = ?1, expires_at = ?2, updated_at = ?3 WHERE id = ?4",
                params![access_token, expires_at, now, account_id],
            )?;
        }
        Ok(())
    }

    pub fn set_account_cooldown(&self, account_id: &str, cooldown_until: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE subscription_accounts SET cooldown_until = ?1 WHERE id = ?2",
            params![cooldown_until, account_id],
        )?;
        Ok(())
    }

    pub fn delete_subscription_account(&self, account_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM subscription_accounts WHERE id = ?1", params![account_id])?;
        Ok(())
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
            auth_type: None,
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

        let removed = db.remove_untouched_seeded_providers().unwrap();
        assert_eq!(removed, vec!["openai".to_string()]);

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
    fn deleting_a_provider_removes_it() {
        let path = temp_db_path("delete");
        let db = Database::new(path.clone()).unwrap();
        db.save_provider(&provider_fixture("p1", "Primary", None)).unwrap();
        db.save_provider(&provider_fixture("p2", "Secondary", None)).unwrap();
        db.save_subscription_account(&SubscriptionAccount {
            id: "sub_1".to_string(),
            provider_id: "p1".to_string(),
            account_type: "antigravity".to_string(),
            email: Some("test@example.com".to_string()),
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: 9999999999,
            cooldown_until: 0,
            enabled: true,
            plan_type: None,
            supported_models: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }).unwrap();

        assert_eq!(db.get_subscription_accounts("p1").unwrap().len(), 1);

        db.delete_provider("p1").unwrap();

        assert_eq!(db.get_providers().unwrap().len(), 1);
        assert!(db.get_subscription_accounts("p1").unwrap().is_empty());
        // Deleting a provider that is already gone is reported instead of ignored.
        assert!(db.delete_provider("p1").is_err());
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn saving_providers_persists_cleanly() {
        let path = temp_db_path("saving_cleanly");
        let db = Database::new(path.clone()).unwrap();
        let first = provider_fixture("first", "First", None);
        db.save_provider(&first).unwrap();

        let second = provider_fixture("second", "Second", None);
        db.save_provider(&second).unwrap();

        let providers = db.get_providers().unwrap();
        assert_eq!(providers.len(), 2);

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

    #[test]
    fn cleans_up_legacy_fabricated_tokens_without_touching_real_usage() {
        let path = temp_db_path("legacy_tokens");
        let db = Database::new(path.clone()).unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Simulate a row fabricated by the legacy latency formula (latency_ms = 2000 -> c = 70, p = 1500)
        let mut fake_log = RequestLog {
            id: "fake-1".into(),
            timestamp: now.clone(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            client_type: "claude_code".into(),
            incoming_model: "claude-sonnet-4".into(),
            routed_provider_id: "p1".into(),
            routed_provider_name: Some("P1".into()),
            routed_model: "claude-sonnet-4".into(),
            status_code: 200,
            latency_ms: 2000,
            prompt_tokens: 1500,
            completion_tokens: 70,
            error_message: None,
            is_stream: true,
            fallback_used: false,
            attempts: vec![],
        };
        db.insert_log(&fake_log).unwrap();

        // Real usage row must remain untouched
        fake_log.id = "real-1".into();
        fake_log.prompt_tokens = 1842;
        fake_log.completion_tokens = 315;
        db.insert_log(&fake_log).unwrap();

        db.init_tables().unwrap();

        let logs = db.get_logs(10, 0).unwrap();
        let cleaned = logs.iter().find(|l| l.id == "fake-1").unwrap();
        let kept = logs.iter().find(|l| l.id == "real-1").unwrap();
        assert_eq!((cleaned.prompt_tokens, cleaned.completion_tokens), (0, 0));
        assert_eq!((kept.prompt_tokens, kept.completion_tokens), (1842, 315));

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn dashboard_stats_separate_unrouted_errors_and_ok_latency() {
        let path = temp_db_path("dash_stats");
        let db = Database::new(path.clone()).unwrap();
        let now_dt = chrono::Utc::now();
        let now_str = now_dt.to_rfc3339();
        let now_sec = now_dt.timestamp();
        let today_start = now_sec - (now_sec.rem_euclid(86_400));

        // 1) 200 stream request updated after completion
        db.insert_log(&RequestLog {
            id: "ok-1".into(),
            timestamp: now_str.clone(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            client_type: "claude_code".into(),
            incoming_model: "gemini-2.5-pro".into(),
            routed_provider_id: "ag".into(),
            routed_provider_name: Some("Antigravity".into()),
            routed_model: "gemini-2.5-pro".into(),
            status_code: 200,
            latency_ms: 120,
            prompt_tokens: 0,
            completion_tokens: 0,
            error_message: None,
            is_stream: true,
            fallback_used: false,
            attempts: vec![],
        })
        .unwrap();
        let updated = db
            .update_request_log_tokens("ok-1", 1000, 250, 1400)
            .unwrap()
            .expect("updated log returned");
        assert_eq!(updated.prompt_tokens, 1000);
        assert_eq!(updated.completion_tokens, 250);
        assert_eq!(updated.latency_ms, 1400);

        // 2) Unrouted 502 error (must be classified as unrouted, NOT upstream_timeout_502_504)
        db.insert_log(&RequestLog {
            id: "unrouted-1".into(),
            timestamp: now_str.clone(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            client_type: "claude_code".into(),
            incoming_model: "unknown-model".into(),
            routed_provider_id: "none".into(),
            routed_provider_name: None,
            routed_model: "".into(),
            status_code: 502,
            latency_ms: 2,
            prompt_tokens: 0,
            completion_tokens: 0,
            error_message: Some("No provider supports unknown-model".into()),
            is_stream: true,
            fallback_used: false,
            attempts: vec![],
        })
        .unwrap();

        // 3) Real upstream 504 timeout (60000ms must NOT pollute ok_latency_sum_ms)
        db.insert_log(&RequestLog {
            id: "timeout-1".into(),
            timestamp: now_str,
            method: "POST".into(),
            path: "/v1/messages".into(),
            client_type: "codex".into(),
            incoming_model: "gpt-5".into(),
            routed_provider_id: "ag".into(),
            routed_provider_name: Some("Antigravity".into()),
            routed_model: "gpt-5".into(),
            status_code: 504,
            latency_ms: 60_000,
            prompt_tokens: 0,
            completion_tokens: 0,
            error_message: Some("Gateway timeout".into()),
            is_stream: true,
            fallback_used: true,
            attempts: vec![],
        })
        .unwrap();

        let stats = db.get_dashboard_stats(None, None, today_start).unwrap();
        assert_eq!(stats.total_reqs, 3);
        assert_eq!(stats.ok_2xx, 1);
        assert_eq!(stats.unrouted_reqs, 1);
        assert_eq!(stats.err_5xx, 1);
        assert_eq!(stats.errors.unrouted, 1);
        assert_eq!(stats.errors.upstream_timeout_502_504, 1);
        assert_eq!(stats.ok_latency_samples, 1);
        assert_eq!(stats.ok_latency_sum_ms, 1400);

        let err_logs = db.get_logs_in_window(None, true, 50).unwrap();
        assert_eq!(err_logs.len(), 2);

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn subscription_account_plan_and_models_migrate_roundtrip_and_prune() {
        let path = temp_db_path("sub_models");
        let db = Database::new(path.clone()).unwrap();

        // 模拟老版本表结构缺少 plan_type / supported_models 列
        {
            let conn = db.conn.lock().unwrap();
            conn.execute("ALTER TABLE subscription_accounts DROP COLUMN plan_type", [])
                .unwrap();
            conn.execute(
                "ALTER TABLE subscription_accounts DROP COLUMN supported_models",
                [],
            )
            .unwrap();
        }
        db.init_tables().unwrap();

        let acc = SubscriptionAccount {
            id: "sub_tier".to_string(),
            provider_id: "chatgpt_subscription".to_string(),
            account_type: "chatgpt".to_string(),
            email: Some("pro@example.com".to_string()),
            access_token: "tok".to_string(),
            refresh_token: Some("rf".to_string()),
            expires_at: 9_999_999_999,
            cooldown_until: 0,
            enabled: true,
            plan_type: Some("Pro".to_string()),
            supported_models: Some(vec!["gpt-6-astra".to_string(), "gpt-6-luna".to_string()]),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };
        db.save_subscription_account(&acc).unwrap();

        let loaded = db.get_subscription_account_by_id("sub_tier").unwrap().unwrap();
        assert_eq!(loaded.plan_type.as_deref(), Some("Pro"));
        assert_eq!(
            loaded.supported_models,
            Some(vec!["gpt-6-astra".to_string(), "gpt-6-luna".to_string()])
        );

        // 空切片不覆盖已有列表
        db.update_account_supported_models("sub_tier", &[]).unwrap();
        let after_empty = db.get_subscription_account_by_id("sub_tier").unwrap().unwrap();
        assert_eq!(after_empty.supported_models.as_ref().map(Vec::len), Some(2));

        // 精准移除单个模型（忽略大小写）
        assert!(db.remove_model_from_account("sub_tier", "GPT-6-ASTRA").unwrap());
        assert!(!db.remove_model_from_account("sub_tier", "non-existent").unwrap());
        let after_remove = db.get_subscription_account_by_id("sub_tier").unwrap().unwrap();
        assert_eq!(
            after_remove.supported_models,
            Some(vec!["gpt-6-luna".to_string()])
        );

        // 移除最后一个模型时保留 Some([])，而非回退为 None
        assert!(db.remove_model_from_account("sub_tier", "gpt-6-luna").unwrap());
        let after_last = db.get_subscription_account_by_id("sub_tier").unwrap().unwrap();
        assert_eq!(after_last.supported_models, Some(vec![]));

        drop(db);
        let _ = std::fs::remove_file(path);
    }
}