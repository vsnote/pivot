use crate::db::presets::{self, Preset};
use crate::gateway::router as gateway_router;
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
        "isRunning": state.stats.is_running.load(Ordering::Relaxed),
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
pub async fn save_provider(state: State<'_, AppState>, mut provider: Provider) -> Result<(), String> {
    crate::provider_models::validate_provider(&mut provider)?;
    state.db.save_provider(&provider).map_err(|e| e.to_string())?;
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    let _ = mgr.sync_codex_catalog_if_intercepted();
    let _ = mgr.sync_claude_settings_if_intercepted();
    Ok(())
}

/// Deletes a provider and returns the routing rules that survive it, because a
/// provider can be the target or a fallback of a rule the caller is showing.
#[tauri::command]
pub async fn delete_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<RouteRule>, String> {
    state
        .db
        .delete_provider(&id)
        .map_err(|_| format!("服务商 {} 不存在或已被删除", id))?;
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    let _ = mgr.sync_codex_catalog_if_intercepted();
    let _ = mgr.sync_claude_settings_if_intercepted();
    state.db.get_routes().map_err(|e| e.to_string())
}

async fn measure_http_latency(
    client: &reqwest::Client,
    protocol: &str,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<i64, String> {
    let url = base_url.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        return Err("请输入有效的接口基地址".to_string());
    }

    let start = Instant::now();
    let mut req = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5));

    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        if protocol == "anthropic" {
            req = req.header("x-api-key", key.trim());
        } else {
            req = req.header("authorization", format!("Bearer {}", key.trim()));
        }
    }

    let res = req.send().await;
    let latency_ms = start.elapsed().as_millis() as i64;

    match res {
        Ok(_) => Ok(latency_ms),
        Err(e) => {
            if e.is_timeout() {
                return Err("连接超时 (5s)".to_string());
            }
            if e.is_connect() {
                return Err("无法连接至服务器，请检查地址或网络".to_string());
            }
            // 404/401 等 HTTP 状态码表明服务物理可通
            Ok(latency_ms)
        }
    }
}

#[tauri::command]
pub async fn test_provider_latency(state: State<'_, AppState>, id: String) -> Result<i64, String> {
    let providers = state.db.get_providers().map_err(|e| e.to_string())?;
    let provider = providers
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Provider not found: {}", id))?;

    let measured_latency = measure_http_latency(
        &state.http_client,
        &provider.protocol,
        &provider.base_url,
        provider.api_keys.first().map(String::as_str),
    )
    .await?;

    let _ = state.db.update_provider_latency(&id, measured_latency);
    Ok(measured_latency)
}

#[tauri::command]
pub async fn test_endpoint_latency(
    state: State<'_, AppState>,
    protocol: String,
    base_url: String,
    api_key: Option<String>,
) -> Result<i64, String> {
    measure_http_latency(
        &state.http_client,
        &protocol,
        &base_url,
        api_key.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn get_routes(state: State<'_, AppState>) -> Result<Vec<RouteRule>, String> {
    state.db.get_routes().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_routes(state: State<'_, AppState>, routes: Vec<RouteRule>) -> Result<(), String> {
    state.db.save_routes(&routes).map_err(|e| e.to_string())?;
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    let _ = mgr.sync_claude_settings_if_intercepted();
    Ok(())
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
pub async fn get_interceptor_status(
    state: State<'_, AppState>,
) -> Result<Vec<InterceptorInfo>, String> {
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
pub async fn import_third_party_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<(Provider, InterceptorInfo), String> {
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    let third_party = mgr
        .get_third_party_config(&id)
        .ok_or_else(|| format!("未检测到 {} 的有效第三方配置", id))?;

    let existing_providers = state.db.get_providers().map_err(|e| e.to_string())?;
    let found = existing_providers
        .iter()
        .find(|p| p.base_url.trim_end_matches('/') == third_party.endpoint.trim_end_matches('/'));

    let models = match crate::provider_models::fetch_models(
        &third_party.protocol,
        &third_party.endpoint,
        third_party.auth_token.as_deref().unwrap_or(""),
    )
    .await
    {
        Ok(fetched) if !fetched.is_empty() => fetched,
        _ => third_party.models,
    };

    let provider = match found {
        Some(existing) => {
            let mut updated = existing.clone();
            updated.enabled = true;
            if updated.name.contains("CC-Switch") {
                updated.name = third_party.suggested_name;
            }
            if !models.is_empty() {
                updated.models = models;
            }
            state.db.save_provider(&updated).map_err(|e| e.to_string())?;
            updated
        }
        None => {
            let new_provider = Provider {
                id: format!("tp_{}_{}", id, &uuid::Uuid::new_v4().to_string()[..8]),
                name: third_party.suggested_name,
                protocol: third_party.protocol,
                base_url: third_party.endpoint,
                preset_id: None,
                api_keys: third_party
                    .auth_token
                    .filter(|s| !s.trim().is_empty())
                    .map(|k| vec![k])
                    .unwrap_or_else(|| vec!["default".to_string()]),
                models,
                default_model: None,
                is_default: existing_providers.is_empty(),
                enabled: true,
                priority: 10,
                weight: 1,
                timeout_secs: 60,
                latency_ms: None,
                created_at: None,
                updated_at: None,
            };
            state.db.save_provider(&new_provider).map_err(|e| e.to_string())?;
            new_provider
        }
    };

    let info = mgr.toggle_tool(&id, true)?;
    Ok((provider, info))
}

/// Explain what the gateway would do with a model name, using the very same
/// resolver the proxy runs, so the advanced page cannot drift from reality.
#[tauri::command]
pub async fn preview_route(
    state: State<'_, AppState>,
    model: String,
) -> Result<gateway_router::RoutePlan, String> {
    let providers = state.db.get_providers().map_err(|e| e.to_string())?;
    let routes = state.db.get_routes().map_err(|e| e.to_string())?;
    gateway_router::plan_route(&model, &providers, &routes)
}

#[tauri::command]
pub fn get_presets() -> Vec<Preset> {
    presets::get_default_presets()
}

#[tauri::command]
pub fn sync_native_tab(tab: String) {
    crate::macos_tab::set_selected_tab(&tab);
}

#[tauri::command]
pub fn set_native_toolbar_enabled(enabled: bool) {
    crate::macos_tab::set_toolbar_enabled(enabled);
}

#[tauri::command]
pub async fn fetch_provider_models(protocol: String, base_url: String, api_key: String) -> Result<Vec<String>, String> {
    crate::provider_models::fetch_models(&protocol, &base_url, &api_key).await
}

#[tauri::command]
pub async fn set_provider_enabled(state: State<'_, AppState>, id: String, enabled: bool) -> Result<(), String> {
    state.db.set_provider_enabled(&id, enabled).map_err(|e| e.to_string())?;
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    let _ = mgr.sync_codex_catalog_if_intercepted();
    let _ = mgr.sync_claude_settings_if_intercepted();
    Ok(())
}


