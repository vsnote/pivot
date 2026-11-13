use crate::db::presets::{self, Preset};
use crate::db::{
    DashboardRawStats, Provider, RequestLog, StatsBucketPlan, SubscriptionAccount,
};
use crate::gateway::router as gateway_router;
use crate::interceptor::{InterceptorInfo, InterceptorManager};
use crate::oauth;
use crate::state::AppState;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::Instant;

/// 前端成本估算用的模型费率表（来自 models.dev 目录）
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
pub fn get_model_capabilities(model_names: Vec<String>) -> Value {
    let mut out = serde_json::Map::new();
    for name in model_names {
        let catalog_model = crate::model_catalog::lookup(&name, None);
        let display_name = crate::model_catalog::resolve_model_display_name(&name, None);
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
                "displayName": display_name,
                "modalities": modalities,
                "contextWindow": context_window,
                "maxOutput": max_output,
                "reasoning": reasoning,
            }),
        );
    }
    Value::Object(out)
}

pub async fn get_gateway_status(state: &AppState) -> Result<Value, String> {
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

pub async fn get_providers(state: &AppState) -> Result<Vec<Provider>, String> {
    state.db.get_providers().map_err(|e| e.to_string())
}

pub async fn save_provider(state: &AppState, mut provider: Provider) -> Result<(), String> {
    crate::provider_models::validate_provider(&mut provider)?;
    state.db.save_provider(&provider).map_err(|e| e.to_string())?;
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    let _ = mgr.sync_all_if_intercepted();
    Ok(())
}

/// 重新排列服务商：列表顺序即路由优先级，返回重排后的完整列表
pub async fn reorder_providers(
    state: &AppState,
    ids: Vec<String>,
) -> Result<Vec<Provider>, String> {
    state
        .db
        .reorder_providers(&ids)
        .map_err(|e| e.to_string())?;
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    let _ = mgr.sync_all_if_intercepted();
    state.db.get_providers().map_err(|e| e.to_string())
}

/// Deletes a provider by id.
pub async fn delete_provider(
    state: &AppState,
    id: String,
) -> Result<(), String> {
    state
        .db
        .delete_provider(&id)
        .map_err(|_| format!("服务商 {} 不存在或已被删除", id))?;
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    let _ = mgr.sync_all_if_intercepted();
    Ok(())
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
        let trimmed_key = key.trim();
        if protocol == "anthropic" {
            req = req
                .header("x-api-key", trimmed_key)
                .header("authorization", format!("Bearer {}", trimmed_key));
        } else {
            req = req.header("authorization", format!("Bearer {}", trimmed_key));
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

pub async fn test_provider_latency(state: &AppState, id: String) -> Result<i64, String> {
    let providers = state.db.get_providers().map_err(|e| e.to_string())?;
    let provider = providers
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Provider not found: {}", id))?;

    let is_sub = provider.auth_type.as_deref() == Some("subscription_pool")
        || provider.preset_id.as_deref() == Some("chatgpt_subscription")
        || provider.preset_id.as_deref() == Some("antigravity")
        || provider.protocol == "chatgpt";

    if is_sub {
        let accounts = state.db.get_subscription_accounts(&provider.id).unwrap_or_default();
        if accounts.is_empty() {
            return Err("未绑定任何订阅账号，请先在编辑中添加账号".to_string());
        }
        // 尝试获取可用账号（未在冷却且能成功刷新 Token）
        match oauth::acquire_subscription_account(&state.db, &state.http_client, &provider.id).await {
            Ok(acc) => {
                let start = Instant::now();
                if provider.protocol == "chatgpt" {
                    let res = state.http_client
                        .get("https://chatgpt.com/backend-api/models")
                        .header("authorization", format!("Bearer {}", acc.access_token))
                        .header("user-agent", oauth::codex_user_agent())
                        .timeout(std::time::Duration::from_secs(5))
                        .send()
                        .await;
                    match res {
                        Ok(r) if r.status().is_success() => Ok(start.elapsed().as_millis() as i64),
                        Ok(r) if r.status() == reqwest::StatusCode::UNAUTHORIZED => {
                            if oauth::try_refresh_subscription_account(&state.db, &state.http_client, &acc.id)
                                .await
                                .is_ok()
                            {
                                Ok(start.elapsed().as_millis() as i64)
                            } else {
                                let _ = state.db.set_account_auth_invalid(&acc.id, true);
                                Err("订阅账号授权已失效，请在编辑中重新授权".to_string())
                            }
                        }
                        Ok(_) => Ok(start.elapsed().as_millis() as i64),
                        Err(e) => Err(format!("连接超时或失败: {}", e)),
                    }
                } else {
                    let base = provider.base_url.trim().trim_end_matches('/');
                    let ep = format!("{}/v1internal:fetchAvailableModels", base);
                    let res = state.http_client
                        .post(&ep)
                        .header("authorization", format!("Bearer {}", acc.access_token))
                        .header("content-type", "application/json")
                        .body("{}")
                        .timeout(std::time::Duration::from_secs(5))
                        .send()
                        .await;
                    match res {
                        Ok(r) if r.status().is_success() => Ok(start.elapsed().as_millis() as i64),
                        Ok(r) if r.status() == reqwest::StatusCode::UNAUTHORIZED => {
                            if oauth::try_refresh_subscription_account(&state.db, &state.http_client, &acc.id)
                                .await
                                .is_ok()
                            {
                                Ok(start.elapsed().as_millis() as i64)
                            } else {
                                let _ = state.db.set_account_auth_invalid(&acc.id, true);
                                Err("订阅账号授权已失效，请在编辑中重新授权".to_string())
                            }
                        }
                        Ok(_) => Ok(start.elapsed().as_millis() as i64),
                        Err(e) => Err(format!("连接超时或失败: {}", e)),
                    }
                }
            }
            Err((_, msg)) => Err(format!("账号异常: {}", msg)),
        }
    } else {
        let measured_latency = measure_http_latency(
            &state.http_client,
            &provider.protocol,
            &provider.base_url,
            provider.api_keys.first().map(String::as_str),
        )
        .await?;

        Ok(measured_latency)
    }
}

pub async fn test_endpoint_latency(
    state: &AppState,
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

pub async fn get_logs(
    state: &AppState,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<RequestLog>, String> {
    let lim = limit.unwrap_or(100);
    let off = offset.unwrap_or(0);
    state.db.get_logs(lim, off).map_err(|e| e.to_string())
}

pub fn get_logs_in_window(
    state: &AppState,
    since_epoch: Option<i64>,
    errors_only: bool,
    limit: usize,
) -> Result<Vec<RequestLog>, String> {
    state
        .db
        .get_logs_in_window(since_epoch, errors_only, limit)
        .map_err(|e| e.to_string())
}

pub fn get_dashboard_stats(
    state: &AppState,
    since_epoch: Option<i64>,
    fixed_buckets: Option<StatsBucketPlan>,
    today_start_epoch: i64,
) -> Result<DashboardRawStats, String> {
    state
        .db
        .get_dashboard_stats(since_epoch, fixed_buckets, today_start_epoch)
        .map_err(|e| e.to_string())
}

pub async fn clear_logs(state: &AppState) -> Result<(), String> {
    state.db.clear_logs().map_err(|e| e.to_string())
}

pub async fn get_interceptor_status(
    state: &AppState,
) -> Result<Vec<InterceptorInfo>, String> {
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    Ok(mgr.get_all_status())
}

pub async fn toggle_interceptor(
    state: &AppState,
    id: String,
    enable: bool,
) -> Result<InterceptorInfo, String> {
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    mgr.toggle_tool(&id, enable)
}

pub async fn set_interceptor_model(
    state: &AppState,
    id: String,
    model: Option<String>,
) -> Result<InterceptorInfo, String> {
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    mgr.set_tool_model(&id, model.as_deref())
}

pub async fn import_third_party_provider(
    state: &AppState,
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
            updated.is_default = false;
            updated.default_model = None;
            if updated.name.contains("CC-Switch") {
                updated.name = third_party.suggested_name;
            }
            if !models.is_empty() {
                updated.models = models;
            }
            crate::provider_models::validate_provider(&mut updated)?;
            state.db.save_provider(&updated).map_err(|e| e.to_string())?;
            updated
        }
        None => {
            let mut new_provider = Provider {
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
                is_default: false,
                enabled: true,
                priority: 10,
                weight: 1,
                timeout_secs: 300,
                latency_ms: None,
                created_at: None,
                updated_at: None,
            };
            crate::provider_models::validate_provider(&mut new_provider)?;
            state.db.save_provider(&new_provider).map_err(|e| e.to_string())?;
            new_provider
        }
    };

    let info = mgr.toggle_tool(&id, true)?;
    let _ = mgr.sync_codex_catalog_if_intercepted();
    let _ = mgr.sync_claude_settings_if_intercepted();
    let _ = mgr.sync_pi_models_if_intercepted();
    let _ = mgr.sync_opencode_models_if_intercepted();
    Ok((provider, info))
}

/// Explain what the gateway would do with a model name, using the very same
/// resolver the proxy runs, so the routing page cannot drift from reality.
pub async fn explain_route(
    state: &AppState,
    model: String,
) -> Result<gateway_router::RouteExplanation, String> {
    let providers = state.db.get_providers().map_err(|e| e.to_string())?;
    Ok(gateway_router::explain_route(&model, &providers))
}

pub fn get_presets() -> Vec<Preset> {
    presets::get_default_presets()
}

pub fn sync_native_tab(_tab: String) {
    // macOS native tab is managed by GPUI in desktop
}

pub fn set_native_toolbar_enabled(_enabled: bool) {
    // Toolbar is managed by GPUI in desktop
}

pub async fn fetch_provider_models(protocol: String, base_url: String, api_key: String) -> Result<Vec<String>, String> {
    crate::provider_models::fetch_models(&protocol, &base_url, &api_key).await
}

pub async fn set_provider_enabled(state: &AppState, id: String, enabled: bool) -> Result<(), String> {
    state.db.set_provider_enabled(&id, enabled).map_err(|e| e.to_string())?;
    let mgr = InterceptorManager::new(state.db.clone(), state.port);
    let _ = mgr.sync_all_if_intercepted();
    Ok(())
}

fn find_matching_existing_account(
    existing: Vec<SubscriptionAccount>,
    account_type: &str,
    email: Option<&str>,
) -> Option<SubscriptionAccount> {
    let target_email = email.map(str::trim).filter(|s| !s.is_empty());
    existing.into_iter().find(|a| {
        if a.account_type != account_type {
            return false;
        }
        let existing_email = a.email.as_deref().map(str::trim).filter(|s| !s.is_empty());
        match (target_email, existing_email) {
            (Some(t), Some(e)) => t.eq_ignore_ascii_case(e),
            (None, None) => true,
            _ => false,
        }
    })
}

async fn probe_account_capabilities(
    state: &AppState,
    provider_id: &str,
    account_type: &str,
    access_token: &str,
) -> (Option<String>, Option<Vec<String>>) {
    if account_type == "chatgpt" {
        let (models_res, plan_type) = tokio::join!(
            oauth::fetch_chatgpt_account_models(&state.http_client, access_token),
            oauth::fetch_chatgpt_plan_type(&state.http_client, access_token),
        );
        let supported_models = models_res.ok().filter(|m| !m.is_empty());
        (plan_type, supported_models)
    } else {
        let base_url = state
            .db
            .get_providers()
            .ok()
            .and_then(|list| {
                list.into_iter()
                    .find(|p| p.id == provider_id)
                    .map(|p| p.base_url)
            })
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| "https://daily-cloudcode-pa.googleapis.com".to_string());
        let supported_models =
            oauth::fetch_antigravity_models(&state.http_client, access_token, &base_url)
                .await
                .ok()
                .filter(|m| !m.is_empty());
        (None, supported_models)
    }
}

fn upsert_subscription_account_from_token(
    state: &AppState,
    provider_id: String,
    account_type: String,
    token_resp: oauth::TokenResponse,
    plan_type: Option<String>,
    supported_models: Option<Vec<String>>,
) -> Result<SubscriptionAccount, String> {
    let now_sec = chrono::Utc::now().timestamp();
    let expires_at = now_sec + token_resp.expires_in;
    let now_str = chrono::Utc::now().to_rfc3339();

    let existing_accounts = state.db.get_subscription_accounts(&provider_id).unwrap_or_default();
    if let Some(target) = find_matching_existing_account(
        existing_accounts,
        &account_type,
        token_resp.email.as_deref(),
    ) {
        state
            .db
            .update_account_tokens(
                &target.id,
                &token_resp.access_token,
                token_resp.refresh_token.as_deref(),
                expires_at,
            )
            .map_err(|e| e.to_string())?;

        let _ = state.db.set_account_cooldown(&target.id, 0);
        if let Some(ref pt) = plan_type {
            let _ = state.db.update_account_plan_type(&target.id, pt);
        }
        if let Some(ref models) = supported_models {
            let _ = state.db.update_account_supported_models(&target.id, models);
        }

        let mut updated = target;
        updated.access_token = token_resp.access_token;
        if let Some(rf) = token_resp.refresh_token {
            updated.refresh_token = Some(rf);
        }
        updated.expires_at = expires_at;
        updated.cooldown_until = 0;
        updated.auth_invalid = false;
        if plan_type.is_some() {
            updated.plan_type = plan_type;
        }
        if supported_models.is_some() {
            updated.supported_models = supported_models;
        }
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
        plan_type,
        supported_models,
        auth_invalid: false,
        created_at: now_str.clone(),
        updated_at: now_str,
    };

    state
        .db
        .save_subscription_account(&account)
        .map_err(|e| e.to_string())?;
    Ok(account)
}

pub async fn start_subscription_oauth(
    state: &AppState,
    provider_id: String,
    account_type: String,
) -> Result<SubscriptionAccount, String> {
    let token_resp = if account_type == "chatgpt" {
        let pkce = oauth::generate_pkce();
        let state_str = uuid::Uuid::new_v4().to_string();
        let auth_url = oauth::build_chatgpt_auth_url(&pkce, &state_str);

        open::that(&auth_url).map_err(|e| format!("无法打开浏览器: {}", e))?;

        let code = oauth::listen_for_chatgpt_code(&state_str, 120).await?;
        oauth::exchange_chatgpt_code(&state.http_client, &code, &pkce.verifier).await?
    } else {
        let pkce = oauth::generate_pkce();
        let state_str = uuid::Uuid::new_v4().to_string();
        let auth_url = oauth::build_auth_url(&pkce, &state_str);

        open::that(&auth_url).map_err(|e| format!("无法打开浏览器: {}", e))?;

        let code = oauth::listen_for_code(&state_str, 120).await?;
        oauth::exchange_code_for_tokens(&state.http_client, &code, &pkce.verifier).await?
    };

    let (plan_type, supported_models) = probe_account_capabilities(
        state,
        &provider_id,
        &account_type,
        &token_resp.access_token,
    )
    .await;

    upsert_subscription_account_from_token(
        state,
        provider_id,
        account_type,
        token_resp,
        plan_type,
        supported_models,
    )
}

pub async fn import_codex_subscription_account(
    state: &AppState,
    provider_id: String,
) -> Result<SubscriptionAccount, String> {
    let token_resp = oauth::import_local_codex_account(&state.http_client).await?;

    let (plan_type, supported_models) = probe_account_capabilities(
        state,
        &provider_id,
        "chatgpt",
        &token_resp.access_token,
    )
    .await;

    upsert_subscription_account_from_token(
        state,
        provider_id,
        "chatgpt".to_string(),
        token_resp,
        plan_type,
        supported_models,
    )
}

pub fn get_subscription_accounts(
    state: &AppState,
    provider_id: String,
) -> Result<Vec<SubscriptionAccount>, String> {
    state
        .db
        .get_subscription_accounts(&provider_id)
        .map_err(|e| e.to_string())
}

pub fn delete_subscription_account(
    state: &AppState,
    account_id: String,
) -> Result<(), String> {
    state
        .db
        .delete_subscription_account(&account_id)
        .map_err(|e| e.to_string())
}

pub fn toggle_subscription_account(
    state: &AppState,
    _provider_id: String,
    account_id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .db
        .set_account_enabled(&account_id, enabled)
        .map_err(|e| e.to_string())
}

/// 同步服务商下所有已启用订阅账号的模型列表，更新各账号自身的 supported_models 并返回去重并集
pub async fn sync_subscription_models(
    state: &AppState,
    provider_id: String,
    base_url: String,
) -> Result<Vec<String>, String> {
    let accounts = state
        .db
        .get_subscription_accounts(&provider_id)
        .map_err(|e| e.to_string())?;
    let enabled_accounts: Vec<_> = accounts.into_iter().filter(|a| a.enabled).collect();
    if enabled_accounts.is_empty() {
        return Err("请先连接并启用至少一个订阅账号".to_string());
    }

    let mut union_models = Vec::new();
    let mut union_seen = std::collections::HashSet::new();
    let mut last_err = String::new();
    let mut is_chatgpt_pool = true;

    for mut account in enabled_accounts.clone() {
        if account.account_type != "chatgpt" {
            is_chatgpt_pool = false;
        }

        // 若 Token 临期，使用单飞锁自动续期后再探测模型
        match oauth::ensure_or_refresh_account_token(&state.db, &state.http_client, &account.id, None).await {
            Ok(refreshed) => {
                account = refreshed;
            }
            Err(e) => {
                last_err = format!(
                    "账号 {} Token 刷新失败: {}",
                    account.email.as_deref().unwrap_or(&account.id),
                    e
                );
                continue;
            }
        }

        let fetched_res = if account.account_type == "chatgpt" {
            let (models_res, plan_type) = tokio::join!(
                oauth::fetch_chatgpt_account_models(&state.http_client, &account.access_token),
                oauth::fetch_chatgpt_plan_type(&state.http_client, &account.access_token),
            );
            if let Some(ref pt) = plan_type {
                let _ = state.db.update_account_plan_type(&account.id, pt);
            }
            models_res
        } else {
            oauth::fetch_antigravity_models(&state.http_client, &account.access_token, &base_url)
                .await
        };

        match fetched_res {
            Ok(models) if !models.is_empty() => {
                let _ = state
                    .db
                    .update_account_supported_models(&account.id, &models);
                for m in models {
                    let trimmed = m.trim().to_string();
                    if !trimmed.is_empty() && union_seen.insert(trimmed.clone()) {
                        union_models.push(trimmed);
                    }
                }
            }
            Ok(_) => {
                last_err = "上游未返回可用模型".to_string();
            }
            Err(e) => {
                if oauth::is_token_revoked_or_invalid(&e) {
                    let _ = state.db.set_account_auth_invalid(&account.id, true);
                }
                last_err = e;
            }
        }
    }

    if !union_models.is_empty() {
        return Ok(union_models);
    }

    // 所有账号本次网络拉取均失败时，优先汇总各启用账号已持久化的 supported_models 并集
    for account in &enabled_accounts {
        if let Some(ref cached) = account.supported_models {
            for m in cached {
                let trimmed = m.trim().to_string();
                if !trimmed.is_empty() && union_seen.insert(trimmed.clone()) {
                    union_models.push(trimmed);
                }
            }
        }
    }
    if !union_models.is_empty() {
        return Ok(union_models);
    }

    if is_chatgpt_pool {
        if let Some(local) = oauth::get_local_codex_cached_models() {
            if !local.is_empty() {
                return Ok(local);
            }
        }
        if let Some(acc) = enabled_accounts.first() {
            return oauth::fetch_chatgpt_models(&state.http_client, &acc.access_token).await;
        }
    }

    Err(if last_err.is_empty() {
        "未能获取到任何可用订阅模型".to_string()
    } else {
        last_err
    })
}

pub async fn get_subscription_account_quota(
    state: &AppState,
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
    match oauth::ensure_or_refresh_account_token(&state.db, &state.http_client, &account.id, None).await {
        Ok(refreshed) => {
            account = refreshed;
        }
        Err(e) => {
            let is_auth_err = account.auth_invalid || oauth::is_token_revoked_or_invalid(&e);
            return Ok(oauth::AccountQuota {
                account_id,
                updated_at: now_sec,
                buckets: Vec::new(),
                reset_credits: None,
                plan_type: account.plan_type.clone(),
                error: Some(if is_auth_err {
                    "账号授权已失效，请重新授权".to_string()
                } else {
                    e
                }),
            });
        }
    }

    let (buckets, reset_credits, detected_plan) = if account.account_type == "chatgpt" {
        match oauth::fetch_chatgpt_quota(&state.http_client, &account.access_token).await {
            Ok((b, rc, pt)) => {
                if let Some(ref p) = pt {
                    let _ = state.db.update_account_plan_type(&account.id, p);
                }
                (b, Some(rc), pt.or_else(|| account.plan_type.clone()))
            }
            Err(e) => {
                let is_auth_err = oauth::is_token_revoked_or_invalid(&e);
                if is_auth_err {
                    let _ = state.db.set_account_auth_invalid(&account.id, true);
                }
                return Ok(oauth::AccountQuota {
                    account_id,
                    updated_at: now_sec,
                    buckets: Vec::new(),
                    reset_credits: None,
                    plan_type: account.plan_type.clone(),
                    error: Some(if is_auth_err { "账号授权已失效，请重新授权".to_string() } else { e }),
                });
            }
        }
    } else {
        let url = base_url
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "https://cloudcode-pa.googleapis.com".to_string());
        match oauth::fetch_antigravity_quota(&state.http_client, &account.access_token, &url).await {
            Ok(b) => (b, None, account.plan_type.clone()),
            Err(e) => {
                let is_auth_err = oauth::is_token_revoked_or_invalid(&e);
                if is_auth_err {
                    let _ = state.db.set_account_auth_invalid(&account.id, true);
                }
                return Ok(oauth::AccountQuota {
                    account_id,
                    updated_at: now_sec,
                    buckets: Vec::new(),
                    reset_credits: None,
                    plan_type: account.plan_type.clone(),
                    error: Some(if is_auth_err { "账号授权已失效，请重新授权".to_string() } else { e }),
                });
            }
        }
    };

    if account.account_type == "chatgpt" {
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
        } else if !any_exhausted && account.cooldown_until > 0 {
            let _ = state.db.set_account_cooldown(&account.id, 0);
        }
    } else {
        // Antigravity 多模型族独立配额：仅各自冷却耗尽的模型族，全部桶耗尽时才写账号级冷却（取最早恢复时间）
        for family in ["claude", "gemini"] {
            let family_buckets: Vec<_> = buckets
                .iter()
                .filter(|b| b.label.to_lowercase().contains(family))
                .collect();
            if !family_buckets.is_empty() {
                if let Some(max_f_reset) = family_buckets
                    .iter()
                    .filter(|b| b.is_exhausted)
                    .filter_map(|b| {
                        b.reset_time
                            .as_deref()
                            .and_then(|rt| chrono::DateTime::parse_from_rfc3339(rt).ok())
                            .map(|dt| dt.timestamp())
                    })
                    .filter(|&ts| ts > now_sec)
                    .max()
                {
                    oauth::mark_account_family_cooldown_until(&account.id, family, max_f_reset);
                } else if family_buckets.iter().all(|b| !b.is_exhausted) {
                    oauth::clear_account_family_cooldown(&account.id, family);
                }
            }
        }

        let all_exhausted = !buckets.is_empty() && buckets.iter().all(|b| b.is_exhausted);
        if all_exhausted {
            let min_reset_sec = buckets
                .iter()
                .filter_map(|b| {
                    b.reset_time
                        .as_deref()
                        .and_then(|rt| chrono::DateTime::parse_from_rfc3339(rt).ok())
                        .map(|dt| dt.timestamp())
                })
                .filter(|&ts| ts > now_sec)
                .min()
                .unwrap_or(0);
            if min_reset_sec > now_sec {
                let _ = state.db.set_account_cooldown(&account.id, min_reset_sec);
            }
        } else if account.cooldown_until > 0 {
            let _ = state.db.set_account_cooldown(&account.id, 0);
        }
    }

    Ok(oauth::AccountQuota {
        account_id,
        updated_at: now_sec,
        buckets,
        reset_credits,
        plan_type: detected_plan,
        error: None,
    })
}

