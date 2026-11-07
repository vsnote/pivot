use crate::db::{Database, LogAttempt, RequestLog};
use crate::gateway::adapters::{
    anthropic_to_gemini, anthropic_to_openai, finish_gemini_stream, finish_openai_stream,
    openai_to_anthropic, parse_anthropic_event_to_openai_chunks,
    parse_gemini_chunk_to_anthropic_events, parse_openai_chunk_to_anthropic_events, responses,
    SseState,
};
use crate::gateway::router::{RouteCandidate, RoutingEngine};
use crate::gateway::types::*;
use async_stream::stream;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Emitter;
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug, Default)]
pub struct GatewayStatsInternal {
    pub is_running: AtomicBool,
    pub active_requests: AtomicI64,
    pub total_requests: AtomicU64,
    pub total_prompt_tokens: AtomicU64,
    pub total_completion_tokens: AtomicU64,
    pub error_count: AtomicU64,
    pub start_time: std::sync::Mutex<Option<Instant>>,
}

#[derive(Clone)]
pub struct GatewayState {
    pub db: Database,
    pub router: Arc<RoutingEngine>,
    pub http_client: reqwest::Client,
    pub stats: Arc<GatewayStatsInternal>,
    pub app_handle: Option<tauri::AppHandle>,
}

struct DispatchResult {
    response: Response,
    prompt_tokens: u32,
    completion_tokens: u32,
}

fn estimate_text_tokens(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let mut cjk_count = 0usize;
    let mut other_count = 0usize;
    for c in text.chars() {
        if ('\u{4e00}'..='\u{9fff}').contains(&c)
            || ('\u{3400}'..='\u{4dbf}').contains(&c)
            || ('\u{3000}'..='\u{303f}').contains(&c)
        {
            cjk_count += 1;
        } else {
            other_count += 1;
        }
    }
    let tokens = (cjk_count as f64 * 1.1 + other_count as f64 * 0.28).ceil() as u32;
    tokens.max(1)
}

fn estimate_anthropic_req_prompt_tokens(req: &AnthropicMessageRequest) -> u32 {
    let mut total: u32 = 20;
    if let Some(ref sys) = req.system {
        match sys {
            Value::String(s) => total += estimate_text_tokens(s),
            Value::Array(arr) => {
                for item in arr {
                    if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                        total += estimate_text_tokens(t);
                    }
                }
            }
            _ => total += estimate_text_tokens(&sys.to_string()),
        }
    }
    for m in &req.messages {
        total += 4;
        match &m.content {
            Value::String(s) => total += estimate_text_tokens(s),
            Value::Array(arr) => {
                for item in arr {
                    if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                        total += estimate_text_tokens(t);
                    } else if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                        total += estimate_text_tokens(content);
                    }
                }
            }
            _ => total += estimate_text_tokens(&m.content.to_string()),
        }
    }
    if let Some(ref tools) = req.tools {
        for t in tools {
            total += 15 + estimate_text_tokens(&t.name) + estimate_text_tokens(t.description.as_deref().unwrap_or(""));
            total += estimate_text_tokens(&t.input_schema.to_string());
        }
    }
    total
}

fn estimate_openai_req_prompt_tokens(req: &OpenAIChatRequest) -> u32 {
    let mut total: u32 = 20;
    for m in &req.messages {
        total += 4;
        if let Some(ref content) = m.content {
            match content {
                Value::String(s) => total += estimate_text_tokens(s),
                _ => total += estimate_text_tokens(&content.to_string()),
            }
        }
    }
    if let Some(ref tools) = req.tools {
        for t in tools {
            total += 15 + estimate_text_tokens(&t.function.name) + estimate_text_tokens(t.function.description.as_deref().unwrap_or(""));
            total += estimate_text_tokens(&t.function.parameters.to_string());
        }
    }
    total
}

/// Normalize OpenAIChatRequest before sending to upstream OpenAI-compatible endpoints.
/// 1. Roles: translates "developer" -> "system", "model" -> "assistant", "human" -> "user"
///    (e.g. DeepSeek, Ollama, Groq, Mistral, Moonshot reject "developer" with 422).
/// 2. Content: flattens text-only array contents into single strings
///    (e.g. DeepSeek rejects array content with "invalid type: sequence, expected a string").
/// 3. Tokens & Stream: reconciles max_tokens/max_completion_tokens and strips orphaned stream_options/tools.
fn normalize_openai_req_for_upstream(req: &mut OpenAIChatRequest) {
    if req.max_tokens.is_none() && req.max_completion_tokens.is_some() {
        req.max_tokens = req.max_completion_tokens.take();
    } else if req.max_tokens.is_some() {
        req.max_completion_tokens = None;
    }

    if req.stream != Some(true) {
        req.stream_options = None;
    }

    if req.tools.as_ref().map(|t| t.is_empty()).unwrap_or(false) {
        req.tools = None;
    }
    if req.tools.is_none() {
        req.tool_choice = None;
    }

    if let Some(ref mut effort) = req.reasoning_effort {
        if effort.is_empty() {
            req.reasoning_effort = None;
        } else if effort.eq_ignore_ascii_case("ultra") {
            *effort = "max".to_string();
        }
    }

    for msg in &mut req.messages {
        match msg.role.as_str() {
            "developer" => msg.role = "system".to_string(),
            "model" => msg.role = "assistant".to_string(),
            "human" => msg.role = "user".to_string(),
            _ => {}
        }

        if msg.reasoning_content.as_ref().map(|s| s.is_empty()).unwrap_or(false) {
            msg.reasoning_content = None;
        }

        if let Some(ref c) = msg.content {
            if c.is_null() && msg.role != "assistant" {
                msg.content = Some(Value::String(String::new()));
            } else if let Some(arr) = c.as_array() {
                let has_multimodal = arr.iter().any(|item| {
                    if let Some(obj) = item.as_object() {
                        let t = obj.get("type").and_then(Value::as_str).unwrap_or("");
                        t == "image_url"
                            || t == "input_image"
                            || t == "image"
                            || t == "input_audio"
                            || t == "input_file"
                    } else {
                        false
                    }
                });

                if !has_multimodal {
                    let mut text_parts = Vec::new();
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            text_parts.push(s.to_string());
                        } else if let Some(text) = item.get("text").and_then(Value::as_str) {
                            text_parts.push(text.to_string());
                        } else if let Some(content) = item.get("content").and_then(Value::as_str) {
                            text_parts.push(content.to_string());
                        }
                    }
                    msg.content = Some(Value::String(text_parts.join("\n")));
                }
            }
        }
    }
}

pub fn create_gateway_router(state: GatewayState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_check))
        .route("/v1/health", get(health_check))
        .route("/v1/models", get(list_models))
        .route("/v1/messages", post(handle_messages))
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/responses", post(handle_responses).get(handle_responses_get))
        .route("/v1beta/models", get(handle_gemini_list_models))
        .route("/v1beta/models/{*action}", post(handle_gemini_generate_content).get(handle_gemini_get_model))
        .route("/models", get(handle_gemini_list_models))
        .route("/models/{*action}", post(handle_gemini_generate_content).get(handle_gemini_get_model))
        .route("/api/stats", get(get_stats))
        .layer(cors)
        .layer(axum::extract::DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(tower_http::decompression::RequestDecompressionLayer::new().pass_through_unaccepted(true))
        .with_state(state)
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "Pivot Local Gateway",
        "version": "0.1.0"
    }))
}

async fn get_stats(State(state): State<GatewayState>) -> Json<Value> {
    let uptime = state
        .stats
        .start_time
        .lock()
        .unwrap()
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);
    Json(json!({
        "activeRequests": state.stats.active_requests.load(Ordering::Relaxed),
        "totalRequests": state.stats.total_requests.load(Ordering::Relaxed),
        "totalPromptTokens": state.stats.total_prompt_tokens.load(Ordering::Relaxed),
        "totalCompletionTokens": state.stats.total_completion_tokens.load(Ordering::Relaxed),
        "errorCount": state.stats.error_count.load(Ordering::Relaxed),
        "uptimeSecs": uptime,
    }))
}

async fn list_models(State(state): State<GatewayState>) -> Json<Value> {
    let providers = state.db.get_providers().unwrap_or_default();
    let mut model_list = Vec::new();

    for p in providers.iter().filter(|p| p.enabled) {
        for m in &p.models {
            model_list.push(json!({
                "id": m,
                "object": "model",
                "created": 1700000000,
                "owned_by": p.id,
                "permission": [],
                "root": m,
                "parent": null
            }));
            // Also expose prefix alias, e.g. "openai/gpt-4o"
            model_list.push(json!({
                "id": format!("{}/{}", p.id, m),
                "object": "model",
                "created": 1700000000,
                "owned_by": p.id,
                "permission": [],
                "root": m,
                "parent": null
            }));
            let provider_prefix = p.name.to_lowercase().replace(' ', "-");
            let named_id = format!("{}/{}", provider_prefix, m);
            if named_id != *m {
                model_list.push(json!({
                    "id": named_id,
                    "object": "model",
                    "created": 1700000000,
                    "owned_by": p.id,
                    "permission": [],
                    "root": m,
                    "parent": null
                }));
            }
        }
    }

    Json(json!({
        "object": "list",
        "data": model_list
    }))
}

async fn handle_gemini_list_models(State(state): State<GatewayState>) -> Json<Value> {
    let providers = state.db.get_providers().unwrap_or_default();
    let mut model_list = Vec::new();

    for p in providers.iter().filter(|p| p.enabled) {
        for m in &p.models {
            model_list.push(json!({
                "name": format!("models/{}", m),
                "version": "001",
                "displayName": m,
                "description": format!("Model {} via Pivot provider {}", m, p.name),
                "inputTokenLimit": 1048576,
                "outputTokenLimit": 8192,
                "supportedGenerationMethods": ["generateContent", "streamGenerateContent", "countTokens"]
            }));
        }
    }

    Json(json!({ "models": model_list }))
}

async fn handle_gemini_get_model(
    State(_state): State<GatewayState>,
    Path(action): Path<String>,
) -> Response {
    let clean_action = action.trim_start_matches('/');
    let model_id = clean_action.strip_prefix("models/").unwrap_or(clean_action);
    Json(json!({
        "name": format!("models/{}", model_id),
        "version": "001",
        "displayName": model_id,
        "inputTokenLimit": 1048576,
        "outputTokenLimit": 8192,
        "supportedGenerationMethods": ["generateContent", "streamGenerateContent", "countTokens"]
    })).into_response()
}

