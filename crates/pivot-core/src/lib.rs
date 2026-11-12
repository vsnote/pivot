pub mod db;
pub mod gateway;
pub mod interceptor;
pub mod model_catalog;
pub mod oauth;
pub mod provider_models;
pub mod service;
pub mod state;

use db::Database;
use gateway::router::RoutingEngine;
use gateway::server::GatewayStatsInternal;
use state::AppState;
use std::path::PathBuf;
use std::sync::Arc;

pub const DEFAULT_PORT: u16 = 16888;

pub fn get_db_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let new_dir = home.join(".pivot");
    let new_db = new_dir.join("pivot.db");

    if !new_db.exists() {
        let legacy_db = home.join(".omniswitch").join("omniswitch.db");
        if legacy_db.exists() {
            let _ = std::fs::create_dir_all(&new_dir);
            let _ = std::fs::copy(&legacy_db, &new_db);
            let legacy_wal = home.join(".omniswitch").join("omniswitch.db-wal");
            let legacy_shm = home.join(".omniswitch").join("omniswitch.db-shm");
            if legacy_wal.exists() {
                let _ = std::fs::copy(legacy_wal, new_dir.join("pivot.db-wal"));
            }
            if legacy_shm.exists() {
                let _ = std::fs::copy(legacy_shm, new_dir.join("pivot.db-shm"));
            }
        }
    }

    new_db
}

pub fn init_core(port: Option<u16>) -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let target_port = port.unwrap_or(DEFAULT_PORT);
    let db_path = get_db_path();
    let db = Database::new(db_path.clone())?;

    let catalog_path = db_path
        .parent()
        .map(|dir| dir.join("model-catalog.json"))
        .unwrap_or_else(|| PathBuf::from("model-catalog.json"));
    model_catalog::init(catalog_path);

    let router = Arc::new(RoutingEngine::new());
    let default_ua = format!("Pivot/{} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::OS);
    let http_client = reqwest::Client::builder()
        .user_agent(default_ua)
        .connect_timeout(std::time::Duration::from_secs(15))
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .tcp_keepalive(std::time::Duration::from_secs(60))
        .build()?;

    let stats = Arc::new(GatewayStatsInternal::default());

    let app_state = AppState {
        db: db.clone(),
        router: router.clone(),
        http_client: http_client.clone(),
        stats: stats.clone(),
        port: target_port,
    };

    Ok(app_state)
}

pub fn start_gateway(
    state: AppState,
    log_sender: Option<tokio::sync::broadcast::Sender<db::RequestLog>>,
) {
    let interceptor_mgr = interceptor::InterceptorManager::new(state.db.clone(), state.port);
    let _ = interceptor_mgr.sync_codex_catalog_if_intercepted();
    let _ = interceptor_mgr.sync_claude_settings_if_intercepted();
    let _ = interceptor_mgr.sync_pi_models_if_intercepted();
    let _ = interceptor_mgr.sync_opencode_models_if_intercepted();

    let gw_state = gateway::server::GatewayState {
        db: state.db.clone(),
        router: state.router.clone(),
        http_client: state.http_client.clone(),
        stats: state.stats.clone(),
        log_sender,
    };

    gateway::start_gateway(gw_state, state.port);

    if !model_catalog::is_fresh(86400) {
        let client = state.http_client.clone();
        let catalog_path = get_db_path()
            .parent()
            .map(|dir| dir.join("model-catalog.json"))
            .unwrap_or_else(|| PathBuf::from("model-catalog.json"));
        tokio::spawn(async move {
            let _ = model_catalog::refresh(&client, &catalog_path).await;
        });
    }
}