pub async fn consume_subscription_reset_credit(
    state: &AppState,
    provider_id: String,
    account_id: String,
) -> Result<oauth::AccountQuota, String> {
    let accounts = state
        .db
        .get_subscription_accounts(&provider_id)
        .map_err(|e| e.to_string())?;

    let mut account = accounts
        .into_iter()
        .find(|a| a.id == account_id)
        .ok_or_else(|| "未找到指定订阅账号".to_string())?;

    // 检查 Token 是否需要续期
    if let Ok(refreshed) =
        oauth::ensure_or_refresh_account_token(&state.db, &state.http_client, &account.id, None).await
    {
        account = refreshed;
    }

    oauth::consume_chatgpt_reset_credit(&state.http_client, &account.access_token).await?;

    // 重置本地冷却时间
    let _ = state.db.set_account_cooldown(&account.id, 0);

    // 重新拉取最新额度并返回
    let (buckets, reset_credits, plan_type) =
        oauth::fetch_chatgpt_quota(&state.http_client, &account.access_token).await?;
    if let Some(ref p) = plan_type {
        let _ = state.db.update_account_plan_type(&account.id, p);
    }

    Ok(oauth::AccountQuota {
        account_id,
        updated_at: chrono::Utc::now().timestamp(),
        buckets,
        reset_credits: Some(reset_credits),
        plan_type: plan_type.or(account.plan_type),
        error: None,
    })
}

