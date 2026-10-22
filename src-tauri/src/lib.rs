pub mod commands;
pub mod provider_models;
pub mod db;
pub mod gateway;
pub mod interceptor;
pub mod macos_tab;
pub mod model_catalog;
pub mod state;
pub mod tray;

use commands::*;
use db::Database;
use gateway::router::RoutingEngine;
use gateway::server::{GatewayState, GatewayStatsInternal};
use state::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

const DEFAULT_PORT: u16 = 16888;

fn get_db_path() -> PathBuf {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = get_db_path();
    let db = Database::new(db_path.clone()).expect("Failed to initialize SQLite database");

    // 模型能力目录：先加载本地缓存，网络刷新在 setup 中后台进行
    let catalog_path = db_path
        .parent()
        .map(|dir| dir.join("model-catalog.json"))
        .unwrap_or_else(|| PathBuf::from("model-catalog.json"));
    model_catalog::init(catalog_path.clone());

    let router = Arc::new(RoutingEngine::new());
    let http_client = reqwest::Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .tcp_keepalive(std::time::Duration::from_secs(60))
        .build()
        .expect("Failed to build reqwest HTTP client");

    let stats = Arc::new(GatewayStatsInternal::default());

    let app_state = AppState {
        db: db.clone(),
        router: router.clone(),
        http_client: http_client.clone(),
        stats: stats.clone(),
        port: DEFAULT_PORT,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .setup(move |app| {
            let handle = app.handle().clone();

            // Setup system tray
            if let Err(e) = tray::setup_tray(&handle, DEFAULT_PORT) {
                tracing::warn!("Failed to setup system tray: {}", e);
            }

            // macOS Native Toolbar: activates unified titlebar style so traffic lights align with macOS standard (Finder-like)
            if let Some(main_win) = handle.get_webview_window("main") {
                macos_tab::setup_unified_toolbar(&main_win);
            }

            // Start Axum HTTP Gateway Server
            let interceptor_mgr = interceptor::InterceptorManager::new(db.clone(), DEFAULT_PORT);
            let _ = interceptor_mgr.sync_codex_catalog_if_intercepted();
            let _ = interceptor_mgr.sync_claude_settings_if_intercepted();
            let _ = interceptor_mgr.sync_pi_models_if_intercepted();

            // 后台刷新模型目录：缓存仍在 TTL 内则跳过；刷新成功后重写依赖模型能力的客户端配置
            if !model_catalog::is_fresh(24 * 60 * 60) {
                let client = http_client.clone();
                let path = catalog_path.clone();
                let db_for_catalog = db.clone();
                tauri::async_runtime::spawn(async move {
                    match model_catalog::refresh(&client, &path).await {
                        Ok(count) => {
                            tracing::info!("Model catalog refreshed: {} entries", count);
                            let mgr =
                                interceptor::InterceptorManager::new(db_for_catalog, DEFAULT_PORT);
                            let _ = mgr.sync_codex_catalog_if_intercepted();
                            let _ = mgr.sync_pi_models_if_intercepted();
                        }
                        Err(e) => tracing::warn!("Model catalog refresh failed: {}", e),
                    }
                });
            }

            let gw_state = GatewayState {
                db,
                router,
                http_client,
                stats,
                app_handle: Some(handle),
            };

            gateway::start_gateway(gw_state, DEFAULT_PORT);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_gateway_status,
            get_model_rates,
            get_model_capabilities,
            get_providers,
            save_provider,
            reorder_providers,
            set_provider_enabled,
            delete_provider,
            test_provider_latency,
            test_endpoint_latency,
            get_routes,
            save_routes,
            get_logs,
            clear_logs,
            get_interceptor_status,
            toggle_interceptor,
            import_third_party_provider,
            get_presets,
            explain_route,
            fetch_provider_models,
            sync_native_tab,
            set_native_toolbar_enabled,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
