use crate::db::{Database, LogAttempt, RequestLog};
use crate::gateway::adapters::{
    anthropic_to_gemini, anthropic_to_openai, finish_gemini_stream, openai_to_anthropic,
    parse_anthropic_event_to_openai_chunks, parse_gemini_chunk_to_anthropic_events,
    parse_openai_chunk_to_anthropic_events, SseState,
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
use futures_util::StreamExt;
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
        .route("/api/stats", get(get_stats))
        .layer(cors)
        .with_state(state)
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "OmniSwitch Local Gateway",
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
            &state.http_client,
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

async fn dispatch_anthropic_request(
    client: &reqwest::Client,
    req: &AnthropicMessageRequest,
    cand: &RouteCandidate,
    req_id: &str,
    is_stream: bool,
    client_key: Option<&str>,
) -> Result<DispatchResult, (StatusCode, String)> {
    let timeout = Duration::from_secs(cand.provider.timeout_secs.max(10) as u64);
    let effective_key = cand.api_key.as_deref().or(client_key);

    match cand.provider.protocol.as_str() {
        "anthropic" => {
            // Direct Anthropic upstream
            let mut url = cand.provider.base_url.trim_end_matches('/').to_string();
            if !url.ends_with("/messages") {
                url = format!("{}/messages", url);
            }

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
                let stream_bytes = res
                    .bytes_stream()
                    .map(|b| b.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(stream_bytes))
                        .unwrap(),
                    prompt_tokens: 0,
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
                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "application/json")
                        .body(Body::from(bytes))
                        .unwrap(),
                    prompt_tokens: usage
                        .as_ref()
                        .and_then(|u| u.get("input_tokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32,
                    completion_tokens: usage
                        .as_ref()
                        .and_then(|u| u.get("output_tokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32,
                })
            }
        }
        "openai" | "openrouter" | "ollama" => {
            // Convert to OpenAI Chat Completions
            let mut url = cand.provider.base_url.trim_end_matches('/').to_string();
            if !url.ends_with("/chat/completions") {
                url = format!("{}/chat/completions", url);
            }

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

                let sse_stream = stream! {
                    let mut sse_state = SseState::new();
                    let mut buffer = String::new();

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            if let Ok(s) = std::str::from_utf8(&bytes) {
                                buffer.push_str(s);

                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();

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
                            }
                        }
                    }
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
                let openai_resp: OpenAIChatResponse = res.json().await.map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to parse OpenAI response: {}", e),
                    )
                })?;
                let usage = openai_resp.usage.clone();
                let anthropic_resp =
                    anthropic_to_openai::convert_openai_resp_to_anthropic(&openai_resp, &req.model);
                Ok(DispatchResult {
                    response: Json(anthropic_resp).into_response(),
                    prompt_tokens: usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
                    completion_tokens: usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
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
                // Gemini SSE is not Anthropic SSE. Translate each upstream data line
                // so Claude Code receives the same event contract as other providers.
                let req_id_str = req_id.to_string();
                let model_str = req.model.clone();
                let byte_stream = res.bytes_stream();

                let sse_stream = stream! {
                    let mut sse_state = SseState::new();
                    let mut buffer = String::new();

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            if let Ok(s) = std::str::from_utf8(&bytes) {
                                buffer.push_str(s);
                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();
                                    if !line.is_empty() {
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
                let gemini_resp: GeminiResponse = res.json().await.map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to parse Gemini response: {}", e),
                    )
                })?;
                let usage = gemini_resp.usage_metadata.clone();
                let anthropic_resp =
                    anthropic_to_gemini::convert_gemini_resp_to_anthropic(&gemini_resp, &req.model);
                Ok(DispatchResult {
                    response: Json(anthropic_resp).into_response(),
                    prompt_tokens: usage
                        .as_ref()
                        .and_then(|u| u.prompt_token_count)
                        .unwrap_or(0),
                    completion_tokens: usage
                        .as_ref()
                        .and_then(|u| u.candidates_token_count)
                        .unwrap_or(0),
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
            &state.http_client,
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

async fn dispatch_openai_request(
    client: &reqwest::Client,
    req: &OpenAIChatRequest,
    cand: &RouteCandidate,
    req_id: &str,
    is_stream: bool,
    client_key: Option<&str>,
) -> Result<DispatchResult, (StatusCode, String)> {
    let timeout = Duration::from_secs(cand.provider.timeout_secs.max(10) as u64);
    let effective_key = cand.api_key.as_deref().or(client_key);

    match cand.provider.protocol.as_str() {
        "openai" | "openrouter" | "ollama" => {
            let mut url = cand.provider.base_url.trim_end_matches('/').to_string();
            if !url.ends_with("/chat/completions") {
                url = format!("{}/chat/completions", url);
            }

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
                let stream_bytes = res
                    .bytes_stream()
                    .map(|b| b.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(stream_bytes))
                        .unwrap(),
                    prompt_tokens: 0,
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
                Ok(DispatchResult {
                    response: Response::builder()
                        .header("content-type", "application/json")
                        .body(Body::from(bytes))
                        .unwrap(),
                    prompt_tokens: usage
                        .as_ref()
                        .and_then(|u| u.get("prompt_tokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32,
                    completion_tokens: usage
                        .as_ref()
                        .and_then(|u| u.get("completion_tokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32,
                })
            }
        }
        "anthropic" => {
            // Client called OpenAI endpoint, but upstream is Anthropic!
            let anthropic_req =
                openai_to_anthropic::convert_openai_req_to_anthropic(req, &cand.target_model);
            let mut url = cand.provider.base_url.trim_end_matches('/').to_string();
            if !url.ends_with("/messages") {
                url = format!("{}/messages", url);
            }

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

                let sse_stream = stream! {
                    let mut buffer = String::new();

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            if let Ok(s) = std::str::from_utf8(&bytes) {
                                buffer.push_str(s);

                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();

                                    if !line.is_empty() {
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