pub async fn get_all_subscription_quotas(
    state: &AppState,
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
            state,
            provider_id.clone(),
            acc.id.clone(),
            base_url.clone(),
        )
        .await?;
        results.push(quota);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sub(id: &str, account_type: &str, email: Option<&str>) -> SubscriptionAccount {
        SubscriptionAccount {
            id: id.to_string(),
            provider_id: "chatgpt_subscription".to_string(),
            account_type: account_type.to_string(),
            email: email.map(str::to_string),
            access_token: "tok".to_string(),
            refresh_token: None,
            expires_at: 9_999_999_999,
            cooldown_until: 0,
            enabled: true,
            plan_type: None,
            supported_models: None,
            auth_invalid: false,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    #[test]
    fn chatgpt_accounts_with_different_emails_coexist_without_overwriting() {
        let existing = vec![sample_sub("sub_plus", "chatgpt", Some("plus@example.com"))];

        // 不同邮箱的 ChatGPT 账号不能命中覆盖
        assert!(find_matching_existing_account(
            existing.clone(),
            "chatgpt",
            Some("pro@example.com")
        )
        .is_none());

        // 同邮箱（忽略大小写）命中更新
        let matched =
            find_matching_existing_account(existing, "chatgpt", Some("PLUS@example.com")).unwrap();
        assert_eq!(matched.id, "sub_plus");
    }
}