fn extract_session_id(headers: &HeaderMap) -> Option<String> {
    for key in &[
        "x-session-id",
        "session-id",
        "anthropic-session-id",
        "x-claude-session-id",
        "x-codex-session-id",
        "authorization",
    ] {
        if let Some(val) = headers.get(*key).and_then(|v| v.to_str().ok()) {
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn extract_client_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(val) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }
    if let Some(val) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        let trimmed = val.trim_start_matches("Bearer ").trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn detect_client_type(headers: &HeaderMap, default_type: &str) -> String {
    if let Some(ua) = headers.get("user-agent").and_then(|v| v.to_str().ok()) {
        let ua_lower = ua.to_lowercase();
        if ua_lower.contains("pi (") || ua_lower.starts_with("pi/") || ua_lower == "pi" {
            return "pi".to_string();
        }
        if ua_lower.contains("claude-code") || ua_lower.contains("claudecode") {
            return "claude_code".to_string();
        }
        if ua_lower.contains("codex") {
            return "codex_cli".to_string();
        }
        if ua_lower.contains("gemini") {
            return "gemini_cli".to_string();
        }
    }
    default_type.to_string()
}

fn resolve_forward_user_agent(client_ua: Option<&str>, client_type: &str) -> String {
    if let Some(ua) = client_ua.filter(|s| !s.trim().is_empty()) {
        return ua.to_string();
    }
    match client_type {
        "codex_cli" => crate::oauth::codex_user_agent(),
        "claude_code" => "Claude-Code/0.2.29".to_string(),
        "gemini_cli" => "gemini-cli/1.0.0".to_string(),
        "pi" => "pi/1.0.0".to_string(),
        _ => format!("Pivot/{} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::OS),
    }
}

/// Handler for Claude Code and Anthropic clients calling `/v1/messages`
async fn handle_messages(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(req): Json<AnthropicMessageRequest>,
) -> Response {
    let start_time = Instant::now();
    let req_id = uuid::Uuid::new_v4().to_string();
    state.stats.active_requests.fetch_add(1, Ordering::Relaxed);
    state.stats.total_requests.fetch_add(1, Ordering::Relaxed);

    let client_type = detect_client_type(&headers, "claude_code");
    let session_id = extract_session_id(&headers);
    let client_key = extract_client_api_key(&headers);
    let client_ua = headers.get("user-agent").and_then(|v| v.to_str().ok());
    let providers = state.db.get_providers().unwrap_or_default();

    let is_stream = req.stream.unwrap_or(false);
    let route_match =
        match state
            .router
            .resolve_route(&req.model, session_id.as_deref(), &providers)
        {
            Ok(rm) => rm,
            Err(err) => {
                state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
                state.stats.error_count.fetch_add(1, Ordering::Relaxed);
                // A routing failure is the first thing users look for in the log
                // table, so record it even though no upstream was contacted.
                log_request(
                    &state,
                    RequestLog {
                        id: req_id.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        method: "POST".to_string(),
                        path: "/v1/messages".to_string(),
                        client_type: client_type.clone(),
                        incoming_model: req.model.clone(),
                        routed_provider_id: "none".to_string(),
                        routed_provider_name: None,
                        routed_model: String::new(),
                        status_code: StatusCode::BAD_GATEWAY.as_u16() as i32,
                        latency_ms: start_time.elapsed().as_millis() as i64,
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        error_message: Some(err.clone()),
                        is_stream,
                        fallback_used: false,
                        attempts: Vec::new(),
                    },
                );
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "error": { "message": err, "type": "routing_error" } })),
                )
                    .into_response();
            }
        };

    let mut candidates = vec![route_match.primary];
    candidates.extend(route_match.fallbacks);

    let mut attempts: Vec<LogAttempt> = Vec::new();
    let mut last_error = String::new();
    let mut last_status = StatusCode::BAD_GATEWAY;
    let mut fallback_used = false;

    for (cand_idx, cand) in candidates.iter().enumerate() {
        if cand_idx > 0 {
            fallback_used = true;
            tracing::warn!(
                "Failing over to fallback candidate {} ({})",
                cand.provider.name,
                cand.target_model
            );
        }

        let attempt_started = Instant::now();
        let resp_result = dispatch_anthropic_request(
            &state,
            &req,
            cand,
            &req_id,
            is_stream,
            client_key.as_deref(),
            client_ua,
            &client_type,
        )
        .await;

        match resp_result {
            Ok(result) => {
                let latency_ms = start_time.elapsed().as_millis() as i64;
                attempts.push(attempt(
                    cand,
                    StatusCode::OK,
                    None,
                    attempt_started.elapsed().as_millis() as i64,
                ));
                state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
                state
                    .stats
                    .total_prompt_tokens
                    .fetch_add(result.prompt_tokens as u64, Ordering::Relaxed);
                state
                    .stats
                    .total_completion_tokens
                    .fetch_add(result.completion_tokens as u64, Ordering::Relaxed);

                // Log the successful request
                log_request(
                    &state,
                    RequestLog {
                        id: req_id.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        method: "POST".to_string(),
                        path: "/v1/messages".to_string(),
                        client_type: client_type.clone(),
                        incoming_model: req.model.clone(),
                        routed_provider_id: cand.provider.id.clone(),
                        routed_provider_name: Some(cand.provider.name.clone()),
                        routed_model: cand.target_model.clone(),
                        status_code: 200,
                        latency_ms,
                        prompt_tokens: result.prompt_tokens as i32,
                        completion_tokens: result.completion_tokens as i32,
                        error_message: None,
                        is_stream,
                        fallback_used,
                        attempts,
                    },
                );

                return result.response;
            }
            Err((status, err_msg)) => {
                attempts.push(attempt(
                    cand,
                    status,
                    Some(summarize_error(&err_msg)),
                    attempt_started.elapsed().as_millis() as i64,
                ));
                last_error = err_msg;
                last_status = status;
                // If 4xx client errors (except 429), don't fallback
                if status == StatusCode::BAD_REQUEST || status == StatusCode::UNAUTHORIZED {
                    break;
                }
            }
        }
    }

    state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
    state.stats.error_count.fetch_add(1, Ordering::Relaxed);
    let latency_ms = start_time.elapsed().as_millis() as i64;

    log_request(
        &state,
        RequestLog {
            id: req_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            method: "POST".to_string(),
            path: "/v1/messages".to_string(),
            client_type,
            incoming_model: req.model.clone(),
            routed_provider_id: candidates.first().map(|c| c.provider.id.clone()).unwrap_or_else(|| "none".to_string()),
            routed_provider_name: candidates.first().map(|c| c.provider.name.clone()),
            routed_model: candidates.first().map(|c| c.target_model.clone()).unwrap_or_default(),
            status_code: last_status.as_u16() as i32,
            latency_ms,
            prompt_tokens: 0,
            completion_tokens: 0,
            error_message: Some(last_error.clone()),
            is_stream,
            fallback_used,
            attempts,
        },
    );

    (
        last_status,
        Json(json!({ "error": { "message": last_error, "type": "upstream_error" } })),
    )
        .into_response()
}

fn resolve_upstream_endpoint(base_url: &str, suffix: &str) -> String {
    let clean = base_url.trim_end_matches('/');
    if clean.ends_with(suffix) {
        return clean.to_string();
    }
    let url_parsed = reqwest::Url::parse(clean);
    let path = url_parsed.as_ref().map(|u| u.path().trim_end_matches('/')).unwrap_or("");
    if path.is_empty() || path == "/api" {
        format!("{clean}/v1{suffix}")
    } else {
        format!("{clean}{suffix}")
    }
}

fn resolve_dispatch_timeout(cand: &RouteCandidate, estimated_prompt: u32) -> Duration {
    let configured = cand.provider.timeout_secs.max(10) as u64;
    // 对 Gemini、Antigravity 订阅池等超大上下文与深度思考模型，动态扩展超时，防止 400k~1M Token 提示词在 60s 临界被强掐
    if cand.provider.protocol == "gemini" || cand.provider.auth_type.as_deref() == Some("subscription_pool") {
        let base = configured.max(180);
        let extra_scale = ((estimated_prompt as u64) / 50_000) * 15;
        Duration::from_secs((base + extra_scale).min(600))
    } else {
        Duration::from_secs(configured)
    }
}

