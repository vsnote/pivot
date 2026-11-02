use crate::db::presets::{self, Preset};
use crate::gateway::router as gateway_router;
use crate::db::{Provider, RequestLog, RouteRule, SubscriptionAccount};
use crate::interceptor::{InterceptorInfo, InterceptorManager};
use crate::oauth;
use crate::state::AppState;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::Instant;
use tauri::State;

/// 前端成本估算用的模型费率表（来自 models.dev 目录）
#[tauri::command]
pub fn get_model_rates() -> Value {
    crate::model_catalog::rate_table()
}

fn is_pure_non_text_model(n: &str) -> bool {
    n.contains("dall-e")
        || n.contains("imagen")
        || n.contains("flux")
        || n.contains("whisper")
        || n.contains("tts")
        || n.contains("embedding")
        || n.contains("embed")
}

fn infer_model_heuristics(name: &str, n: &str) -> (bool, bool, Option<u64>) {
    // 1. 深度思考 / 推理模型推断
    let (spec_levels, default_lvl) = crate::interceptor::resolve_model_reasoning_spec(name);
    let reasoning = (!spec_levels.is_empty() && default_lvl != "none")
        || n.contains("thinking")
        || n.contains("thought")
        || n.contains("reason")
        || n.contains("r1")
        || n.contains("o1")
        || n.contains("o3")
        || n.contains("o4")
        || n.contains("high")
        || n.contains("gemini-3")
        || n.contains("gemini-2.5");

    // 2. 视觉多模态推断 (支持图片输入)
    let vision = if is_pure_non_text_model(n) {
        false
    } else {
        n.contains("vision")
            || n.contains("image")
            || n.contains("vl")
            || n.contains("multimodal")
            || n.starts_with("gemini-")
            || n.starts_with("claude-3")
            || n.starts_with("claude-4")
            || n.starts_with("gpt-4o")
            || n.starts_with("gpt-4-turbo")
            || n.starts_with("gpt-5")
            || n.starts_with("gpt-6")
            || n.starts_with("o1")
            || n.starts_with("o3")
            || n.starts_with("o4")
            || n.contains("qwen-vl")
            || n.contains("qwen2-vl")
            || n.contains("qwen2.5-vl")
    };

    // 3. 上下文窗口保底推断
    let context_window = if n.starts_with("gemini-") {
        Some(1_048_576) // 1M
    } else if n.starts_with("claude-3") || n.starts_with("claude-4") {
        Some(200_000) // 200K
    } else if n.starts_with("gpt-4o") || n.starts_with("o1") || n.starts_with("o3") || n.starts_with("gpt-5") {
        Some(128_000) // 128K
    } else if n.contains("deepseek") {
        Some(64_000) // 64K
    } else {
        None
    };

    (reasoning, vision, context_window)
}

/// 批量查询模型能力，用于编辑器里展示多模态 / 推理等标记
#[tauri::command]
pub fn get_model_capabilities(model_names: Vec<String>) -> Value {
    let mut out = serde_json::Map::new();
    for name in model_names {
        let catalog_model = crate::model_catalog::lookup(&name, None);
        let lower = name.to_lowercase();
        let n = lower.split('/').last().unwrap_or(&lower);
        let (inferred_reasoning, inferred_vision, inferred_context) = infer_model_heuristics(&name, n);

        let reasoning = catalog_model
            .as_ref()
            .map(|m| m.reasoning)
            .unwrap_or(false)
            || inferred_reasoning;

        let mut modalities: Vec<String> = catalog_model
            .as_ref()
            .map(|m| m.client_input_modalities().into_iter().map(String::from).collect())
            .unwrap_or_default();

        if !modalities.contains(&"text".to_string()) && !is_pure_non_text_model(n) {
            modalities.push("text".to_string());
        }
        if inferred_vision && !modalities.contains(&"image".to_string()) {
            modalities.push("image".to_string());
        }

        let context_window = catalog_model
            .as_ref()
            .and_then(|m| m.context_window)
            .or(inferred_context);

        let max_output = catalog_model.as_ref().and_then(|m| m.max_output);

        out.insert(
            name,
            json!({
                "modalities": modalities,
                "contextWindow": context_window,
                "maxOutput": max_output,
                "reasoning": reasoning,
            }),
        );
    }
    Value::Object(out)
}

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
    let _ = mgr.sync_pi_models_if_intercepted();
    Ok(())
}

