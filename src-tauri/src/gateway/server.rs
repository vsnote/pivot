use crate::db::{Database, LogAttempt, RequestLog};
use crate::gateway::adapters::{
    anthropic_to_gemini, anthropic_to_openai, finish_gemini_stream, openai_to_anthropic,
    parse_anthropic_event_to_openai_chunks, parse_gemini_chunk_to_anthropic_events,
    parse_openai_chunk_to_anthropic_events, responses, SseState,
};
use crate::gateway::router::{RouteCandidate, RoutingEngine};
use crate::gateway::types::*;
use async_stream::stream;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
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
        .route("/v1/responses", post(handle_responses))
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

    let session_id = extract_session_id(&headers);
    let client_key = extract_client_api_key(&headers);
    let providers = state.db.get_providers().unwrap_or_default();
    let routes = state.db.get_routes().unwrap_or_default();

    let is_stream = req.stream.unwrap_or(false);
    let route_match =
        match state
            .router
            .resolve_route(&req.model, session_id.as_deref(), &providers, &routes)
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
                        client_type: "claude_code".to_string(),
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
                        client_type: "claude_code".to_string(),
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
            client_type: "claude_code".to_string(),
            incoming_model: req.model.clone(),
            routed_provider_id: "none".to_string(),
            routed_provider_name: None,
            routed_model: "".to_string(),
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
    if path.is_empty() {
        format!("{clean}/v1{suffix}")
    } else {
        format!("{clean}{suffix}")
    }
}

async fn dispatch_anthropic_request(
    state: &GatewayState,
    req: &AnthropicMessageRequest,
    cand: &RouteCandidate,
    req_id: &str,
    is_stream: bool,
    client_key: Option<&str>,
) -> Result<DispatchResult, (StatusCode, String)> {
    let client = &state.http_client;
    let timeout = Duration::from_secs(cand.provider.timeout_secs.max(10) as u64);
    let effective_key = cand.api_key.as_deref().or(client_key);
    let estimated_prompt = estimate_anthropic_req_prompt_tokens(req);

    match cand.provider.protocol.as_str() {
        "anthropic" => {
            // Direct Anthropic upstream
            let url = resolve_upstream_endpoint(&cand.provider.base_url, "/messages");

            let mut req_body = req.clone();
            req_body.model = cand.target_model.clone();

            let mut req_builder = client
                .post(&url)
                .timeout(timeout)
                .header("content-type", "application/json")
                .header("anthropic-version", "2023-06-01");

            if let Some(key) = effective_key {
                req_builder = req_builder.header("x-api-key", key);
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

            let openai_req =
                anthropic_to_openai::convert_anthropic_req_to_openai(req, &cand.target_model);

            let mut req_builder = client
                .post(&url)
                .timeout(timeout)
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
        "gemini" => {
            let api_key = cand.api_key.as_deref().unwrap_or("");
            let base = cand.provider.base_url.trim_end_matches('/');
            let gemini_req = anthropic_to_gemini::convert_anthropic_req_to_gemini(req);

            let endpoint = if is_stream {
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

            let res = client
                .post(&endpoint)
                .timeout(timeout)
                .header("content-type", "application/json")
                .json(&gemini_req)
                .send()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Gemini connection error: {}", e),
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
                                                if let Some(usage) = val.get("usageMetadata") {
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

                    for event in finish_gemini_stream(&mut sse_state, "end_turn") {
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
                let gemini_resp: GeminiResponse = res.json().await.map_err(|e| {
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

    let session_id = extract_session_id(&headers);
    let client_key = extract_client_api_key(&headers);
    let providers = state.db.get_providers().unwrap_or_default();
    let routes = state.db.get_routes().unwrap_or_default();

    let is_stream = req.stream.unwrap_or(false);
    let route_match =
        match state
            .router
            .resolve_route(&req.model, session_id.as_deref(), &providers, &routes)
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
                        client_type: "codex_cli".to_string(),
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
            client_type: "codex_cli".to_string(),
            incoming_model: req.model.clone(),
            routed_provider_id: "none".to_string(),
            routed_provider_name: None,
            routed_model: "".to_string(),
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

    let session_id = extract_session_id(&headers);
    let client_key = extract_client_api_key(&headers);
    let providers = state.db.get_providers().unwrap_or_default();
    let routes = state.db.get_routes().unwrap_or_default();

    let is_compaction = responses::is_compaction_request(&body);
    let req = responses::convert_responses_req_to_chat(&body);
    let is_stream = req.stream.unwrap_or(true);

    let route_match = match state
        .router
        .resolve_route(&req.model, session_id.as_deref(), &providers, &routes)
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
                        client_type: "codex_cli".to_string(),
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

                    let responses_stream = stream! {
                        let mut sse_state = responses::ResponsesSseState::new_with_compaction(&response_id, &model_str, is_compaction);
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
                    let responses_val = responses::chat_response_to_responses(&chat_json, &resp_id, &req.model, is_compaction);
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
            client_type: "codex_cli".to_string(),
            incoming_model: req.model,
            routed_provider_id: "none".to_string(),
            routed_provider_name: None,
            routed_model: "".to_string(),
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

    (
        last_status,
        Json(json!({ "error": { "message": last_error, "type": "upstream_error" } })),
    )
        .into_response()
}

async fn dispatch_openai_request(
    state: &GatewayState,
    req: &OpenAIChatRequest,
    cand: &RouteCandidate,
    req_id: &str,
    is_stream: bool,
    client_key: Option<&str>,
) -> Result<DispatchResult, (StatusCode, String)> {
    let client = &state.http_client;
    let timeout = Duration::from_secs(cand.provider.timeout_secs.max(10) as u64);
    let effective_key = cand.api_key.as_deref().or(client_key);
    let estimated_prompt = estimate_openai_req_prompt_tokens(req);

    match cand.provider.protocol.as_str() {
        "openai" | "openrouter" | "ollama" => {
            let url = resolve_upstream_endpoint(&cand.provider.base_url, "/chat/completions");

            let mut req_body = req.clone();
            req_body.model = cand.target_model.clone();

            let mut req_builder = client
                .post(&url)
                .timeout(timeout)
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
                .header("content-type", "application/json")
                .header("anthropic-version", "2023-06-01");

            if let Some(key) = effective_key {
                req_builder = req_builder.header("x-api-key", key);
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
        _ => Err((
            StatusCode::BAD_GATEWAY,
            format!(
                "Unsupported protocol for chat completions: {}",
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
}