async fn dispatch_anthropic_request(
    state: &GatewayState,
    req: &AnthropicMessageRequest,
    cand: &RouteCandidate,
    req_id: &str,
    is_stream: bool,
    client_key: Option<&str>,
    client_ua: Option<&str>,
    client_type: &str,
) -> Result<DispatchResult, (StatusCode, String)> {
    let client = &state.http_client;
    let estimated_prompt = estimate_anthropic_req_prompt_tokens(req);
    let timeout = resolve_dispatch_timeout(cand, estimated_prompt);
    let effective_key = cand.api_key.as_deref().or(client_key);
    let forward_ua = resolve_forward_user_agent(client_ua, client_type);

    match cand.provider.protocol.as_str() {
        "anthropic" => {
            // Direct Anthropic upstream
            let url = resolve_upstream_endpoint(&cand.provider.base_url, "/messages");

            let mut req_body = req.clone();
            req_body.model = cand.target_model.clone();

            let mut req_builder = client
                .post(&url)
                .timeout(timeout)
                .header("user-agent", &forward_ua)
                .header("content-type", "application/json")
                .header("anthropic-version", "2023-06-01");

            if let Some(key) = effective_key {
                req_builder = req_builder
                    .header("x-api-key", key)
                    .header("authorization", format!("Bearer {}", key));
            }

            let res = req_builder.json(&req_body).send().await.map_err(|e| {
                (
                    StatusCode::BAD_GATEWAY,
                    format!("Upstream connection failed: {}", e),
                )
            })?;

            if !res.status().is_success() {
                let status =
                    StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                let text = res.text().await.unwrap_or_default();
                return Err((status, text));
            }

            if is_stream {
                let db_clone = state.db.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();
                let req_id_str = req_id.to_string();
                let byte_stream = res.bytes_stream();

                let sse_stream = stream! {
                    let mut buffer = String::new();
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;
                    let mut estimated_output_chars = 0usize;

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            if let Ok(s) = std::str::from_utf8(&bytes) {
                                buffer.push_str(s);

                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();

                                    if line.starts_with("data:") {
                                        let payload = line.trim_start_matches("data:").trim();
                                        if let Ok(val) = serde_json::from_str::<Value>(payload) {
                                            if let Some(msg) = val.get("message") {
                                                if let Some(u) = msg.get("usage") {
                                                    if let Some(it) = u.get("input_tokens").and_then(Value::as_u64) {
                                                        if it > 0 { final_prompt_tokens = it as u32; }
                                                    }
                                                }
                                            }
                                            if let Some(u) = val.get("usage") {
                                                if let Some(ot) = u.get("output_tokens").and_then(Value::as_u64) {
                                                    if ot > 0 { final_completion_tokens = ot as u32; }
                                                }
                                            }
                                            if let Some(delta) = val.get("delta") {
                                                if let Some(text) = delta.get("text").and_then(Value::as_str) {
                                                    estimated_output_chars += text.len();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            yield Ok::<_, std::io::Error>(bytes);
                        }
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = (estimated_output_chars as f64 * 0.3).ceil() as u32;
                        if final_completion_tokens == 0 { final_completion_tokens = 1; }
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    let _ = db_clone.update_request_log_tokens(
                        &req_id_str,
                        final_prompt_tokens as i32,
                        final_completion_tokens as i32,
                        stream_latency,
                    );
                    if final_prompt_tokens > estimated_prompt {
                        stats_clone.total_prompt_tokens.fetch_add((final_prompt_tokens - estimated_prompt) as u64, Ordering::Relaxed);
                    }
                    stats_clone.total_completion_tokens.fetch_add(final_completion_tokens as u64, Ordering::Relaxed);
                };

                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(sse_stream))
                        .unwrap(),
                    prompt_tokens: estimated_prompt,
                    completion_tokens: 0,
                })
            } else {
                let bytes = res
                    .bytes()
                    .await
                    .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
                let usage = serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .and_then(|body| body.get("usage").cloned());
                let prompt_tokens = usage
                    .as_ref()
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(Value::as_u64)
                    .map(|v| v as u32)
                    .unwrap_or(estimated_prompt);
                let completion_tokens = usage
                    .as_ref()
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32;

                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "application/json")
                        .body(Body::from(bytes))
                        .unwrap(),
                    prompt_tokens,
                    completion_tokens,
                })
            }
        }
        "openai" | "openrouter" | "ollama" => {
            // Convert to OpenAI Chat Completions
            let url = resolve_upstream_endpoint(&cand.provider.base_url, "/chat/completions");

            let mut openai_req =
                anthropic_to_openai::convert_anthropic_req_to_openai(req, &cand.target_model);
            normalize_openai_req_for_upstream(&mut openai_req);

            let mut req_builder = client
                .post(&url)
                .timeout(timeout)
                .header("user-agent", &forward_ua)
                .header("content-type", "application/json");

            if let Some(key) = effective_key {
                req_builder = req_builder.header("authorization", format!("Bearer {}", key));
            }

            let res = req_builder.json(&openai_req).send().await.map_err(|e| {
                (
                    StatusCode::BAD_GATEWAY,
                    format!("OpenAI upstream connection failed: {}", e),
                )
            })?;

            if !res.status().is_success() {
                let status =
                    StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                let text = res.text().await.unwrap_or_default();
                return Err((status, text));
            }

            if is_stream {
                let req_id_str = req_id.to_string();
                let model_str = req.model.clone();
                let byte_stream = res.bytes_stream();
                let db_clone = state.db.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut sse_state = SseState::new();
                    let mut byte_buffer: Vec<u8> = Vec::new();
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            byte_buffer.extend_from_slice(&bytes);

                            while let Some(pos) = byte_buffer.iter().position(|&b| b == b'\n') {
                                let line_bytes = &byte_buffer[..pos];
                                let line = String::from_utf8_lossy(line_bytes).trim().to_string();
                                byte_buffer.drain(..=pos);

                                if !line.is_empty() {
                                    if line.starts_with("data:") && !line.contains("[DONE]") {
                                        let payload = line.trim_start_matches("data:").trim();
                                        if let Ok(val) = serde_json::from_str::<Value>(payload) {
                                            if let Some(usage) = val.get("usage") {
                                                if let Some(pt) = usage.get("prompt_tokens").and_then(Value::as_u64) {
                                                    if pt > 0 { final_prompt_tokens = pt as u32; }
                                                }
                                                if let Some(ct) = usage.get("completion_tokens").and_then(Value::as_u64) {
                                                    if ct > 0 { final_completion_tokens = ct as u32; }
                                                }
                                            }
                                        }
                                    }

                                    let events = parse_openai_chunk_to_anthropic_events(
                                        &line,
                                        &mut sse_state,
                                        &req_id_str,
                                        &model_str,
                                    );
                                    for ev in events {
                                        yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                                    }
                                }
                            }
                        }
                    }

                    // Flush any trailing line in byte_buffer that didn't end with '\n'
                    if !byte_buffer.is_empty() {
                        let line = String::from_utf8_lossy(&byte_buffer).trim().to_string();
                        byte_buffer.clear();
                        if !line.is_empty() {
                            let events = parse_openai_chunk_to_anthropic_events(
                                &line,
                                &mut sse_state,
                                &req_id_str,
                                &model_str,
                            );
                            for ev in events {
                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                            }
                        }
                    }

                    // Safeguard: Ensure stream is cleanly closed with content_block_stop & message_stop
                    if !sse_state.stream_finished {
                        let final_events = finish_openai_stream(&mut sse_state, "end_turn");
                        for ev in final_events {
                            yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                        }
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = sse_state.total_output_tokens.max(1);
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    let _ = db_clone.update_request_log_tokens(
                        &req_id_str,
                        final_prompt_tokens as i32,
                        final_completion_tokens as i32,
                        stream_latency,
                    );
                    if final_prompt_tokens > estimated_prompt {
                        stats_clone.total_prompt_tokens.fetch_add((final_prompt_tokens - estimated_prompt) as u64, Ordering::Relaxed);
                    }
                    stats_clone.total_completion_tokens.fetch_add(final_completion_tokens as u64, Ordering::Relaxed);
                };

                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(sse_stream))
                        .unwrap(),
                    prompt_tokens: estimated_prompt,
                    completion_tokens: 0,
                })
            } else {
                let openai_resp: OpenAIChatResponse = res.json().await.map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to parse OpenAI response: {}", e),
                    )
                })?;
                let usage = openai_resp.usage.clone();
                let prompt_tokens = usage
                    .as_ref()
                    .map(|u| u.prompt_tokens)
                    .unwrap_or(estimated_prompt);
                let completion_tokens = usage
                    .as_ref()
                    .map(|u| u.completion_tokens)
                    .unwrap_or(0);
                let anthropic_resp =
                    anthropic_to_openai::convert_openai_resp_to_anthropic(&openai_resp, &req.model);
                Ok(DispatchResult {
                    response: Json(anthropic_resp).into_response(),
                    prompt_tokens,
                    completion_tokens,
                })
            }
        }
        "chatgpt" => {
            let mut openai_req =
                anthropic_to_openai::convert_anthropic_req_to_openai(req, &cand.target_model);
            normalize_openai_req_for_upstream(&mut openai_req);
            let dispatch_res =
                dispatch_chatgpt_request(state, &openai_req, cand, req_id, is_stream).await?;
            if is_stream {
                let stream_id = format!("chatgpt_sub_{}", req_id.replace('-', ""));
                let byte_stream = dispatch_res.response.into_body().into_data_stream();
                let model_str = req.model.clone();
                let anthropic_stream = stream! {
                    let mut sse_state = crate::gateway::adapters::SseState::new();
                    let mut buffer = String::new();
                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            if let Ok(s) = std::str::from_utf8(&bytes) {
                                buffer.push_str(s);
                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();
                                    if line.starts_with("data:") {
                                        let events = crate::gateway::adapters::parse_openai_chunk_to_anthropic_events(
                                            &line,
                                            &mut sse_state,
                                            &stream_id,
                                            &model_str,
                                        );
                                        for ev in events {
                                            yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                                        }
                                    }
                                }
                            }
                        }
                    }
                };
                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(anthropic_stream))
                        .unwrap(),
                    prompt_tokens: dispatch_res.prompt_tokens,
                    completion_tokens: dispatch_res.completion_tokens,
                })
            } else {
                let body_bytes = axum::body::to_bytes(dispatch_res.response.into_body(), 10 * 1024 * 1024)
                    .await
                    .unwrap_or_default();
                let chat_resp: OpenAIChatResponse = serde_json::from_slice(&body_bytes).map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to parse ChatGPT response: {}", e),
                    )
                })?;
                let anthropic_resp =
                    anthropic_to_openai::convert_openai_resp_to_anthropic(&chat_resp, &req.model);
                Ok(DispatchResult {
                    response: Json(anthropic_resp).into_response(),
                    prompt_tokens: dispatch_res.prompt_tokens,
                    completion_tokens: dispatch_res.completion_tokens,
                })
            }
        }
        "gemini" => {
            let base = cand.provider.base_url.trim_end_matches('/');
            let gemini_req = anthropic_to_gemini::convert_anthropic_req_to_gemini(req);
            let is_subscription_pool = cand.provider.auth_type.as_deref() == Some("subscription_pool");

            let req_body_val = if is_subscription_pool {
                json!({
                    "project": "aicode-consumers",
                    "model": cand.target_model,
                    "request": serde_json::to_value(&gemini_req).unwrap_or(json!({}))
                })
            } else {
                serde_json::to_value(&gemini_req).unwrap_or(json!({}))
            };

            let max_account_attempts = if is_subscription_pool { 2 } else { 1 };
            let mut last_upstream_err = None;
            let mut res_opt = None;

            for attempt_idx in 0..max_account_attempts {
                let (sub_account_id, endpoint, auth_header) = if is_subscription_pool {
                    let account = match crate::oauth::acquire_subscription_account(
                        &state.db,
                        &state.http_client,
                        &cand.provider.id,
                    )
                    .await {
                        Ok(acc) => acc,
                        Err(e) => {
                            if attempt_idx > 0 && last_upstream_err.is_some() {
                                break;
                            }
                            return Err(e);
                        }
                    };
                    let ep = if is_stream {
                        format!("{}/v1internal:streamGenerateContent?alt=sse", base)
                    } else {
                        format!("{}/v1internal:generateContent", base)
                    };
                    (
                        Some(account.id),
                        ep,
                        Some(format!("Bearer {}", account.access_token)),
                    )
                } else {
                    let api_key = cand.api_key.as_deref().unwrap_or("");
                    let ep = if is_stream {
                        format!(
                            "{}/models/{}:streamGenerateContent?alt=sse&key={}",
                            base, cand.target_model, api_key
                        )
                    } else {
                        format!(
                            "{}/models/{}:generateContent?key={}",
                            base, cand.target_model, api_key
                        )
                    };
                    (None, ep, None)
                };

                let mut req_builder = client
                    .post(&endpoint)
                    .timeout(timeout)
                    .header("user-agent", &forward_ua)
                    .header("content-type", "application/json");

                if let Some(auth) = auth_header {
                    req_builder = req_builder
                        .header("authorization", auth)
                        .header(
                            "user-agent",
                            "antigravity/cli/1.4.6 (aidev_client; os_type=darwin; arch=arm64; cl=964361259; auth_method=consumer)",
                        );
                }

                let send_res = req_builder.json(&req_body_val).send().await;
                let res = match send_res {
                    Ok(r) => r,
                    Err(e) => {
                        let is_timeout = e.is_timeout();
                        let err_msg = format!("Gemini connection error: {}", e);
                        if is_subscription_pool && attempt_idx + 1 < max_account_attempts && !is_timeout {
                            if let Some(ref acc_id) = sub_account_id {
                                crate::oauth::mark_account_cooldown(&state.db, acc_id, 30);
                            }
                            tracing::warn!("Subscription account connection error ({}), trying next account in pool...", e);
                            last_upstream_err = Some((StatusCode::BAD_GATEWAY, err_msg));
                            continue;
                        }
                        return Err((StatusCode::BAD_GATEWAY, err_msg));
                    }
                };

                if !res.status().is_success() {
                    let status_code = res.status();
                    let text = res.text().await.unwrap_or_default();
                    let is_capacity_or_rate = status_code == StatusCode::TOO_MANY_REQUESTS
                        || status_code == StatusCode::SERVICE_UNAVAILABLE
                        || text.contains("MODEL_CAPACITY_EXHAUSTED");

                    if is_subscription_pool {
                        if let Some(ref acc_id) = sub_account_id {
                            let cooldown_secs = if status_code == StatusCode::TOO_MANY_REQUESTS {
                                300
                            } else if status_code == StatusCode::SERVICE_UNAVAILABLE || text.contains("MODEL_CAPACITY_EXHAUSTED") {
                                60
                            } else {
                                30
                            };
                            crate::oauth::mark_account_cooldown(&state.db, acc_id, cooldown_secs);
                        }

                        if attempt_idx + 1 < max_account_attempts && is_capacity_or_rate {
                            tracing::warn!(
                                "Subscription account returned status {} ({}), failing over to next account in pool...",
                                status_code,
                                if text.contains("MODEL_CAPACITY_EXHAUSTED") { "MODEL_CAPACITY_EXHAUSTED" } else { "HTTP error" }
                            );
                            let status = StatusCode::from_u16(status_code.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                            last_upstream_err = Some((status, text));
                            continue;
                        }
                    }

                    let status =
                        StatusCode::from_u16(status_code.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                    return Err((status, text));
                }

                res_opt = Some(res);
                break;
            }

            let res = match res_opt {
                Some(r) => r,
                None => match last_upstream_err {
                    Some(err) => return Err(err),
                    None => return Err((StatusCode::BAD_GATEWAY, "No response from Gemini upstream".to_string())),
                },
            };

            if is_stream {
                let req_id_str = req_id.to_string();
                let model_str = req.model.clone();
                let byte_stream = res.bytes_stream();
                let db_clone = state.db.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut sse_state = SseState::new();
                    let mut buffer = String::new();
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            if let Ok(s) = std::str::from_utf8(&bytes) {
                                buffer.push_str(s);
                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();
                                    if !line.is_empty() {
                                        if line.starts_with("data:") {
                                            let payload = line.trim_start_matches("data:").trim();
                                            if let Ok(val) = serde_json::from_str::<Value>(payload) {
                                                let actual_val = val.get("response").unwrap_or(&val);
                                                if let Some(usage) = actual_val.get("usageMetadata") {
                                                    if let Some(pt) = usage.get("promptTokenCount").and_then(Value::as_u64) {
                                                        if pt > 0 { final_prompt_tokens = pt as u32; }
                                                    }
                                                    if let Some(ct) = usage.get("candidatesTokenCount").and_then(Value::as_u64) {
                                                        if ct > 0 { final_completion_tokens = ct as u32; }
                                                    }
                                                }
                                            }
                                        }

                                        let events = parse_gemini_chunk_to_anthropic_events(
                                            &line,
                                            &mut sse_state,
                                            &req_id_str,
                                            &model_str,
                                        );
                                        for event in events {
                                            yield Ok::<_, std::io::Error>(axum::body::Bytes::from(event));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if !buffer.trim().is_empty() {
                        let events = parse_gemini_chunk_to_anthropic_events(
                            buffer.trim(),
                            &mut sse_state,
                            &req_id_str,
                            &model_str,
                        );
                        for event in events {
                            yield Ok::<_, std::io::Error>(axum::body::Bytes::from(event));
                        }
                    }

                    let final_stop_reason = if sse_state.current_tool_id.is_some() {
                        "tool_use"
                    } else {
                        "end_turn"
                    };
                    for event in finish_gemini_stream(&mut sse_state, final_stop_reason) {
                        yield Ok::<_, std::io::Error>(axum::body::Bytes::from(event));
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = sse_state.total_output_tokens.max(1);
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    let _ = db_clone.update_request_log_tokens(
                        &req_id_str,
                        final_prompt_tokens as i32,
                        final_completion_tokens as i32,
                        stream_latency,
                    );
                    if final_prompt_tokens > estimated_prompt {
                        stats_clone.total_prompt_tokens.fetch_add((final_prompt_tokens - estimated_prompt) as u64, Ordering::Relaxed);
                    }
                    stats_clone.total_completion_tokens.fetch_add(final_completion_tokens as u64, Ordering::Relaxed);
                };

                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(sse_stream))
                        .unwrap(),
                    prompt_tokens: estimated_prompt,
                    completion_tokens: 0,
                })
            } else {
                let raw_val: Value = res.json().await.map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to parse Gemini response: {}", e),
                    )
                })?;
                let resp_val = raw_val.get("response").unwrap_or(&raw_val);
                let gemini_resp: GeminiResponse = serde_json::from_value(resp_val.clone()).map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to parse Gemini response: {}", e),
                    )
                })?;
                let usage = gemini_resp.usage_metadata.clone();
                let prompt_tokens = usage
                    .as_ref()
                    .and_then(|u| u.prompt_token_count)
                    .unwrap_or(estimated_prompt);
                let completion_tokens = usage
                    .as_ref()
                    .and_then(|u| u.candidates_token_count)
                    .unwrap_or(0);
                let anthropic_resp =
                    anthropic_to_gemini::convert_gemini_resp_to_anthropic(&gemini_resp, &req.model);
                Ok(DispatchResult {
                    response: Json(anthropic_resp).into_response(),
                    prompt_tokens,
                    completion_tokens,
                })
            }
        }
        _ => Err((
            StatusCode::BAD_GATEWAY,
            format!("Unsupported protocol: {}", cand.provider.protocol),
        )),
    }
}