/// 重新排列服务商：列表顺序即路由优先级，返回重排后的完整列表
#[tauri::command]
pub async fn reorder_providers(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<Vec<Provider>, String> {
    state
        .db
        .reorder_providers(&ids)
        .map_err(|e| e.to_string())?;
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    let _ = mgr.sync_codex_catalog_if_intercepted();
    let _ = mgr.sync_pi_models_if_intercepted();
    state.db.get_providers().map_err(|e| e.to_string())
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
    let _ = mgr.sync_pi_models_if_intercepted();
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
    let _ = mgr.sync_pi_models_if_intercepted();
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
                auth_type: None,
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
/// resolver the proxy runs, so the routing page cannot drift from reality.
#[tauri::command]
pub async fn explain_route(
    state: State<'_, AppState>,
    model: String,
) -> Result<gateway_router::RouteExplanation, String> {
    let providers = state.db.get_providers().map_err(|e| e.to_string())?;
    let routes = state.db.get_routes().map_err(|e| e.to_string())?;
    Ok(gateway_router::explain_route(&model, &providers, &routes))
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
    let _ = mgr.sync_pi_models_if_intercepted();
    Ok(())
}

#[tauri::command]
pub async fn start_subscription_oauth(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    provider_id: String,
    account_type: String,
) -> Result<SubscriptionAccount, String> {
    let token_resp = if account_type == "chatgpt" {
        let pkce = oauth::generate_pkce();
        let state_str = uuid::Uuid::new_v4().to_string();
        let auth_url = oauth::build_chatgpt_auth_url(&pkce, &state_str);

        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(&auth_url, None::<&str>)
            .map_err(|e| format!("无法打开浏览器: {}", e))?;

        let code = oauth::listen_for_chatgpt_code(&state_str, 120).await?;
        oauth::exchange_chatgpt_code(&state.http_client, &code, &pkce.verifier).await?
    } else {
        let pkce = oauth::generate_pkce();
        let state_str = uuid::Uuid::new_v4().to_string();
        let auth_url = oauth::build_auth_url(&pkce, &state_str);

        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(&auth_url, None::<&str>)
            .map_err(|e| format!("无法打开浏览器: {}", e))?;

        let code = oauth::listen_for_code(&state_str, 120).await?;
        oauth::exchange_code_for_tokens(&state.http_client, &code, &pkce.verifier).await?
    };

    let now_sec = chrono::Utc::now().timestamp();
    let expires_at = now_sec + token_resp.expires_in;
    let now_str = chrono::Utc::now().to_rfc3339();

    let existing_accounts = state.db.get_subscription_accounts(&provider_id).unwrap_or_default();
    if let Some(target) = existing_accounts
        .into_iter()
        .find(|a| (a.email == token_resp.email && token_resp.email.is_some()) || (account_type == "chatgpt" && a.account_type == "chatgpt"))
    {
        state
            .db
            .update_account_tokens(
                &target.id,
                &token_resp.access_token,
                token_resp.refresh_token.as_deref(),
                expires_at,
            )
            .map_err(|e| e.to_string())?;

        let mut updated = target;
        updated.access_token = token_resp.access_token;
        if let Some(rf) = token_resp.refresh_token {
            updated.refresh_token = Some(rf);
        }
        updated.expires_at = expires_at;
        updated.updated_at = now_str;
        return Ok(updated);
    }

    let account = SubscriptionAccount {
        id: format!("sub_{}", &uuid::Uuid::new_v4().to_string()[..8]),
        provider_id,
        account_type,
        email: token_resp.email,
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at,
        cooldown_until: 0,
        enabled: true,
        created_at: now_str.clone(),
        updated_at: now_str,
    };

    state
        .db
        .save_subscription_account(&account)
        .map_err(|e| e.to_string())?;
    Ok(account)
}

#[tauri::command]
pub async fn import_codex_subscription_account(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<SubscriptionAccount, String> {
    let token_resp = oauth::import_local_codex_account(&state.http_client).await?;

    let now_sec = chrono::Utc::now().timestamp();
    let expires_at = now_sec + token_resp.expires_in;
    let now_str = chrono::Utc::now().to_rfc3339();

    let existing_accounts = state.db.get_subscription_accounts(&provider_id).unwrap_or_default();
    if let Some(target) = existing_accounts
        .into_iter()
        .find(|a| (a.email == token_resp.email && token_resp.email.is_some()) || a.account_type == "chatgpt")
    {
        state
            .db
            .update_account_tokens(
                &target.id,
                &token_resp.access_token,
                token_resp.refresh_token.as_deref(),
                expires_at,
            )
            .map_err(|e| e.to_string())?;

        let mut updated = target;
        updated.access_token = token_resp.access_token;
        if let Some(rf) = token_resp.refresh_token {
            updated.refresh_token = Some(rf);
        }
        updated.expires_at = expires_at;
        updated.updated_at = now_str;
        return Ok(updated);
    }

    let account = SubscriptionAccount {
        id: format!("sub_{}", &uuid::Uuid::new_v4().to_string()[..8]),
        provider_id,
        account_type: "chatgpt".to_string(),
        email: token_resp.email,
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at,
        cooldown_until: 0,
        enabled: true,
        created_at: now_str.clone(),
        updated_at: now_str,
    };

    state
        .db
        .save_subscription_account(&account)
        .map_err(|e| e.to_string())?;
    Ok(account)
}

#[tauri::command]
pub fn get_subscription_accounts(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<SubscriptionAccount>, String> {
    state
        .db
        .get_subscription_accounts(&provider_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_subscription_account(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<(), String> {
    state
        .db
        .delete_subscription_account(&account_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_subscription_account(
    state: State<'_, AppState>,
    provider_id: String,
    account_id: String,
    enabled: bool,
) -> Result<(), String> {
    let accounts = state
        .db
        .get_subscription_accounts(&provider_id)
        .map_err(|e| e.to_string())?;
    if let Some(mut account) = accounts.into_iter().find(|a| a.id == account_id) {
        account.enabled = enabled;
        state
            .db
            .save_subscription_account(&account)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_subscription_models(
    state: State<'_, AppState>,
    provider_id: String,
    base_url: String,
) -> Result<Vec<String>, String> {
    let account = oauth::acquire_subscription_account(&state.db, &state.http_client, &provider_id)
        .await
        .map_err(|(_, msg)| msg)?;

    if account.account_type == "chatgpt" {
        oauth::fetch_chatgpt_models(&state.http_client, &account.access_token).await
    } else {
        oauth::fetch_antigravity_models(&state.http_client, &account.access_token, &base_url).await
    }
}

#[tauri::command]
pub async fn get_subscription_account_quota(
    state: State<'_, AppState>,
    provider_id: String,
    account_id: String,
    base_url: Option<String>,
) -> Result<oauth::AccountQuota, String> {
    let accounts = state
        .db
        .get_subscription_accounts(&provider_id)
        .map_err(|e| e.to_string())?;

    let mut account = accounts
        .into_iter()
        .find(|a| a.id == account_id)
        .ok_or_else(|| "未找到指定订阅账号".to_string())?;

    let now_sec = chrono::Utc::now().timestamp();
    // 检查 Token 是否需要刷新
    if account.expires_at - now_sec < 300 {
        if let Some(ref refresh_token) = account.refresh_token {
            let refresh_res = if account.account_type == "chatgpt" {
                oauth::refresh_chatgpt_token(&state.http_client, refresh_token).await
            } else {
                oauth::refresh_antigravity_token(&state.http_client, refresh_token).await
            };
            if let Ok((new_token, expires_in)) = refresh_res {
                let new_expires_at = now_sec + expires_in;
                let _ = state.db.update_account_tokens(
                    &account.id,
                    &new_token,
                    account.refresh_token.as_deref(),
                    new_expires_at,
                );
                account.access_token = new_token;
                account.expires_at = new_expires_at;
            }
        }
    }

    if account.account_type == "chatgpt" {
        let buckets = oauth::fetch_chatgpt_quota(&state.http_client, &account.access_token)
            .await
            .unwrap_or_else(|_| vec![oauth::QuotaBucket {
                id: "chatgpt_subscription_status".to_string(),
                label: "ChatGPT 订阅有效".to_string(),
                remaining_fraction: 1.0,
                reset_time: None,
                is_exhausted: false,
            }]);
        return Ok(oauth::AccountQuota {
            account_id,
            updated_at: chrono::Utc::now().timestamp(),
            buckets,
            error: None,
        });
    }

    let url = base_url.unwrap_or_else(|| "https://cloudcode-pa.googleapis.com".to_string());

    match oauth::fetch_antigravity_quota(&state.http_client, &account.access_token, &url).await {
        Ok(buckets) => {
            let mut any_exhausted = false;
            let mut max_reset_sec = 0i64;

            for b in &buckets {
                if b.is_exhausted {
                    any_exhausted = true;
                    if let Some(ref rt) = b.reset_time {
                        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(rt) {
                            let reset_sec = dt.timestamp();
                            if reset_sec > max_reset_sec {
                                max_reset_sec = reset_sec;
                            }
                        }
                    }
                }
            }

            if any_exhausted && max_reset_sec > now_sec {
                let _ = state.db.set_account_cooldown(&account.id, max_reset_sec);
            } else if !any_exhausted && account.cooldown_until > 0 && account.cooldown_until <= now_sec {
                // 已过冷却时间或额度已恢复，解除冷却标记
                let _ = state.db.set_account_cooldown(&account.id, 0);
            }

            Ok(oauth::AccountQuota {
                account_id,
                updated_at: now_sec,
                buckets,
                error: None,
            })
        }
        Err(e) => Ok(oauth::AccountQuota {
            account_id,
            updated_at: now_sec,
            buckets: Vec::new(),
            error: Some(e),
        }),
    }
}

#[tauri::command]
pub async fn get_all_subscription_quotas(
    state: State<'_, AppState>,
    provider_id: String,
    base_url: Option<String>,
) -> Result<Vec<oauth::AccountQuota>, String> {
    let accounts = state
        .db
        .get_subscription_accounts(&provider_id)
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for acc in accounts {
        let quota = get_subscription_account_quota(
            state.clone(),
            provider_id.clone(),
            acc.id.clone(),
            base_url.clone(),
        )
        .await?;
        results.push(quota);
    }
    Ok(results)
}



