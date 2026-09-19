pub mod commands;
pub mod provider_models;
pub mod db;
pub mod gateway;
pub mod interceptor;
pub mod macos_tab;
pub mod state;
pub mod tray;

use commands::*;
use db::Database;
use gateway::router::RoutingEngine;
use gateway::server::{GatewayState, GatewayStatsInternal};
use state::AppState;
use std::path::PathBuf;
use std::sync::Arc;

const DEFAULT_PORT: u16 = 16888;

fn get_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".omniswitch")
        .join("omniswitch.db")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = get_db_path();
    let db = Database::new(db_path).expect("Failed to initialize SQLite database");

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

            // macOS Native Tab Bar disabled in favor of unified responsive desktop header
            // if let Some(main_win) = handle.get_webview_window("main") {
            //     macos_tab::setup_native_tabs(&main_win);
            // }

            // Start Axum HTTP Gateway Server
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
            get_providers,
            save_provider,
            set_provider_enabled,
            delete_provider,
            test_provider_latency,
            get_routes,
            save_routes,
            get_logs,
            clear_logs,
            get_interceptor_status,
            toggle_interceptor,
            get_presets,
            preview_route,
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