/// Handler for OpenAI-compatible clients calling `/v1/chat/completions`
async fn handle_chat_completions(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(req): Json<OpenAIChatRequest>,
) -> Response {
    let start_time = Instant::now();
    let req_id = uuid::Uuid::new_v4().to_string();
    state.stats.active_requests.fetch_add(1, Ordering::Relaxed);
    state.stats.total_requests.fetch_add(1, Ordering::Relaxed);

    let client_type = detect_client_type(&headers, "codex_cli");
    let session_id = extract_session_id(&headers);
    let client_key = extract_client_api_key(&headers);
    let client_ua = headers.get("user-agent").and_then(|v| v.to_str().ok());
    let providers = state.db.get_providers().unwrap_or_default();

    let is_stream = req.stream.unwrap_or(false);
    let route_match =
        match state
            .router
            .resolve_route(&req.model, session_id.as_deref(), &providers)
        {
            Ok(rm) => rm,
            Err(err) => {
                state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
                state.stats.error_count.fetch_add(1, Ordering::Relaxed);
                log_request(
                    &state,
                    RequestLog {
                        id: req_id.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        method: "POST".to_string(),
                        path: "/v1/chat/completions".to_string(),
                        client_type: client_type.clone(),
                        incoming_model: req.model.clone(),
                        routed_provider_id: "none".to_string(),
                        routed_provider_name: None,
                        routed_model: String::new(),
                        status_code: StatusCode::BAD_GATEWAY.as_u16() as i32,
                        latency_ms: start_time.elapsed().as_millis() as i64,
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        error_message: Some(err.clone()),
                        is_stream,
                        fallback_used: false,
                        attempts: Vec::new(),
                    },
                );
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "error": { "message": err, "type": "routing_error" } })),
                )
                    .into_response();
            }
        };

    let mut candidates = vec![route_match.primary];
    candidates.extend(route_match.fallbacks);

    let mut attempts: Vec<LogAttempt> = Vec::new();
    let mut last_error = String::new();
    let mut last_status = StatusCode::BAD_GATEWAY;
    let mut fallback_used = false;

    for (cand_idx, cand) in candidates.iter().enumerate() {
        if cand_idx > 0 {
            fallback_used = true;
            tracing::warn!(
                "Failing over chat/completions to fallback candidate {} ({})",
                cand.provider.name,
                cand.target_model
            );
        }

        let attempt_started = Instant::now();
        let resp_result = dispatch_openai_request(
            &state,
            &req,
            cand,
            &req_id,
            is_stream,
            client_key.as_deref(),
            client_ua,
            &client_type,
        )
        .await;

        match resp_result {
            Ok(result) => {
                let latency_ms = start_time.elapsed().as_millis() as i64;
                attempts.push(attempt(
                    cand,
                    StatusCode::OK,
                    None,
                    attempt_started.elapsed().as_millis() as i64,
                ));
                state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
                state
                    .stats
                    .total_prompt_tokens
                    .fetch_add(result.prompt_tokens as u64, Ordering::Relaxed);
                state
                    .stats
                    .total_completion_tokens
                    .fetch_add(result.completion_tokens as u64, Ordering::Relaxed);

                log_request(
                    &state,
                    RequestLog {
                        id: req_id.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        method: "POST".to_string(),
                        path: "/v1/chat/completions".to_string(),
                        client_type: client_type.clone(),
                        incoming_model: req.model.clone(),
                        routed_provider_id: cand.provider.id.clone(),
                        routed_provider_name: Some(cand.provider.name.clone()),
                        routed_model: cand.target_model.clone(),
                        status_code: 200,
                        latency_ms,
                        prompt_tokens: result.prompt_tokens as i32,
                        completion_tokens: result.completion_tokens as i32,
                        error_message: None,
                        is_stream,
                        fallback_used,
                        attempts,
                    },
                );

                return result.response;
            }
            Err((status, err_msg)) => {
                attempts.push(attempt(
                    cand,
                    status,
                    Some(summarize_error(&err_msg)),
                    attempt_started.elapsed().as_millis() as i64,
                ));
                last_error = err_msg;
                last_status = status;
                if status == StatusCode::BAD_REQUEST || status == StatusCode::UNAUTHORIZED {
                    break;
                }
            }
        }
    }

    state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
    state.stats.error_count.fetch_add(1, Ordering::Relaxed);
    let latency_ms = start_time.elapsed().as_millis() as i64;

    log_request(
        &state,
        RequestLog {
            id: req_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            client_type,
            incoming_model: req.model.clone(),
            routed_provider_id: candidates.first().map(|c| c.provider.id.clone()).unwrap_or_else(|| "none".to_string()),
            routed_provider_name: candidates.first().map(|c| c.provider.name.clone()),
            routed_model: candidates.first().map(|c| c.target_model.clone()).unwrap_or_default(),
            status_code: last_status.as_u16() as i32,
            latency_ms,
            prompt_tokens: 0,
            completion_tokens: 0,
            error_message: Some(last_error.clone()),
            is_stream,
            fallback_used,
            attempts,
        },
    );

    (
        last_status,
        Json(json!({ "error": { "message": last_error, "type": "upstream_error" } })),
    )
        .into_response()
}

/// Fallback for GET or WebSocket upgrade probes on `/v1/responses`
async fn handle_responses_get() -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": {
                "message": "WebSocket streaming is not supported on Pivot responses gateway; please use HTTP POST for /v1/responses (set supports_websockets = false in ~/.codex/config.toml).",
                "type": "invalid_request_error"
            }
        })),
    )
        .into_response()
}

