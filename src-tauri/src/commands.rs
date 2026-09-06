use crate::db::presets::{self, Preset};
use crate::db::{Provider, RequestLog, RouteRule};
use crate::interceptor::{InterceptorInfo, InterceptorManager};
use crate::state::AppState;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub async fn get_gateway_status(state: State<'_, AppState>) -> Result<Value, String> {
    let uptime = state
        .stats
        .start_time
        .lock()
        .unwrap()
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);

    Ok(json!({
        "isRunning": true,
        "host": "127.0.0.1",
        "port": state.port,
        "activeRequests": state.stats.active_requests.load(Ordering::Relaxed),
        "totalRequests": state.stats.total_requests.load(Ordering::Relaxed),
        "totalPromptTokens": state.stats.total_prompt_tokens.load(Ordering::Relaxed),
        "totalCompletionTokens": state.stats.total_completion_tokens.load(Ordering::Relaxed),
        "errorCount": state.stats.error_count.load(Ordering::Relaxed),
        "uptimeSecs": uptime,
    }))
}

#[tauri::command]
pub async fn get_providers(state: State<'_, AppState>) -> Result<Vec<Provider>, String> {
    state.db.get_providers().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_provider(state: State<'_, AppState>, provider: Provider) -> Result<(), String> {
    state.db.save_provider(&provider).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_provider(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_provider(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_provider_latency(state: State<'_, AppState>, id: String) -> Result<i64, String> {
    let providers = state.db.get_providers().map_err(|e| e.to_string())?;
    let provider = providers
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Provider not found: {}", id))?;

    let start = Instant::now();
    let url = provider.base_url.trim_end_matches('/').to_string();

    let mut req = state.http_client.get(&url).timeout(std::time::Duration::from_secs(5));
    if let Some(key) = provider.api_keys.first() {
        if provider.protocol == "anthropic" {
            req = req.header("x-api-key", key);
        } else {
            req = req.header("authorization", format!("Bearer {}", key));
        }
    }

    let res = req.send().await;
    let latency_ms = start.elapsed().as_millis() as i64;

    // Even if it returns 401 or 404, it means the server is reached and network latency is measured
    let measured_latency = match res {
        Ok(_) => latency_ms,
        Err(e) => {
            if e.is_timeout() {
                return Err("Connection timed out (5s)".to_string());
            }
            if e.is_connect() {
                return Err("Connection failed: host unreachable".to_string());
            }
            latency_ms
        }
    };

    let _ = state.db.update_provider_latency(&id, measured_latency);
    Ok(measured_latency)
}

#[tauri::command]
pub async fn get_routes(state: State<'_, AppState>) -> Result<Vec<RouteRule>, String> {
    state.db.get_routes().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_routes(state: State<'_, AppState>, routes: Vec<RouteRule>) -> Result<(), String> {
    state.db.save_routes(&routes).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_logs(
    state: State<'_, AppState>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<RequestLog>, String> {
    let lim = limit.unwrap_or(100);
    let off = offset.unwrap_or(0);
    state.db.get_logs(lim, off).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    state.db.clear_logs().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_interceptor_status(state: State<'_, AppState>) -> Result<Vec<InterceptorInfo>, String> {
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    Ok(mgr.get_all_status())
}

#[tauri::command]
pub async fn toggle_interceptor(
    state: State<'_, AppState>,
    id: String,
    enable: bool,
) -> Result<InterceptorInfo, String> {
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    mgr.toggle_tool(&id, enable)
}

#[tauri::command]
pub fn get_presets() -> Vec<Preset> {
    presets::get_default_presets()
}

#[tauri::command]
pub async fn import_preset(state: State<'_, AppState>, preset_id: String) -> Result<Provider, String> {
    let presets = presets::get_default_presets();
    let preset = presets
        .into_iter()
        .find(|p| p.id == preset_id)
        .ok_or_else(|| format!("Preset not found: {}", preset_id))?;

    let now = chrono::Utc::now().to_rfc3339();
    let provider = Provider {
        id: preset.id,
        name: preset.name,
        protocol: preset.protocol,
        base_url: preset.base_url,
        api_keys: Vec::new(),
        models: preset.default_models,
        enabled: true,
        priority: 10,
        weight: 1,
        timeout_secs: 60,
        latency_ms: None,
        created_at: Some(now.clone()),
        updated_at: Some(now),
    };

    state.db.save_provider(&provider).map_err(|e| e.to_string())?;
    Ok(provider)
}