/// Handler for OpenAI Responses API clients (e.g. Codex CLI / Desktop) calling `/v1/responses`
async fn handle_responses(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    body_res: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let body = match body_res {
        Ok(Json(b)) => b,
        Err(e) => {
            tracing::error!("Failed to parse /v1/responses request body: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "message": format!("Failed to parse request body as JSON: {}", e),
                        "type": "invalid_request_error"
                    }
                })),
            )
                .into_response();
        }
    };
    let start_time = Instant::now();
    let req_id = uuid::Uuid::new_v4().to_string();
    state.stats.active_requests.fetch_add(1, Ordering::Relaxed);
    state.stats.total_requests.fetch_add(1, Ordering::Relaxed);

    let client_type = detect_client_type(&headers, "codex_cli");
    let session_id = extract_session_id(&headers);
    let client_key = extract_client_api_key(&headers);
    let client_ua = headers.get("user-agent").and_then(|v| v.to_str().ok());
    let providers = state.db.get_providers().unwrap_or_default();

    let is_compaction = responses::is_compaction_request(&body);
    let custom_tools = responses::extract_custom_tool_names(&body);
    let req = responses::convert_responses_req_to_chat(&body);
    let is_stream = req.stream.unwrap_or(true);

    let route_match = match state
        .router
        .resolve_route(&req.model, session_id.as_deref(), &providers)
    {
        Ok(rm) => rm,
        Err(err) => {
            state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
            state.stats.error_count.fetch_add(1, Ordering::Relaxed);
            log_request(
                &state,
                RequestLog {
                    id: req_id.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    method: "POST".to_string(),
                    path: "/v1/responses".to_string(),
                    client_type: "codex_cli".to_string(),
                    incoming_model: req.model.clone(),
                    routed_provider_id: "none".to_string(),
                    routed_provider_name: None,
                    routed_model: String::new(),
                    status_code: StatusCode::BAD_GATEWAY.as_u16() as i32,
                    latency_ms: start_time.elapsed().as_millis() as i64,
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    error_message: Some(err.clone()),
                    is_stream,
                    fallback_used: false,
                    attempts: Vec::new(),
                },
            );
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": { "message": err, "type": "routing_error" } })),
            )
                .into_response();
        }
    };

    let mut candidates = vec![route_match.primary];
    candidates.extend(route_match.fallbacks);

    let mut attempts: Vec<LogAttempt> = Vec::new();
    let mut last_error = String::new();
    let mut last_status = StatusCode::BAD_GATEWAY;
    let mut fallback_used = false;

    for (cand_idx, cand) in candidates.iter().enumerate() {
        if cand_idx > 0 {
            fallback_used = true;
            tracing::warn!(
                "Failing over responses to fallback candidate {} ({})",
                cand.provider.name,
                cand.target_model
            );
        }

        let attempt_started = Instant::now();
        match dispatch_openai_request(
            &state,
            &req,
            cand,
            &req_id,
            is_stream,
            client_key.as_deref(),
            client_ua,
            &client_type,
        )
        .await
        {
            Ok(result) => {
                attempts.push(attempt(
                    cand,
                    StatusCode::OK,
                    None,
                    attempt_started.elapsed().as_millis() as i64,
                ));
                let latency_ms = start_time.elapsed().as_millis() as i64;
                state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
                state
                    .stats
                    .total_prompt_tokens
                    .fetch_add(result.prompt_tokens as u64, Ordering::Relaxed);
                state
                    .stats
                    .total_completion_tokens
                    .fetch_add(result.completion_tokens as u64, Ordering::Relaxed);

                log_request(
                    &state,
                    RequestLog {
                        id: req_id.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        method: "POST".to_string(),
                        path: "/v1/responses".to_string(),
                        client_type: client_type.clone(),
                        incoming_model: req.model.clone(),
                        routed_provider_id: cand.provider.id.clone(),
                        routed_provider_name: Some(cand.provider.name.clone()),
                        routed_model: cand.target_model.clone(),
                        status_code: 200,
                        latency_ms,
                        prompt_tokens: result.prompt_tokens as i32,
                        completion_tokens: result.completion_tokens as i32,
                        error_message: None,
                        is_stream,
                        fallback_used,
                        attempts,
                    },
                );

                if is_stream {
                    let response_id = format!("resp_{}", req_id.replace('-', ""));
                    let model_str = req.model.clone();
                    let byte_stream = result.response.into_body().into_data_stream();
                    let custom_tools_clone = custom_tools.clone();

                    let responses_stream = stream! {
                        let mut sse_state = responses::ResponsesSseState::new_full(&response_id, &model_str, is_compaction, custom_tools_clone);
                        let mut buffer = String::new();

                        for await chunk_res in byte_stream {
                            if let Ok(bytes) = chunk_res {
                                if let Ok(s) = std::str::from_utf8(&bytes) {
                                    buffer.push_str(s);

                                    while let Some(pos) = buffer.find('\n') {
                                        let line = buffer[..pos].trim().to_string();
                                        buffer = buffer[pos + 1..].to_string();

                                        if !line.is_empty() {
                                            let events = sse_state.process_line(&line);
                                            for ev in events {
                                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let trailing = sse_state.finish();
                        for ev in trailing {
                            yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                        }
                    };

                    return Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(responses_stream))
                        .unwrap();
                } else {
                    let body_bytes = axum::body::to_bytes(result.response.into_body(), 10 * 1024 * 1024)
                        .await
                        .unwrap_or_default();
                    let chat_json: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);
                    let resp_id = format!("resp_{}", req_id.replace('-', ""));
                    let responses_val = responses::chat_response_to_responses_full(&chat_json, &resp_id, &req.model, is_compaction, &custom_tools);
                    return Json(responses_val).into_response();
                }
            }
            Err((status, err_msg)) => {
                attempts.push(attempt(
                    cand,
                    status,
                    Some(summarize_error(&err_msg)),
                    attempt_started.elapsed().as_millis() as i64,
                ));
                last_error = err_msg;
                last_status = status;
                if status == StatusCode::BAD_REQUEST || status == StatusCode::UNAUTHORIZED {
                    break;
                }
            }
        }
    }

    state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
    state.stats.error_count.fetch_add(1, Ordering::Relaxed);
    log_request(
        &state,
        RequestLog {
            id: req_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            method: "POST".to_string(),
            path: "/v1/responses".to_string(),
            client_type,
            incoming_model: req.model,
            routed_provider_id: candidates.first().map(|c| c.provider.id.clone()).unwrap_or_else(|| "none".to_string()),
            routed_provider_name: candidates.first().map(|c| c.provider.name.clone()),
            routed_model: candidates.first().map(|c| c.target_model.clone()).unwrap_or_default(),
            status_code: last_status.as_u16() as i32,
            latency_ms: start_time.elapsed().as_millis() as i64,
            prompt_tokens: 0,
            completion_tokens: 0,
            error_message: Some(last_error.clone()),
            is_stream,
            fallback_used,
            attempts,
        },
    );

    let friendly_message = if last_error.contains("MODEL_CAPACITY_EXHAUSTED") || last_error.contains("OVERLOADED_TOO_MANY_RETRIES_PER_REQUEST") {
        "Google 上游服务容量当前耗尽 (MODEL_CAPACITY_EXHAUSTED)。当前会话上下文过大容易触发排队超限，建议稍后重试或执行 /compact 压缩会话。".to_string()
    } else if last_error.contains("timed out") || (last_status == StatusCode::BAD_GATEWAY && last_error.contains("upstream connection failed")) {
        format!("请求上游超时或连接失败。当前会话提示词体量较大，建议使用 /compact 压缩上下文后重试 (详细: {})", summarize_error(&last_error))
    } else {
        summarize_error(&last_error)
    };

    (
        last_status,
        Json(json!({ "error": { "message": friendly_message, "type": "upstream_error" } })),
    )
        .into_response()
}

async fn dispatch_chatgpt_request(
    state: &GatewayState,
    req: &OpenAIChatRequest,
    cand: &RouteCandidate,
    req_id: &str,
    is_stream: bool,
) -> Result<DispatchResult, (StatusCode, String)> {
    let client = &state.http_client;
    let estimated_prompt = estimate_openai_req_prompt_tokens(req);
    let timeout = resolve_dispatch_timeout(cand, estimated_prompt);

    let account = crate::oauth::acquire_subscription_account(
        &state.db,
        &state.http_client,
        &cand.provider.id,
    )
    .await?;

    let upstream_url = "https://chatgpt.com/backend-api/codex/responses";

    let mut responses_body = responses::convert_chat_req_to_responses(req, &cand.target_model);
    responses_body["store"] = json!(false);
    responses_body["stream"] = json!(true);

    let req_builder = client
        .post(upstream_url)
        .timeout(timeout)
        .header("authorization", format!("Bearer {}", account.access_token))
        .header("user-agent", "codex-cli/0.154.0")
        .header("content-type", "application/json");

    let res = req_builder.json(&responses_body).send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Upstream ChatGPT connection failed: {}", e),
        )
    })?;

    if !res.status().is_success() {
        let status =
            StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let text = res.text().await.unwrap_or_default();
        if status == StatusCode::TOO_MANY_REQUESTS {
            crate::oauth::mark_account_cooldown(&state.db, &account.id, 300);
        }
        return Err((status, text));
    }

    if is_stream {
        let req_id_str = req_id.to_string();
        let byte_stream = res.bytes_stream();
        let model_str = cand.target_model.clone();

        let sse_stream = stream! {
            let mut parser = responses::ResponsesToChatSseParser::new(&req_id_str, &model_str);
            let mut buffer = String::new();

            for await chunk_res in byte_stream {
                if let Ok(bytes) = chunk_res {
                    if let Ok(s) = std::str::from_utf8(&bytes) {
                        buffer.push_str(s);
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();
                            let chat_lines = parser.process_line(&line);
                            for cl in chat_lines {
                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(cl));
                            }
                        }
                    }
                }
            }

            let finish_lines = parser.finish();
            for fl in finish_lines {
                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(fl));
            }
        };

        Ok(DispatchResult {
            response: Response::builder()
                .header("content-type", "text/event-stream")
                .header("cache-control", "no-cache")
                .header("connection", "keep-alive")
                .body(Body::from_stream(sse_stream))
                .unwrap(),
            prompt_tokens: estimated_prompt as u32,
            completion_tokens: 0,
        })
    } else {
        let byte_stream = res.bytes_stream();
        let req_id_str = req_id.to_string();
        let model_str = cand.target_model.clone();

        let mut parser = responses::ResponsesToChatSseParser::new(&req_id_str, &model_str);
        let mut buffer = String::new();
        let mut full_content = String::new();
        let mut prompt_tokens = estimated_prompt as u32;
        let mut completion_tokens = 0u32;

        use tokio_stream::StreamExt;
        tokio::pin!(byte_stream);
        while let Some(chunk_res) = byte_stream.next().await {
            if let Ok(bytes) = chunk_res {
                if let Ok(s) = std::str::from_utf8(&bytes) {
                    buffer.push_str(s);
                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].trim().to_string();
                        buffer = buffer[pos + 1..].to_string();
                        let chat_lines = parser.process_line(&line);
                        for cl in chat_lines {
                            let trimmed = cl.trim();
                            if trimmed.starts_with("data:") && !trimmed.contains("[DONE]") {
                                if let Ok(val) = serde_json::from_str::<Value>(trimmed.trim_start_matches("data:").trim()) {
                                    if let Some(c) = val["choices"][0]["delta"]["content"].as_str() {
                                        full_content.push_str(c);
                                    }
                                    if let Some(u) = val.get("usage") {
                                        prompt_tokens = u["prompt_tokens"].as_u64().unwrap_or(prompt_tokens as u64) as u32;
                                        completion_tokens = u["completion_tokens"].as_u64().unwrap_or(completion_tokens as u64) as u32;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let resp_json = json!({
            "id": format!("chatcmpl_{}", req_id_str.replace('-', "")),
            "object": "chat.completion",
            "created": chrono::Utc::now().timestamp(),
            "model": model_str,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": full_content
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": prompt_tokens + completion_tokens
            }
        });

        Ok(DispatchResult {
            response: Json(resp_json).into_response(),
            prompt_tokens,
            completion_tokens,
        })
    }
}

async fn dispatch_openai_request(
    state: &GatewayState,
    req: &OpenAIChatRequest,
    cand: &RouteCandidate,
    req_id: &str,
    is_stream: bool,
    client_key: Option<&str>,
    client_ua: Option<&str>,
    client_type: &str,
) -> Result<DispatchResult, (StatusCode, String)> {
    let client = &state.http_client;
    let estimated_prompt = estimate_openai_req_prompt_tokens(req);
    let timeout = resolve_dispatch_timeout(cand, estimated_prompt);
    let effective_key = cand.api_key.as_deref().or(client_key);
    let forward_ua = resolve_forward_user_agent(client_ua, client_type);

    match cand.provider.protocol.as_str() {
        "chatgpt" => {
            dispatch_chatgpt_request(state, req, cand, req_id, is_stream).await
        }
        "openai" | "openrouter" | "ollama" => {
            let url = resolve_upstream_endpoint(&cand.provider.base_url, "/chat/completions");

            let mut req_body = req.clone();
            req_body.model = cand.target_model.clone();
            normalize_openai_req_for_upstream(&mut req_body);

            let mut req_builder = client
                .post(&url)
                .timeout(timeout)
                .header("user-agent", &forward_ua)
                .header("content-type", "application/json");

            if let Some(key) = effective_key {
                req_builder = req_builder.header("authorization", format!("Bearer {}", key));
            }

            let res = req_builder.json(&req_body).send().await.map_err(|e| {
                (
                    StatusCode::BAD_GATEWAY,
                    format!("Upstream connection failed: {}", e),
                )
            })?;

            if !res.status().is_success() {
                let status =
                    StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                let text = res.text().await.unwrap_or_default();
                return Err((status, text));
            }

            if is_stream {
                let req_id_str = req_id.to_string();
                let byte_stream = res.bytes_stream();
                let db_clone = state.db.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut buffer = String::new();
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;
                    let mut generated_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            if let Ok(s) = std::str::from_utf8(&bytes) {
                                buffer.push_str(s);

                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();

                                    if line.starts_with("data:") && !line.contains("[DONE]") {
                                        let payload = line.trim_start_matches("data:").trim();
                                        if let Ok(val) = serde_json::from_str::<Value>(payload) {
                                            if let Some(usage) = val.get("usage") {
                                                if let Some(pt) = usage.get("prompt_tokens").and_then(Value::as_u64) {
                                                    if pt > 0 { final_prompt_tokens = pt as u32; }
                                                }
                                                if let Some(ct) = usage.get("completion_tokens").and_then(Value::as_u64) {
                                                    if ct > 0 { final_completion_tokens = ct as u32; }
                                                }
                                            }
                                            if let Some(choices) = val.get("choices").and_then(Value::as_array) {
                                                if let Some(c0) = choices.first() {
                                                    if let Some(d) = c0.get("delta") {
                                                        if d.get("content").is_some() || d.get("reasoning_content").is_some() {
                                                            generated_tokens += 1;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            yield Ok::<_, std::io::Error>(bytes);
                        }
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = generated_tokens.max(1);
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    let _ = db_clone.update_request_log_tokens(
                        &req_id_str,
                        final_prompt_tokens as i32,
                        final_completion_tokens as i32,
                        stream_latency,
                    );
                    if final_prompt_tokens > estimated_prompt {
                        stats_clone.total_prompt_tokens.fetch_add((final_prompt_tokens - estimated_prompt) as u64, Ordering::Relaxed);
                    }
                    stats_clone.total_completion_tokens.fetch_add(final_completion_tokens as u64, Ordering::Relaxed);
                };

                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(sse_stream))
                        .unwrap(),
                    prompt_tokens: estimated_prompt,
                    completion_tokens: 0,
                })
            } else {
                let bytes = res
                    .bytes()
                    .await
                    .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
                let usage = serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .and_then(|body| body.get("usage").cloned());
                let prompt_tokens = usage
                    .as_ref()
                    .and_then(|u| u.get("prompt_tokens"))
                    .and_then(Value::as_u64)
                    .map(|v| v as u32)
                    .unwrap_or(estimated_prompt);
                let completion_tokens = usage
                    .as_ref()
                    .and_then(|u| u.get("completion_tokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32;

                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "application/json")
                        .body(Body::from(bytes))
                        .unwrap(),
                    prompt_tokens,
                    completion_tokens,
                })
            }
        }
        "anthropic" => {
            // Client called OpenAI endpoint, but upstream is Anthropic!
            let anthropic_req =
                openai_to_anthropic::convert_openai_req_to_anthropic(req, &cand.target_model);
            let url = resolve_upstream_endpoint(&cand.provider.base_url, "/messages");

            let mut req_builder = client
                .post(&url)
                .timeout(timeout)
                .header("user-agent", &forward_ua)
                .header("content-type", "application/json")
                .header("anthropic-version", "2023-06-01");

            if let Some(key) = effective_key {
                req_builder = req_builder
                    .header("x-api-key", key)
                    .header("authorization", format!("Bearer {}", key));
            }

            let res = req_builder.json(&anthropic_req).send().await.map_err(|e| {
                (
                    StatusCode::BAD_GATEWAY,
                    format!("Anthropic upstream connection failed: {}", e),
                )
            })?;

            if !res.status().is_success() {
                let status =
                    StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                let text = res.text().await.unwrap_or_default();
                return Err((status, text));
            }

            if is_stream {
                let req_id_str = req_id.to_string();
                let model_str = req.model.clone();
                let byte_stream = res.bytes_stream();
                let db_clone = state.db.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut buffer = String::new();
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;
                    let mut generated_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            if let Ok(s) = std::str::from_utf8(&bytes) {
                                buffer.push_str(s);

                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();

                                    if !line.is_empty() {
                                        if line.starts_with("data:") {
                                            let payload = line.trim_start_matches("data:").trim();
                                            if let Ok(val) = serde_json::from_str::<Value>(payload) {
                                                if let Some(msg) = val.get("message") {
                                                    if let Some(u) = msg.get("usage") {
                                                        if let Some(it) = u.get("input_tokens").and_then(Value::as_u64) {
                                                            if it > 0 { final_prompt_tokens = it as u32; }
                                                        }
                                                    }
                                                }
                                                if let Some(u) = val.get("usage") {
                                                    if let Some(ot) = u.get("output_tokens").and_then(Value::as_u64) {
                                                        if ot > 0 { final_completion_tokens = ot as u32; }
                                                    }
                                                }
                                                if let Some(delta) = val.get("delta") {
                                                    if delta.get("text").is_some() {
                                                        generated_tokens += 1;
                                                    }
                                                }
                                            }
                                        }

                                        let chunks = parse_anthropic_event_to_openai_chunks(
                                            &line,
                                            &req_id_str,
                                            &model_str,
                                        );
                                        for c in chunks {
                                            yield Ok::<_, std::io::Error>(axum::body::Bytes::from(c));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = generated_tokens.max(1);
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    let _ = db_clone.update_request_log_tokens(
                        &req_id_str,
                        final_prompt_tokens as i32,
                        final_completion_tokens as i32,
                        stream_latency,
                    );
                    if final_prompt_tokens > estimated_prompt {
                        stats_clone.total_prompt_tokens.fetch_add((final_prompt_tokens - estimated_prompt) as u64, Ordering::Relaxed);
                    }
                    stats_clone.total_completion_tokens.fetch_add(final_completion_tokens as u64, Ordering::Relaxed);
                };

                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(sse_stream))
                        .unwrap(),
                    prompt_tokens: estimated_prompt,
                    completion_tokens: 0,
                })
            } else {
                let anthropic_resp: AnthropicResponse = res.json().await.map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to parse Anthropic response: {}", e),
                    )
                })?;
                let prompt_tokens = anthropic_resp.usage.input_tokens;
                let completion_tokens = anthropic_resp.usage.output_tokens;
                let openai_resp = openai_to_anthropic::convert_anthropic_resp_to_openai(
                    &anthropic_resp,
                    &req.model,
                );
                Ok(DispatchResult {
                    response: Json(openai_resp).into_response(),
                    prompt_tokens,
                    completion_tokens,
                })
            }
        }
        "gemini" => {
            let anthropic_req =
                openai_to_anthropic::convert_openai_req_to_anthropic(req, &cand.target_model);
            let gemini_req = anthropic_to_gemini::convert_anthropic_req_to_gemini(&anthropic_req);

            let is_subscription_pool =
                cand.provider.auth_type.as_deref() == Some("subscription_pool");
            let base = cand.provider.base_url.trim_end_matches('/');

            let req_body_val = if is_subscription_pool {
                json!({
                    "project": "aicode-consumers",
                    "model": cand.target_model,
                    "request": serde_json::to_value(&gemini_req).unwrap_or(json!({}))
                })
            } else {
                serde_json::to_value(&gemini_req).unwrap_or(json!({}))
            };

            let max_account_attempts = if is_subscription_pool { 2 } else { 1 };
            let mut last_upstream_err = None;
            let mut res_opt = None;

            for attempt_idx in 0..max_account_attempts {
                let (sub_account_id, endpoint, auth_header) = if is_subscription_pool {
                    let account = match crate::oauth::acquire_subscription_account(
                        &state.db,
                        &state.http_client,
                        &cand.provider.id,
                    )
                    .await {
                        Ok(acc) => acc,
                        Err(e) => {
                            if attempt_idx > 0 && last_upstream_err.is_some() {
                                break;
                            }
                            return Err(e);
                        }
                    };
                    let ep = if is_stream {
                        format!("{}/v1internal:streamGenerateContent?alt=sse", base)
                    } else {
                        format!("{}/v1internal:generateContent", base)
                    };
                    (
                        Some(account.id),
                        ep,
                        Some(format!("Bearer {}", account.access_token)),
                    )
                } else {
                    let api_key = cand.api_key.as_deref().or(client_key).unwrap_or("");
                    let ep = if is_stream {
                        format!(
                            "{}/models/{}:streamGenerateContent?alt=sse&key={}",
                            base, cand.target_model, api_key
                        )
                    } else {
                        format!(
                            "{}/models/{}:generateContent?key={}",
                            base, cand.target_model, api_key
                        )
                    };
                    (None, ep, None)
                };

                let mut req_builder = client
                    .post(&endpoint)
                    .timeout(timeout)
                    .header("user-agent", &forward_ua)
                    .header("content-type", "application/json");

                if let Some(auth) = auth_header {
                    req_builder = req_builder
                        .header("authorization", auth)
                        .header(
                            "user-agent",
                            "antigravity/cli/1.4.6 (aidev_client; os_type=darwin; arch=arm64; cl=964361259; auth_method=consumer)",
                        );
                }

                let send_res = req_builder.json(&req_body_val).send().await;
                let res = match send_res {
                    Ok(r) => r,
                    Err(e) => {
                        let is_timeout = e.is_timeout();
                        let err_msg = format!("Gemini upstream connection failed: {}", e);
                        if is_subscription_pool && attempt_idx + 1 < max_account_attempts && !is_timeout {
                            if let Some(ref acc_id) = sub_account_id {
                                crate::oauth::mark_account_cooldown(&state.db, acc_id, 30);
                            }
                            tracing::warn!("Subscription account connection error ({}), trying next account in pool...", e);
                            last_upstream_err = Some((StatusCode::BAD_GATEWAY, err_msg));
                            continue;
                        }
                        return Err((StatusCode::BAD_GATEWAY, err_msg));
                    }
                };

                if !res.status().is_success() {
                    let status_code = res.status();
                    let text = res.text().await.unwrap_or_default();
                    let is_capacity_or_rate = status_code == StatusCode::TOO_MANY_REQUESTS
                        || status_code == StatusCode::SERVICE_UNAVAILABLE
                        || text.contains("MODEL_CAPACITY_EXHAUSTED");

                    if is_subscription_pool {
                        if let Some(ref acc_id) = sub_account_id {
                            let cooldown_secs = if status_code == StatusCode::TOO_MANY_REQUESTS {
                                300
                            } else if status_code == StatusCode::SERVICE_UNAVAILABLE || text.contains("MODEL_CAPACITY_EXHAUSTED") {
                                60
                            } else {
                                30
                            };
                            crate::oauth::mark_account_cooldown(&state.db, acc_id, cooldown_secs);
                        }

                        if attempt_idx + 1 < max_account_attempts && is_capacity_or_rate {
                            tracing::warn!(
                                "Subscription account returned status {} ({}), failing over to next account in pool...",
                                status_code,
                                if text.contains("MODEL_CAPACITY_EXHAUSTED") { "MODEL_CAPACITY_EXHAUSTED" } else { "HTTP error" }
                            );
                            let status = StatusCode::from_u16(status_code.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                            last_upstream_err = Some((status, text));
                            continue;
                        }
                    }

                    let status =
                        StatusCode::from_u16(status_code.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                    return Err((status, text));
                }

                res_opt = Some(res);
                break;
            }

            let res = match res_opt {
                Some(r) => r,
                None => match last_upstream_err {
                    Some(err) => return Err(err),
                    None => return Err((StatusCode::BAD_GATEWAY, "No response from Gemini upstream".to_string())),
                },
            };

            if is_stream {
                let req_id_str = req_id.to_string();
                let model_str = req.model.clone();
                let byte_stream = res.bytes_stream();
                let db_clone = state.db.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut buffer = String::new();
                    let mut sse_state = SseState::new();
                    sse_state.total_input_tokens = estimated_prompt;
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            if let Ok(s) = std::str::from_utf8(&bytes) {
                                buffer.push_str(s);

                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();

                                    if !line.is_empty() {
                                        if line.starts_with("data:") {
                                            let payload = line.trim_start_matches("data:").trim();
                                            if let Ok(val) = serde_json::from_str::<Value>(payload) {
                                                let actual_val = val.get("response").unwrap_or(&val);
                                                if let Some(usage) = actual_val.get("usageMetadata") {
                                                    if let Some(pt) = usage.get("promptTokenCount").and_then(Value::as_u64) {
                                                        if pt > 0 { final_prompt_tokens = pt as u32; }
                                                    }
                                                    if let Some(ct) = usage.get("candidatesTokenCount").and_then(Value::as_u64) {
                                                        if ct > 0 { final_completion_tokens = ct as u32; }
                                                    }
                                                }
                                            }
                                        }

                                        let anthropic_events = parse_gemini_chunk_to_anthropic_events(
                                            &line,
                                            &mut sse_state,
                                            &req_id_str,
                                            &model_str,
                                        );
                                        for a_evt in anthropic_events {
                                            let oai_chunks = parse_anthropic_event_to_openai_chunks(
                                                &a_evt,
                                                &req_id_str,
                                                &model_str,
                                            );
                                            for o_chunk in oai_chunks {
                                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(o_chunk));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if !buffer.trim().is_empty() {
                        let anthropic_events = parse_gemini_chunk_to_anthropic_events(
                            buffer.trim(),
                            &mut sse_state,
                            &req_id_str,
                            &model_str,
                        );
                        for a_evt in anthropic_events {
                            let oai_chunks = parse_anthropic_event_to_openai_chunks(
                                &a_evt,
                                &req_id_str,
                                &model_str,
                            );
                            for o_chunk in oai_chunks {
                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(o_chunk));
                            }
                        }
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = sse_state.total_output_tokens.max(1);
                    }
                    if final_prompt_tokens == 0 {
                        final_prompt_tokens = sse_state.total_input_tokens;
                    }

                    let final_stop_reason = if sse_state.current_tool_id.is_some() {
                        "tool_use"
                    } else {
                        "end_turn"
                    };
                    for a_evt in finish_gemini_stream(&mut sse_state, final_stop_reason) {
                        let oai_chunks = parse_anthropic_event_to_openai_chunks(
                            &a_evt,
                            &req_id_str,
                            &model_str,
                        );
                        for o_chunk in oai_chunks {
                            yield Ok::<_, std::io::Error>(axum::body::Bytes::from(o_chunk));
                        }
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    let _ = db_clone.update_request_log_tokens(
                        &req_id_str,
                        final_prompt_tokens as i32,
                        final_completion_tokens as i32,
                        stream_latency,
                    );
                    if final_prompt_tokens > estimated_prompt {
                        stats_clone.total_prompt_tokens.fetch_add((final_prompt_tokens - estimated_prompt) as u64, Ordering::Relaxed);
                    }
                    stats_clone.total_completion_tokens.fetch_add(final_completion_tokens as u64, Ordering::Relaxed);
                };

                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(sse_stream))
                        .unwrap(),
                    prompt_tokens: estimated_prompt,
                    completion_tokens: 0,
                })
            } else {
                let raw_val: Value = res.json().await.map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to parse Gemini response: {}", e),
                    )
                })?;
                let resp_val = raw_val.get("response").unwrap_or(&raw_val);
                let gemini_resp: GeminiResponse = serde_json::from_value(resp_val.clone()).map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to parse Gemini response: {}", e),
                    )
                })?;
                let usage = gemini_resp.usage_metadata.clone();
                let prompt_tokens = usage
                    .as_ref()
                    .and_then(|u| u.prompt_token_count)
                    .unwrap_or(estimated_prompt);
                let completion_tokens = usage
                    .as_ref()
                    .and_then(|u| u.candidates_token_count)
                    .unwrap_or(0);

                let anthropic_resp = anthropic_to_gemini::convert_gemini_resp_to_anthropic(
                    &gemini_resp,
                    &cand.target_model,
                );
                let openai_resp = openai_to_anthropic::convert_anthropic_resp_to_openai(
                    &anthropic_resp,
                    &req.model,
                );

                Ok(DispatchResult {
                    response: Json(openai_resp).into_response(),
                    prompt_tokens,
                    completion_tokens,
                })
            }
        }
        _ => Err((
            StatusCode::BAD_GATEWAY,
            format!(
                "Unsupported protocol for chat completions: {}",
                cand.provider.protocol
            ),
        )),
    }
}

async fn handle_gemini_generate_content(
    State(state): State<GatewayState>,
    Path(action): Path<String>,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    Json(req_body): Json<Value>,
) -> Response {
    let start_time = Instant::now();
    let req_id = uuid::Uuid::new_v4().to_string();
    state.stats.active_requests.fetch_add(1, Ordering::Relaxed);
    state.stats.total_requests.fetch_add(1, Ordering::Relaxed);

    let clean_action = action.trim_start_matches('/');
    let model_and_method = clean_action.strip_prefix("models/").unwrap_or(clean_action);
    let (model, method) = match model_and_method.split_once(':') {
        Some((m, method)) => (m, method),
        None => (model_and_method, "generateContent"),
    };
    let is_stream = method.contains("streamGenerateContent")
        || query_params.get("alt").map(|s| s.as_str()) == Some("sse");

    let client_key = headers
        .get("x-goog-api-key")
        .and_then(|v| v.to_str().ok())
        .or_else(|| query_params.get("key").map(|s| s.as_str()))
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
        });

    let client_type = detect_client_type(&headers, "gemini_cli");
    let client_ua = headers.get("user-agent").and_then(|v| v.to_str().ok());
    let session_id = extract_session_id(&headers);
    let providers = state.db.get_providers().unwrap_or_default();

    let route_match = match state.router.resolve_route(
        model,
        session_id.as_deref(),
        &providers,
    ) {
        Ok(rm) => rm,
        Err(err) => {
            state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
            state.stats.error_count.fetch_add(1, Ordering::Relaxed);
            log_request(
                &state,
                RequestLog {
                    id: req_id.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    method: "POST".to_string(),
                    path: format!("/models/{}", action),
                    client_type: client_type.clone(),
                    incoming_model: model.to_string(),
                    routed_provider_id: "none".to_string(),
                    routed_provider_name: None,
                    routed_model: String::new(),
                    status_code: StatusCode::BAD_GATEWAY.as_u16() as i32,
                    latency_ms: start_time.elapsed().as_millis() as i64,
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    error_message: Some(err.clone()),
                    is_stream,
                    fallback_used: false,
                    attempts: vec![],
                },
            );
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": {
                        "code": 502,
                        "message": err,
                        "status": "UNAVAILABLE"
                    }
                })),
            )
                .into_response();
        }
    };

    let mut candidates = vec![route_match.primary];
    candidates.extend(route_match.fallbacks);

    let mut attempts: Vec<LogAttempt> = Vec::new();
    let mut last_error = String::new();
    let mut last_status = StatusCode::BAD_GATEWAY;
    let mut fallback_used = false;

    for (cand_idx, cand) in candidates.iter().enumerate() {
        if cand_idx > 0 {
            fallback_used = true;
            tracing::warn!(
                "Failing over Gemini request to fallback candidate {} ({})",
                cand.provider.name,
                cand.target_model
            );
        }

        let attempt_started = Instant::now();
        let resp_result = dispatch_gemini_request(
            &state,
            &req_body,
            cand,
            &req_id,
            is_stream,
            method,
            client_key,
            client_ua,
            &client_type,
        )
        .await;

        match resp_result {
            Ok(result) => {
                let latency_ms = start_time.elapsed().as_millis() as i64;
                attempts.push(attempt(
                    cand,
                    StatusCode::OK,
                    None,
                    attempt_started.elapsed().as_millis() as i64,
                ));
                state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
                state
                    .stats
                    .total_prompt_tokens
                    .fetch_add(result.prompt_tokens as u64, Ordering::Relaxed);
                state
                    .stats
                    .total_completion_tokens
                    .fetch_add(result.completion_tokens as u64, Ordering::Relaxed);

                let log = RequestLog {
                    id: req_id,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    method: "POST".to_string(),
                    path: format!("/models/{}", action),
                    client_type,
                    incoming_model: model.to_string(),
                    routed_provider_id: cand.provider.id.clone(),
                    routed_provider_name: Some(cand.provider.name.clone()),
                    routed_model: cand.target_model.clone(),
                    status_code: StatusCode::OK.as_u16() as i32,
                    latency_ms,
                    prompt_tokens: result.prompt_tokens as i32,
                    completion_tokens: result.completion_tokens as i32,
                    error_message: None,
                    is_stream,
                    fallback_used,
                    attempts,
                };
                log_request(&state, log.clone());
                emit_log_event(&state, &log);
                return result.response;
            }
            Err((status, err_text)) => {
                let attempt_latency = attempt_started.elapsed().as_millis() as i64;
                attempts.push(attempt(cand, status, Some(err_text.clone()), attempt_latency));
                last_error = err_text;
                last_status = status;
                tracing::warn!(
                    "Gemini candidate {} ({}) failed with status {}: {}",
                    cand.provider.name,
                    cand.target_model,
                    status,
                    last_error
                );
            }
        }
    }

    state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
    state.stats.error_count.fetch_add(1, Ordering::Relaxed);

    let log = RequestLog {
        id: req_id,
        timestamp: chrono::Utc::now().to_rfc3339(),
        method: "POST".to_string(),
        path: format!("/models/{}", action),
        client_type,
        incoming_model: model.to_string(),
        routed_provider_id: candidates.first().map(|c| c.provider.id.clone()).unwrap_or_else(|| "none".to_string()),
        routed_provider_name: candidates.first().map(|c| c.provider.name.clone()),
        routed_model: candidates.first().map(|c| c.target_model.clone()).unwrap_or_default(),
        status_code: last_status.as_u16() as i32,
        latency_ms: start_time.elapsed().as_millis() as i64,
        prompt_tokens: 0,
        completion_tokens: 0,
        error_message: Some(last_error.clone()),
        is_stream,
        fallback_used,
        attempts,
    };
    log_request(&state, log.clone());
    emit_log_event(&state, &log);

    (
        last_status,
        Json(json!({
            "error": {
                "code": last_status.as_u16(),
                "message": last_error,
                "status": "UNAVAILABLE"
            }
        })),
    )
        .into_response()
}

async fn dispatch_gemini_request(
    state: &GatewayState,
    req_body: &Value,
    cand: &RouteCandidate,
    req_id: &str,
    is_stream: bool,
    _method: &str,
    client_key: Option<&str>,
    client_ua: Option<&str>,
    client_type: &str,
) -> Result<DispatchResult, (StatusCode, String)> {
    let client = &state.http_client;
    let timeout = resolve_dispatch_timeout(cand, 0);
    let forward_ua = resolve_forward_user_agent(client_ua, client_type);

    match cand.provider.protocol.as_str() {
        "gemini" => {
            let mut sanitized_req = req_body.clone();
            if let Some(tools) = sanitized_req.get_mut("tools").and_then(Value::as_array_mut) {
                for tool in tools {
                    let decls_opt = if tool.get("functionDeclarations").is_some() {
                        tool.get_mut("functionDeclarations").and_then(Value::as_array_mut)
                    } else {
                        tool.get_mut("function_declarations").and_then(Value::as_array_mut)
                    };
                    if let Some(decls) = decls_opt {
                        for decl in decls {
                            if let Some(params) = decl.get_mut("parameters") {
                                anthropic_to_gemini::sanitize_gemini_schema(params);
                            }
                        }
                    }
                }
            }

            let is_subscription_pool =
                cand.provider.auth_type.as_deref() == Some("subscription_pool");
            let base = cand.provider.base_url.trim_end_matches('/');

            let req_body_val = if is_subscription_pool {
                json!({
                    "project": "aicode-consumers",
                    "model": cand.target_model,
                    "request": sanitized_req
                })
            } else {
                sanitized_req
            };

            let max_account_attempts = if is_subscription_pool { 2 } else { 1 };
            let mut last_upstream_err = None;
            let mut res_opt = None;

            for attempt_idx in 0..max_account_attempts {
                let (sub_account_id, endpoint, auth_header) = if is_subscription_pool {
                    let account = match crate::oauth::acquire_subscription_account(
                        &state.db,
                        &state.http_client,
                        &cand.provider.id,
                    )
                    .await {
                        Ok(acc) => acc,
                        Err(e) => {
                            if attempt_idx > 0 && last_upstream_err.is_some() {
                                break;
                            }
                            return Err(e);
                        }
                    };
                    let ep = if is_stream {
                        format!("{}/v1internal:streamGenerateContent?alt=sse", base)
                    } else {
                        format!("{}/v1internal:generateContent", base)
                    };
                    (
                        Some(account.id),
                        ep,
                        Some(format!("Bearer {}", account.access_token)),
                    )
                } else {
                    let api_key = cand.api_key.as_deref().or(client_key).unwrap_or("");
                    let ep = if is_stream {
                        format!(
                            "{}/models/{}:streamGenerateContent?alt=sse&key={}",
                            base, cand.target_model, api_key
                        )
                    } else {
                        format!(
                            "{}/models/{}:generateContent?key={}",
                            base, cand.target_model, api_key
                        )
                    };
                    (None, ep, None)
                };

                let mut req_builder = client
                    .post(&endpoint)
                    .timeout(timeout)
                    .header("user-agent", &forward_ua)
                    .header("content-type", "application/json");

                if let Some(auth) = auth_header {
                    req_builder = req_builder
                        .header("authorization", auth)
                        .header(
                            "user-agent",
                            "antigravity/cli/1.4.6 (aidev_client; os_type=darwin; arch=arm64; cl=964361259; auth_method=consumer)",
                        );
                }

                let send_res = req_builder.json(&req_body_val).send().await;
                let res = match send_res {
                    Ok(r) => r,
                    Err(e) => {
                        let is_timeout = e.is_timeout();
                        let err_msg = format!("Gemini upstream connection failed: {}", e);
                        if is_subscription_pool && attempt_idx + 1 < max_account_attempts && !is_timeout {
                            if let Some(ref acc_id) = sub_account_id {
                                crate::oauth::mark_account_cooldown(&state.db, acc_id, 30);
                            }
                            tracing::warn!("Subscription account connection error ({}), trying next account in pool...", e);
                            last_upstream_err = Some((StatusCode::BAD_GATEWAY, err_msg));
                            continue;
                        }
                        return Err((StatusCode::BAD_GATEWAY, err_msg));
                    }
                };

                if !res.status().is_success() {
                    let status_code = res.status();
                    let text = res.text().await.unwrap_or_default();
                    let is_capacity_or_rate = status_code == StatusCode::TOO_MANY_REQUESTS
                        || status_code == StatusCode::SERVICE_UNAVAILABLE
                        || text.contains("MODEL_CAPACITY_EXHAUSTED");

                    if is_subscription_pool {
                        if let Some(ref acc_id) = sub_account_id {
                            let cooldown_secs = if status_code == StatusCode::TOO_MANY_REQUESTS {
                                300
                            } else if status_code == StatusCode::SERVICE_UNAVAILABLE || text.contains("MODEL_CAPACITY_EXHAUSTED") {
                                60
                            } else {
                                30
                            };
                            crate::oauth::mark_account_cooldown(&state.db, acc_id, cooldown_secs);
                        }

                        if attempt_idx + 1 < max_account_attempts && is_capacity_or_rate {
                            tracing::warn!(
                                "Subscription account returned status {} ({}), failing over to next account in pool...",
                                status_code,
                                if text.contains("MODEL_CAPACITY_EXHAUSTED") { "MODEL_CAPACITY_EXHAUSTED" } else { "HTTP error" }
                            );
                            let status = StatusCode::from_u16(status_code.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                            last_upstream_err = Some((status, text));
                            continue;
                        }
                    }

                    let status =
                        StatusCode::from_u16(status_code.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                    return Err((status, text));
                }

                res_opt = Some(res);
                break;
            }

            let res = match res_opt {
                Some(r) => r,
                None => match last_upstream_err {
                    Some(err) => return Err(err),
                    None => return Err((StatusCode::BAD_GATEWAY, "No response from Gemini upstream".to_string())),
                },
            };

            if is_stream {
                let req_id_str = req_id.to_string();
                let byte_stream = res.bytes_stream();
                let db_clone = state.db.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut buffer = String::new();
                    let mut final_prompt_tokens = 0u32;
                    let mut final_completion_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            if let Ok(s) = std::str::from_utf8(&bytes) {
                                buffer.push_str(s);

                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();

                                    if !line.is_empty() {
                                        if line.starts_with("data:") {
                                            let payload = line.trim_start_matches("data:").trim();
                                            if let Ok(val) = serde_json::from_str::<Value>(payload) {
                                                let actual_val = val.get("response").unwrap_or(&val);
                                                if let Some(usage) = actual_val.get("usageMetadata") {
                                                    if let Some(pt) = usage.get("promptTokenCount").and_then(Value::as_u64) {
                                                        if pt > 0 { final_prompt_tokens = pt as u32; }
                                                    }
                                                    if let Some(ct) = usage.get("candidatesTokenCount").and_then(Value::as_u64) {
                                                        if ct > 0 { final_completion_tokens = ct as u32; }
                                                    }
                                                }

                                                if val.get("response").is_some() {
                                                    let clean_line = format!("data: {}\n\n", serde_json::to_string(actual_val).unwrap_or_default());
                                                    yield Ok::<_, std::io::Error>(axum::body::Bytes::from(clean_line));
                                                    continue;
                                                }
                                            }
                                        }

                                        let out_line = format!("{}\n", line);
                                        yield Ok::<_, std::io::Error>(axum::body::Bytes::from(out_line));
                                    }
                                }
                            }
                        }
                    }

                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    let _ = db_clone.update_request_log_tokens(
                        &req_id_str,
                        final_prompt_tokens as i32,
                        final_completion_tokens as i32,
                        stream_latency,
                    );
                    stats_clone.total_prompt_tokens.fetch_add(final_prompt_tokens as u64, Ordering::Relaxed);
                    stats_clone.total_completion_tokens.fetch_add(final_completion_tokens as u64, Ordering::Relaxed);
                };

                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(sse_stream))
                        .unwrap(),
                    prompt_tokens: 0,
                    completion_tokens: 0,
                })
            } else {
                let raw_val: Value = res.json().await.map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to parse Gemini response: {}", e),
                    )
                })?;
                let resp_val = raw_val.get("response").unwrap_or(&raw_val).clone();
                let usage = resp_val.get("usageMetadata");
                let prompt_tokens = usage
                    .and_then(|u| u.get("promptTokenCount"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32;
                let completion_tokens = usage
                    .and_then(|u| u.get("candidatesTokenCount"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32;

                Ok(DispatchResult {
                    response: Json(resp_val).into_response(),
                    prompt_tokens,
                    completion_tokens,
                })
            }
        }
        _ => Err((
            StatusCode::BAD_GATEWAY,
            format!(
                "Gemini endpoint currently only supports Gemini providers (got protocol {})",
                cand.provider.protocol
            ),
        )),
    }
}

fn emit_log_event(state: &GatewayState, log: &RequestLog) {
    if let Some(ref handle) = state.app_handle {
        let _ = handle.emit("gw-log", log);
    }
}

fn log_request(state: &GatewayState, log: RequestLog) {
    let _ = state.db.insert_log(&log);
    emit_log_event(state, &log);
}

fn attempt(candidate: &RouteCandidate, status: StatusCode, error: Option<String>, latency_ms: i64) -> LogAttempt {
    LogAttempt {
        provider_id: candidate.provider.id.clone(),
        provider_name: candidate.provider.name.clone(),
        model: candidate.target_model.clone(),
        status: status.as_u16() as i32,
        error,
        latency_ms,
    }
}

/// Upstream error bodies are often verbose envelopes (or whole HTML pages when a
/// proxy challenges the request). Keep the readable reason and cap the length so
/// the attempt chain stays scannable in the log table.
fn summarize_error(body: &str) -> String {
    const LIMIT: usize = 240;

    let text = serde_json::from_str::<Value>(body)
        .ok()
        .map(|value| {
            value
                .get("error")
                .and_then(|error| {
                    error
                        .get("message")
                        .and_then(Value::as_str)
                        .or_else(|| error.as_str())
                })
                .or_else(|| value.get("message").and_then(Value::as_str))
                .unwrap_or(body)
                .to_string()
        })
        .unwrap_or_else(|| body.to_string());

    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return "Upstream returned an error without a body".to_string();
    }
    if collapsed.chars().count() <= LIMIT {
        collapsed
    } else {
        let mut truncated: String = collapsed.chars().take(LIMIT).collect();
        truncated.push('…');
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempt_summaries_keep_the_readable_reason() {
        let body = r#"{"error":{"message":"Rate limit reached on Primary","type":"rate_limit_error"}}"#;
        assert_eq!(summarize_error(body), "Rate limit reached on Primary");
        assert_eq!(summarize_error(r#"{"error":"plain failure"}"#), "plain failure");
        assert_eq!(summarize_error(r#"{"message":"bad request"}"#), "bad request");
        // Proxies answering with HTML must not flood the log entry.
        let page = format!("<html>\n  <body>{}</body>\n</html>", "challenge ".repeat(80));
        let summary = summarize_error(&page);
        assert_eq!(summary.chars().count(), 241);
        assert!(summary.ends_with('…'));
        assert_eq!(
            summarize_error("   "),
            "Upstream returned an error without a body"
        );
    }

    #[test]
    fn test_normalize_openai_req_for_upstream() {
        let mut req = OpenAIChatRequest {
            model: "deepseek-chat".to_string(),
            messages: vec![
                OpenAIMessage {
                    role: "developer".to_string(),
                    content: Some(json!([
                        { "type": "text", "text": "System instructions part 1" },
                        { "type": "text", "text": "System instructions part 2" }
                    ])),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                    name: None,
                },
                OpenAIMessage {
                    role: "human".to_string(),
                    content: Some(json!([
                        { "type": "text", "text": "Hello world" }
                    ])),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                    name: None,
                },
                OpenAIMessage {
                    role: "model".to_string(),
                    content: Some(json!("Sure, I can help.")),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                    name: None,
                },
                OpenAIMessage {
                    role: "user".to_string(),
                    content: Some(json!([
                        { "type": "text", "text": "Look at image" },
                        { "type": "image_url", "image_url": { "url": "https://example.com/img.png" } }
                    ])),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                    name: None,
                },
            ],
            temperature: None,
            max_tokens: None,
            max_completion_tokens: Some(4096),
            reasoning_effort: None,
            tools: Some(vec![]),
            tool_choice: Some(json!("none")),
            stream: Some(false),
            stream_options: Some(json!({"include_usage": true})),
        };

        normalize_openai_req_for_upstream(&mut req);

        // max_completion_tokens transferred to max_tokens for older providers
        assert_eq!(req.max_tokens, Some(4096));
        assert_eq!(req.max_completion_tokens, None);

        // Empty tools and orphaned tool_choice stripped
        assert!(req.tools.is_none());
        assert_eq!(req.tool_choice, None);

        // Non-stream stream_options stripped
        assert_eq!(req.stream_options, None);

        // 1. "developer" -> "system", text array flattened to string
        assert_eq!(req.messages[0].role, "system");
        assert_eq!(
            req.messages[0].content,
            Some(Value::String("System instructions part 1\nSystem instructions part 2".to_string()))
        );

        // 2. "human" -> "user", text array flattened to string
        assert_eq!(req.messages[1].role, "user");
        assert_eq!(
            req.messages[1].content,
            Some(Value::String("Hello world".to_string()))
        );

        // 3. "model" -> "assistant"
        assert_eq!(req.messages[2].role, "assistant");
        assert_eq!(
            req.messages[2].content,
            Some(Value::String("Sure, I can help.".to_string()))
        );

        // 4. Multimodal message preserved as array
        assert_eq!(req.messages[3].role, "user");
        assert!(req.messages[3].content.as_ref().unwrap().is_array());
    }

    #[test]
    fn test_resolve_forward_user_agent() {
        assert_eq!(
            resolve_forward_user_agent(Some("codex_exec/0.154.0 (Mac OS)"), "codex_cli"),
            "codex_exec/0.154.0 (Mac OS)"
        );
        assert_eq!(
            resolve_forward_user_agent(None, "codex_cli"),
            crate::oauth::codex_user_agent()
        );
        assert_eq!(
            resolve_forward_user_agent(None, "claude_code"),
            "Claude-Code/0.2.29"
        );
        assert_eq!(
            resolve_forward_user_agent(Some("   "), "gemini_cli"),
            "gemini-cli/1.0.0"
        );
        assert!(resolve_forward_user_agent(None, "custom").starts_with("Pivot/"));
    }

    #[test]
    fn test_resolve_dispatch_timeout() {
        let mut cand = RouteCandidate {
            provider: crate::db::Provider {
                id: "prov_1".to_string(),
                name: "Standard OpenAI".to_string(),
                protocol: "openai".to_string(),
                base_url: "https://api.openai.com".to_string(),
                api_keys: vec!["k".to_string()],
                models: vec!["gpt-4o".to_string()],
                enabled: true,
                priority: 10,
                weight: 1,
                timeout_secs: 60,
                latency_ms: None,
                created_at: None,
                updated_at: None,
                preset_id: None,
                default_model: None,
                is_default: false,
                auth_type: None,
            },
            target_model: "gpt-4o".to_string(),
            api_key: None,
        };

        // Standard provider keeps configured timeout
        assert_eq!(resolve_dispatch_timeout(&cand, 1000), Duration::from_secs(60));

        // Gemini protocol: base timeout defaults to at least 180s
        cand.provider.protocol = "gemini".to_string();
        assert_eq!(resolve_dispatch_timeout(&cand, 1000), Duration::from_secs(180));

        // Large context (e.g. 500k prompt tokens) scales dynamically: 180 + (500k/50k)*15 = 180 + 150 = 330s
        assert_eq!(resolve_dispatch_timeout(&cand, 500_000), Duration::from_secs(330));
    }
}
