use crate::db::{Database, LogAttempt, Provider, RequestLog};
use crate::gateway::adapters::{
    anthropic_to_gemini, anthropic_to_openai, finish_gemini_stream, finish_openai_stream,
    openai_to_anthropic, parse_anthropic_event_to_openai_chunks_with_state,
    parse_gemini_chunk_to_anthropic_events, parse_openai_chunk_to_anthropic_events, responses,
    AnthropicToOpenAiSseState, SseLineBuffer, SseState,
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
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
// use tauri::Emitter;
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
    pub log_sender: Option<tokio::sync::broadcast::Sender<crate::db::RequestLog>>,
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
                    let block_type = item.get("type").and_then(Value::as_str).unwrap_or("");
                    if block_type == "image" {
                        total += 1000;
                    } else if let Some(t) = item.get("text").and_then(Value::as_str) {
                        total += estimate_text_tokens(t);
                    } else if let Some(thinking) = item.get("thinking").and_then(Value::as_str) {
                        total += estimate_text_tokens(thinking);
                    } else if let Some(input) = item.get("input") {
                        total += estimate_text_tokens(&input.to_string());
                    } else if let Some(content) = item.get("content") {
                        match content {
                            Value::String(s) => total += estimate_text_tokens(s),
                            Value::Array(sub_arr) => {
                                for sub in sub_arr {
                                    if sub.get("type").and_then(Value::as_str) == Some("image") {
                                        total += 1000;
                                    } else if let Some(st) = sub.get("text").and_then(Value::as_str) {
                                        total += estimate_text_tokens(st);
                                    } else {
                                        total += estimate_text_tokens(&sub.to_string());
                                    }
                                }
                            }
                            _ => total += estimate_text_tokens(&content.to_string()),
                        }
                    } else {
                        total += estimate_text_tokens(&item.to_string());
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
                Value::Array(arr) => {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            total += estimate_text_tokens(s);
                        } else if let Some(t) = item.get("text").and_then(Value::as_str) {
                            total += estimate_text_tokens(t);
                        } else if let Some(content_str) = item.get("content").and_then(Value::as_str) {
                            total += estimate_text_tokens(content_str);
                        } else if let Some(p_type) = item.get("type").and_then(Value::as_str) {
                            if p_type == "image_url" || p_type == "input_image" || p_type == "image" {
                                total += 1000;
                            }
                        }
                    }
                }
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

fn sanitize_openai_tool_parameters(parameters: &mut Value, function_name: &str) {
    if !parameters.is_object() {
        if function_name == "exec" || function_name == "js" {
            *parameters = json!({
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "JavaScript code to execute in isolate"
                    }
                },
                "required": ["code"]
            });
        } else {
            *parameters = json!({ "type": "object", "properties": {} });
        }
        return;
    }

    let map = match parameters.as_object_mut() {
        Some(m) => m,
        None => return,
    };

    // 1. Remove $schema
    map.remove("$schema");

    // 2. Remove top-level logical combinators forbidden by OpenAI
    // ("schema must have type 'object' and not have 'oneOf'/'anyOf'/'allOf'/'enum'/'const'/'not' at the top level")
    let combinator_variants = map
        .remove("anyOf")
        .or_else(|| map.remove("oneOf"))
        .or_else(|| map.remove("allOf"));
    map.remove("anyOf");
    map.remove("oneOf");
    map.remove("allOf");
    map.remove("enum");
    map.remove("const");
    map.remove("not");

    // If properties is missing, try recovering properties and required from the first object variant in anyOf/oneOf/allOf
    if !map.contains_key("properties") {
        if let Some(variants) = combinator_variants {
            if let Some(first_obj) = variants.as_array().and_then(|arr| arr.iter().find(|v| v.is_object())) {
                if let Some(props) = first_obj.get("properties") {
                    map.insert("properties".to_string(), props.clone());
                }
                if let Some(req) = first_obj.get("required") {
                    map.insert("required".to_string(), req.clone());
                }
            }
        }
    }

    // 3. Ensure "type" is strictly "object"
    let has_type_object = map.get("type").and_then(Value::as_str) == Some("object");
    if !has_type_object {
        map.insert("type".to_string(), json!("object"));
    }

    // 4. Ensure "properties" is present and is an object
    let has_properties_object = map.get("properties").map(|p| p.is_object()).unwrap_or(false);
    if !has_properties_object {
        if function_name == "exec" || function_name == "js" {
            map.insert("properties".to_string(), json!({
                "code": {
                    "type": "string",
                    "description": "JavaScript code to execute in isolate"
                }
            }));
            if !map.contains_key("required") {
                map.insert("required".to_string(), json!(["code"]));
            }
        } else {
            map.insert("properties".to_string(), json!({}));
        }
    } else if (function_name == "exec" || function_name == "js")
        && map["properties"].as_object().map(|p| p.is_empty()).unwrap_or(false)
    {
        // For exec / js, if properties is empty {}, populate code parameter
        map.insert("properties".to_string(), json!({
            "code": {
                "type": "string",
                "description": "JavaScript code to execute in isolate"
            }
        }));
        if !map.contains_key("required") {
            map.insert("required".to_string(), json!(["code"]));
        }
    }

    // 5. Ensure "required" only lists property names that exist in "properties"
    let valid_prop_keys: Option<std::collections::HashSet<String>> = map
        .get("properties")
        .and_then(Value::as_object)
        .map(|p| p.keys().cloned().collect());

    if let Some(req_val) = map.get_mut("required") {
        if let Some(req_arr) = req_val.as_array_mut() {
            if let Some(ref valid_keys) = valid_prop_keys {
                req_arr.retain(|item| {
                    item.as_str().map(|s| valid_keys.contains(s)).unwrap_or(false)
                });
            } else {
                *req_val = json!([]);
            }
        } else {
            map.remove("required");
        }
    }
}

/// 判断目标模型是否原生支持 OpenAI 的 reasoning_effort 调节参数
/// 仅 OpenAI 官方推理模型（o1, o3, o4, gpt-5, gpt-6 系列，排除 o1-preview）支持该参数；
/// 其余非推理模型（如 gpt-4o 等）及绝大多数第三方服务商（如 DeepSeek, Qwen 等）会报 400 错误。
fn is_openai_reasoning_effort_supported(model: &str) -> bool {
    let lower = model.to_lowercase();
    let name = lower.split('/').last().unwrap_or(&lower);
    (name.starts_with("o1")
        || name.starts_with("o3")
        || name.starts_with("o4")
        || name.starts_with("gpt-5")
        || name.starts_with("gpt-6"))
        && !name.contains("preview")
}

/// Normalize OpenAIChatRequest before sending to upstream OpenAI-compatible endpoints.
/// 1. Roles: translates "developer" -> "system", "model" -> "assistant", "human" -> "user"
///    (e.g. DeepSeek, Ollama, Groq, Mistral, Moonshot reject "developer" with 422).
/// 2. Content: flattens text-only array contents into single strings
///    (e.g. DeepSeek rejects array content with "invalid type: sequence, expected a string").
/// 3. Tokens & Stream: reconciles max_tokens/max_completion_tokens and strips orphaned stream_options/tools.
fn normalize_openai_req_for_upstream(req: &mut OpenAIChatRequest) {
    if is_openai_reasoning_effort_supported(&req.model) {
        // OpenAI 官方推理模型要求使用 max_completion_tokens，拒绝已废弃的 max_tokens
        if req.max_completion_tokens.is_none() && req.max_tokens.is_some() {
            req.max_completion_tokens = req.max_tokens.take();
        } else if req.max_completion_tokens.is_some() {
            req.max_tokens = None;
        }
    } else if req.max_tokens.is_none() && req.max_completion_tokens.is_some() {
        req.max_tokens = req.max_completion_tokens.take();
    } else if req.max_tokens.is_some() {
        req.max_completion_tokens = None;
    }

    if req.stream != Some(true) {
        req.stream_options = None;
    }

    const MAX_OPENAI_TOOLS: usize = 128;

    if let Some(ref mut tools) = req.tools {
        // 1. Sanitize tool schemas ($schema removal, ensure type=object, properties, and strip top-level combinators)
        for t in tools.iter_mut() {
            sanitize_openai_tool_parameters(&mut t.function.parameters, &t.function.name);
        }

        // 2. Deduplicate tools by function name and drop tools with empty function names
        let mut seen = std::collections::HashSet::new();
        tools.retain(|t| {
            let name = t.function.name.trim();
            !name.is_empty() && seen.insert(name.to_string())
        });

        // 3. Limit tools to OpenAI's strict maximum of 128 items
        if tools.len() > MAX_OPENAI_TOOLS {
            let original_count = tools.len();
            tracing::warn!(
                "Tools count ({}) exceeds OpenAI maximum limit of {}; prioritizing active/chosen tools and capping at 128",
                original_count,
                MAX_OPENAI_TOOLS
            );

            // Extract required tool name from tool_choice if present
            let required_tool_name = req.tool_choice.as_ref().and_then(|tc| {
                if let Some(s) = tc.as_str() {
                    if s != "auto" && s != "none" && s != "required" {
                        Some(s.to_string())
                    } else {
                        None
                    }
                } else if let Some(obj) = tc.as_object() {
                    obj.get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(Value::as_str)
                        .map(|s| s.to_string())
                } else {
                    None
                }
            });

            // Collect names of tools referenced in recent message history (e.g. tool_calls in assistant messages or tool results)
            let mut priority_names = std::collections::HashSet::new();
            if let Some(ref name) = required_tool_name {
                priority_names.insert(name.clone());
            }

            for msg in req.messages.iter().rev().take(10) {
                if let Some(ref tcs) = msg.tool_calls {
                    for tc in tcs {
                        if !tc.function.name.is_empty() {
                            priority_names.insert(tc.function.name.clone());
                        }
                    }
                }
                if let Some(ref name) = msg.name {
                    if !name.is_empty() {
                        priority_names.insert(name.clone());
                    }
                }
            }

            // Partition into priority tools and remaining tools
            let mut prioritized = Vec::with_capacity(MAX_OPENAI_TOOLS);
            let mut remaining = Vec::with_capacity(tools.len());

            for t in tools.drain(..) {
                if priority_names.contains(&t.function.name) && prioritized.len() < MAX_OPENAI_TOOLS {
                    prioritized.push(t);
                } else {
                    remaining.push(t);
                }
            }

            let needed = MAX_OPENAI_TOOLS.saturating_sub(prioritized.len());
            prioritized.extend(remaining.into_iter().take(needed));

            *tools = prioritized;
        }

        if tools.is_empty() {
            req.tools = None;
            req.tool_choice = None;
        }
    } else {
        req.tool_choice = None;
    }

    // 4. Validate tool_choice integrity against available tools
    if let Some(ref tc) = req.tool_choice {
        let specified_tool = if let Some(s) = tc.as_str() {
            if s != "auto" && s != "none" && s != "required" {
                Some(s.to_string())
            } else {
                None
            }
        } else if let Some(obj) = tc.as_object() {
            obj.get("function")
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        } else {
            None
        };

        if let Some(name) = specified_tool {
            let exists = req
                .tools
                .as_ref()
                .map(|ts| ts.iter().any(|t| t.function.name == name))
                .unwrap_or(false);
            if !exists {
                req.tool_choice = Some(Value::String("auto".to_string()));
            }
        }
    }

    if let Some(ref mut effort) = req.reasoning_effort {
        if effort.is_empty()
            || effort.eq_ignore_ascii_case("none")
            || !is_openai_reasoning_effort_supported(&req.model)
        {
            req.reasoning_effort = None;
        } else {
            let lower = req.model.to_lowercase();
            let name = lower.split('/').last().unwrap_or(&lower);
            if effort.eq_ignore_ascii_case("ultra") {
                *effort = if name.starts_with("gpt-5") || name.starts_with("gpt-6") {
                    "max".to_string()
                } else {
                    "high".to_string()
                };
            } else if effort.eq_ignore_ascii_case("xhigh") || effort.eq_ignore_ascii_case("max") {
                if !name.starts_with("gpt-5") && !name.starts_with("gpt-6") {
                    *effort = "high".to_string();
                }
            }
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
            let provider_prefix = crate::gateway::router::provider_route_prefix(&p.name);
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
        "x-opencode-session",
        "x-session-id",
        "session-id",
        "anthropic-session-id",
        "x-claude-session-id",
        "x-codex-session-id",
        "conversation-id",
        "x-conversation-id",
    ] {
        if let Some(val) = headers.get(*key).and_then(|v| v.to_str().ok()) {
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn is_opencode_provider(provider: &Provider) -> bool {
    provider.preset_id.as_deref() == Some("opencode")
        || provider.base_url.contains("opencode.ai")
        || provider.name.to_lowercase().contains("opencode")
}

fn stable_uuid_from_seed(seed: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let hash = hasher.finalize();
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        hash[0], hash[1], hash[2], hash[3],
        hash[4], hash[5],
        (hash[6] & 0x0f) | 0x40, // RFC 4122 v4 marker
        hash[7],
        (hash[8] & 0x3f) | 0x80, // RFC 4122 variant marker
        hash[9], hash[10], hash[11], hash[12], hash[13], hash[14], hash[15]
    )
}

fn derive_opencode_session_id(
    headers: &HeaderMap,
    session_hint: Option<&str>,
    req_id: &str,
) -> String {
    // 1. 优先从请求头携带的会话 ID 获取
    for key in &[
        "x-opencode-session",
        "x-session-id",
        "session-id",
        "anthropic-session-id",
        "x-claude-session-id",
        "x-codex-session-id",
        "conversation-id",
        "x-conversation-id",
    ] {
        if let Some(val) = headers.get(*key).and_then(|v| v.to_str().ok()) {
            let trimmed = val.trim();
            if !trimmed.is_empty()
                && !trimmed.starts_with("Bearer ")
                && !trimmed.starts_with("sk-")
                && !trimmed.starts_with("oc_sk_")
            {
                if uuid::Uuid::parse_str(trimmed).is_ok() {
                    return trimmed.to_string();
                }
                return stable_uuid_from_seed(trimmed);
            }
        }
    }

    // 2. 基于首条消息/上下文提示生成稳定会话 UUID，保证同会话多轮对话复用同一个 Prompt Cache
    if let Some(hint) = session_hint.map(str::trim).filter(|s| !s.is_empty()) {
        return stable_uuid_from_seed(hint);
    }

    // 3. 兜底回落为本次请求的合法 UUID
    if uuid::Uuid::parse_str(req_id).is_ok() {
        req_id.to_string()
    } else {
        uuid::Uuid::new_v4().to_string()
    }
}

fn apply_upstream_headers(
    mut builder: reqwest::RequestBuilder,
    provider: &Provider,
    headers: &HeaderMap,
    session_hint: Option<&str>,
    req_id: &str,
) -> reqwest::RequestBuilder {
    if is_opencode_provider(provider) || headers.contains_key("x-opencode-session") {
        let session_str = derive_opencode_session_id(headers, session_hint, req_id);
        builder = builder.header("x-opencode-session", session_str);
    }
    builder
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
        if ua_lower.contains("opencode") {
            return "opencode".to_string();
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
        "opencode" => "opencode/1.0.0".to_string(),
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
            &headers,
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
    // 编程 Agent 与现代推理模型普遍伴随长上下文与深度思考阶段（实测 66k 上下文首包可达 210s+），
    // 基础首包等待超时统一保底 300s，并随提示词体量动态扩展（自动上限 600s，若显式配置更高则尊重显式配置）；
    // 网络连接不可达由全局 http_client 的 15s connect_timeout 快速拦截。
    let base = cand.provider.timeout_secs.max(300) as u64;
    let extra_scale = ((estimated_prompt as u64) / 50_000) * 15;
    let max_cap = 600u64.max(base);
    Duration::from_secs((base + extra_scale).min(max_cap))
}

fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut msg = err.to_string();
    let mut current = err.source();
    while let Some(cause) = current {
        let cause_str = cause.to_string();
        if !cause_str.is_empty() && !msg.contains(&cause_str) {
            msg.push_str(": ");
            msg.push_str(&cause_str);
        }
        current = cause.source();
    }
    msg
}

fn map_upstream_request_error(err: &reqwest::Error, timeout: Duration) -> (StatusCode, String) {
    let detail = format_error_chain(err);
    if err.is_timeout() && !err.is_connect() {
        (
            StatusCode::GATEWAY_TIMEOUT,
            format!(
                "Upstream request timed out after {}s: {}",
                timeout.as_secs(),
                detail
            ),
        )
    } else if err.is_timeout() {
        (
            StatusCode::GATEWAY_TIMEOUT,
            format!("Upstream connect timed out: {}", detail),
        )
    } else {
        (
            StatusCode::BAD_GATEWAY,
            format!("Upstream connection failed: {}", detail),
        )
    }
}

/// 统一发送 Gemini / Antigravity 上游请求（支持订阅账号池按模型选号、401 自动续期、限流故障转移及模型权限自愈）
#[allow(clippy::too_many_arguments)]
async fn send_gemini_upstream_request(
    state: &GatewayState,
    cand: &RouteCandidate,
    req_body_val: &Value,
    is_subscription_pool: bool,
    is_stream: bool,
    is_count_tokens: bool,
    timeout: Duration,
    forward_ua: &str,
    headers: &HeaderMap,
    session_hint: Option<&str>,
    req_id: &str,
    fallback_api_key: Option<&str>,
) -> Result<reqwest::Response, (StatusCode, String)> {
    let client = &state.http_client;
    let base = cand.provider.base_url.trim_end_matches('/');

    if !is_subscription_pool {
        let api_key = fallback_api_key.unwrap_or("");
        let endpoint = if is_stream {
            format!(
                "{}/models/{}:streamGenerateContent?alt=sse&key={}",
                base, cand.target_model, api_key
            )
        } else if is_count_tokens {
            format!(
                "{}/models/{}:countTokens?key={}",
                base, cand.target_model, api_key
            )
        } else {
            format!(
                "{}/models/{}:generateContent?key={}",
                base, cand.target_model, api_key
            )
        };
        let mut req_builder = client
            .post(&endpoint)
            .timeout(timeout)
            .header("user-agent", forward_ua)
            .header("content-type", "application/json");
        req_builder = apply_upstream_headers(req_builder, &cand.provider, headers, session_hint, req_id);
        let res = req_builder
            .json(req_body_val)
            .send()
            .await
            .map_err(|e| map_upstream_request_error(&e, timeout))?;
        if !res.status().is_success() {
            let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let text = res.text().await.unwrap_or_default();
            return Err((status, text));
        }
        return Ok(res);
    }

    const MAX_ACCOUNT_ATTEMPTS: usize = 4;
    let endpoint = if is_stream {
        format!("{}/v1internal:streamGenerateContent?alt=sse", base)
    } else if is_count_tokens {
        format!("{}/v1internal:countTokens", base)
    } else {
        format!("{}/v1internal:generateContent", base)
    };
    let mut excluded_accounts: Vec<String> = Vec::new();
    let mut last_upstream_err: Option<(StatusCode, String)> = None;

    let send_with_token = |token: &str| {
        let mut builder = client
            .post(&endpoint)
            .timeout(timeout)
            .header("user-agent", forward_ua)
            .header("content-type", "application/json");
        builder = apply_upstream_headers(builder, &cand.provider, headers, session_hint, req_id);
        builder
            .header("authorization", format!("Bearer {}", token))
            .header(
                "user-agent",
                "antigravity/cli/1.4.6 (aidev_client; os_type=darwin; arch=arm64; cl=964361259; auth_method=consumer)",
            )
            .json(req_body_val)
            .send()
    };

    for attempt_idx in 0..MAX_ACCOUNT_ATTEMPTS {
        let account = match crate::oauth::acquire_subscription_account_for_model(
            &state.db,
            client,
            &cand.provider.id,
            Some(&cand.target_model),
            &excluded_accounts,
        )
        .await
        {
            Ok(acc) => acc,
            Err(e) => {
                if attempt_idx > 0 && last_upstream_err.is_some() {
                    break;
                }
                return Err(e);
            }
        };

        let mut res = match send_with_token(&account.access_token).await {
            Ok(r) => r,
            Err(e) => {
                let is_req_timeout = e.is_timeout() && !e.is_connect();
                let (err_status, err_msg) = map_upstream_request_error(&e, timeout);
                if !is_req_timeout && attempt_idx + 1 < MAX_ACCOUNT_ATTEMPTS {
                    crate::oauth::mark_account_cooldown(&state.db, &account.id, 30);
                    excluded_accounts.push(account.id.clone());
                    tracing::warn!(
                        "Subscription account connection error ({}), trying next account in pool...",
                        err_msg
                    );
                    last_upstream_err = Some((err_status, err_msg));
                    continue;
                }
                return Err((err_status, err_msg));
            }
        };

        // 遇到 401 时使用单飞锁刷新 Token 并重试一次当前账号
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            match crate::oauth::ensure_or_refresh_account_token(
                &state.db,
                client,
                &account.id,
                Some(&account.access_token),
            )
            .await
            {
                Ok(refreshed) => match send_with_token(&refreshed.access_token).await {
                    Ok(retried) => {
                        res = retried;
                        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
                            let _ = state.db.set_account_auth_invalid(&refreshed.id, true);
                        }
                    }
                    Err(retry_err) => {
                        let (_, err_msg) = map_upstream_request_error(&retry_err, timeout);
                        tracing::warn!(
                            "Upstream Antigravity retry after 401 refresh hit network error for {}: {}",
                            account.id,
                            err_msg
                        );
                        crate::oauth::mark_account_cooldown(&state.db, &account.id, 30);
                        excluded_accounts.push(account.id.clone());
                        last_upstream_err = Some((
                            StatusCode::BAD_GATEWAY,
                            format!("Upstream retry network error: {}", err_msg),
                        ));
                        continue;
                    }
                },
                Err(refresh_err) => {
                    tracing::warn!(
                        "Upstream Antigravity returned 401 and refresh failed for {}: {}",
                        account.id,
                        refresh_err
                    );
                    if !crate::oauth::is_token_revoked_or_invalid(&refresh_err) {
                        crate::oauth::mark_account_cooldown(&state.db, &account.id, 30);
                    }
                    excluded_accounts.push(account.id.clone());
                    last_upstream_err = Some((StatusCode::UNAUTHORIZED, refresh_err));
                    continue;
                }
            }
        }

        if res.status().is_success() {
            return Ok(res);
        }

        let status_code = res.status();
        let status = StatusCode::from_u16(status_code.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let text = res.text().await.unwrap_or_default();

        if status == StatusCode::UNAUTHORIZED {
            let _ = state.db.set_account_auth_invalid(&account.id, true);
            excluded_accounts.push(account.id.clone());
            last_upstream_err = Some((status, text));
            continue;
        }

        if crate::oauth::is_model_unsupported_for_account(status, &text) {
            tracing::warn!(
                "Subscription account {} does not support model '{}', pruning and failing over...",
                account.id,
                cand.target_model
            );
            crate::oauth::handle_account_model_unsupported(
                &state.db,
                client,
                &account,
                &cand.target_model,
                &cand.provider.base_url,
            );
            excluded_accounts.push(account.id.clone());
            last_upstream_err = Some((status, text));
            continue;
        }

        let is_capacity_or_rate = status == StatusCode::TOO_MANY_REQUESTS
            || status == StatusCode::SERVICE_UNAVAILABLE
            || text.contains("MODEL_CAPACITY_EXHAUSTED");

        if status == StatusCode::TOO_MANY_REQUESTS {
            crate::oauth::mark_account_family_cooldown(&account.id, &cand.target_model, 300);
        } else if status == StatusCode::SERVICE_UNAVAILABLE || text.contains("MODEL_CAPACITY_EXHAUSTED") {
            crate::oauth::mark_account_family_cooldown(&account.id, &cand.target_model, 60);
        }

        if is_capacity_or_rate && attempt_idx + 1 < MAX_ACCOUNT_ATTEMPTS {
            tracing::warn!(
                "Subscription account {} returned status {} ({}), failing over to next account in pool...",
                account.id,
                status_code,
                if text.contains("MODEL_CAPACITY_EXHAUSTED") {
                    "MODEL_CAPACITY_EXHAUSTED"
                } else {
                    "HTTP error"
                }
            );
            excluded_accounts.push(account.id.clone());
            last_upstream_err = Some((status, text));
            continue;
        }

        return Err((status, text));
    }

    Err(last_upstream_err.unwrap_or_else(|| {
        (
            StatusCode::BAD_GATEWAY,
            "No response from Gemini upstream".to_string(),
        )
    }))
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
    headers: &HeaderMap,
) -> Result<DispatchResult, (StatusCode, String)> {
    let client = &state.http_client;
    let estimated_prompt = estimate_anthropic_req_prompt_tokens(req);
    let timeout = resolve_dispatch_timeout(cand, estimated_prompt);
    let effective_key = cand.api_key.as_deref().or(client_key);
    let forward_ua = resolve_forward_user_agent(client_ua, client_type);
    let session_hint = req.messages.first().map(|m| format!("{:?}", m.content));

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

            req_builder = apply_upstream_headers(
                req_builder,
                &cand.provider,
                headers,
                session_hint.as_deref(),
                req_id,
            );

            if let Some(key) = effective_key {
                req_builder = req_builder
                    .header("x-api-key", key)
                    .header("authorization", format!("Bearer {}", key));
            }

            let res = req_builder
                .json(&req_body)
                .send()
                .await
                .map_err(|e| map_upstream_request_error(&e, timeout))?;

            if !res.status().is_success() {
                let status =
                    StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                let text = res.text().await.unwrap_or_default();
                return Err((status, text));
            }

            if is_stream {
                let db_clone = state.db.clone();
                let log_sender_clone = state.log_sender.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();
                let req_id_str = req_id.to_string();
                let byte_stream = res.bytes_stream();

                let sse_stream = stream! {
                    let mut sse_buf = SseLineBuffer::new();
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;
                    let mut estimated_output_chars = 0usize;

                    let mut parse_anth_line = |line: &str| {
                        let line = line.trim();
                        if line.starts_with("data:") {
                            let payload = line.trim_start_matches("data:").trim();
                            if let Ok(val) = serde_json::from_str::<Value>(payload) {
                                if let Some(msg) = val.get("message") {
                                    if let Some(u) = msg.get("usage") {
                                        let it = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0)
                                            + u.get("cache_read_input_tokens").and_then(Value::as_u64).unwrap_or(0)
                                            + u.get("cache_creation_input_tokens").and_then(Value::as_u64).unwrap_or(0);
                                        if it > 0 {
                                            final_prompt_tokens = it as u32;
                                        }
                                    }
                                }
                                if let Some(u) = val.get("usage") {
                                    let it = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0)
                                        + u.get("cache_read_input_tokens").and_then(Value::as_u64).unwrap_or(0)
                                        + u.get("cache_creation_input_tokens").and_then(Value::as_u64).unwrap_or(0);
                                    if it > 0 {
                                        final_prompt_tokens = it as u32;
                                    }
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
                    };

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            for line in sse_buf.push_and_read_lines(&bytes) {
                                parse_anth_line(&line);
                            }
                            yield Ok::<_, std::io::Error>(bytes);
                        } else {
                            break;
                        }
                    }

                    if let Some(trailing) = sse_buf.finish() {
                        parse_anth_line(&trailing);
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = (estimated_output_chars as f64 * 0.3).ceil() as u32;
                        if final_completion_tokens == 0 { final_completion_tokens = 1; }
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    finalize_stream_log(
                        &db_clone,
                        &log_sender_clone,
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
                    .map(|u| {
                        u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0)
                            + u.get("cache_read_input_tokens").and_then(Value::as_u64).unwrap_or(0)
                            + u.get("cache_creation_input_tokens").and_then(Value::as_u64).unwrap_or(0)
                    })
                    .filter(|&v| v > 0)
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

            req_builder = apply_upstream_headers(
                req_builder,
                &cand.provider,
                headers,
                session_hint.as_deref(),
                req_id,
            );

            if let Some(key) = effective_key {
                req_builder = req_builder.header("authorization", format!("Bearer {}", key));
            }

            let res = req_builder
                .json(&openai_req)
                .send()
                .await
                .map_err(|e| map_upstream_request_error(&e, timeout))?;

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
                let log_sender_clone = state.log_sender.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut sse_state = SseState::new();
                    let mut sse_buf = SseLineBuffer::new();
                    let mut stream_aborted = false;
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        match chunk_res {
                            Ok(bytes) => {
                                for raw_line in sse_buf.push_and_read_lines(&bytes) {
                                    let line = raw_line.trim();
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
                                            line,
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
                            Err(_) => {
                                stream_aborted = true;
                                break;
                            }
                        }
                    }

                    if !stream_aborted {
                        if let Some(raw_line) = sse_buf.finish() {
                            let line = raw_line.trim();
                            if !line.is_empty() {
                                let events = parse_openai_chunk_to_anthropic_events(
                                    line,
                                    &mut sse_state,
                                    &req_id_str,
                                    &model_str,
                                );
                                for ev in events {
                                    yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                                }
                            }
                        }

                        // Safeguard: 仅当上游正常结束时补齐 content_block_stop & message_stop
                        if !sse_state.stream_finished {
                            let final_events = finish_openai_stream(&mut sse_state, "end_turn");
                            for ev in final_events {
                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                            }
                        }
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = sse_state.total_output_tokens.max(1);
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    finalize_stream_log(
                        &db_clone,
                        &log_sender_clone,
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
                dispatch_chatgpt_request(state, &openai_req, cand, req_id, is_stream, false).await?;
            if is_stream {
                let stream_id = format!("chatgpt_sub_{}", req_id.replace('-', ""));
                let req_id_str = req_id.to_string();
                let byte_stream = dispatch_res.response.into_body().into_data_stream();
                let model_str = req.model.clone();
                let db_clone = state.db.clone();
                let log_sender_clone = state.log_sender.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();
                let initial_prompt = dispatch_res.prompt_tokens;

                let anthropic_stream = stream! {
                    let mut sse_state = crate::gateway::adapters::SseState::new();
                    let mut sse_buf = SseLineBuffer::new();
                    let mut stream_aborted = false;
                    let mut final_prompt_tokens = initial_prompt;
                    let mut final_completion_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        match chunk_res {
                            Ok(bytes) => {
                                for raw_line in sse_buf.push_and_read_lines(&bytes) {
                                    let line = raw_line.trim();
                                    if line.starts_with("data:") {
                                        if !line.contains("[DONE]") {
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
                                        let events = crate::gateway::adapters::parse_openai_chunk_to_anthropic_events(
                                            line,
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
                            Err(_) => {
                                stream_aborted = true;
                                break;
                            }
                        }
                    }

                    if !stream_aborted {
                        if let Some(raw_line) = sse_buf.finish() {
                            let line = raw_line.trim();
                            if line.starts_with("data:") {
                                if !line.contains("[DONE]") {
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
                                let events = crate::gateway::adapters::parse_openai_chunk_to_anthropic_events(
                                    line,
                                    &mut sse_state,
                                    &stream_id,
                                    &model_str,
                                );
                                for ev in events {
                                    yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                                }
                            }
                        }

                        if !sse_state.stream_finished {
                            for ev in finish_openai_stream(&mut sse_state, "end_turn") {
                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                            }
                        }
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = sse_state.total_output_tokens.max(1);
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    finalize_stream_log(
                        &db_clone,
                        &log_sender_clone,
                        &req_id_str,
                        final_prompt_tokens as i32,
                        final_completion_tokens as i32,
                        stream_latency,
                    );
                    if final_prompt_tokens > initial_prompt {
                        stats_clone.total_prompt_tokens.fetch_add((final_prompt_tokens - initial_prompt) as u64, Ordering::Relaxed);
                    }
                    stats_clone.total_completion_tokens.fetch_add(final_completion_tokens as u64, Ordering::Relaxed);
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

            let res = send_gemini_upstream_request(
                state,
                cand,
                &req_body_val,
                is_subscription_pool,
                is_stream,
                false,
                timeout,
                &forward_ua,
                headers,
                session_hint.as_deref(),
                req_id,
                cand.api_key.as_deref(),
            )
            .await?;

            if is_stream {
                let req_id_str = req_id.to_string();
                let model_str = req.model.clone();
                let byte_stream = res.bytes_stream();
                let db_clone = state.db.clone();
                let log_sender_clone = state.log_sender.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut sse_state = SseState::new();
                    let mut sse_buf = SseLineBuffer::new();
                    let mut stream_aborted = false;
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        match chunk_res {
                            Ok(bytes) => {
                                for raw_line in sse_buf.push_and_read_lines(&bytes) {
                                    let line = raw_line.trim();
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
                                            line,
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
                            Err(_) => {
                                stream_aborted = true;
                                break;
                            }
                        }
                    }

                    if !stream_aborted {
                        if let Some(raw_line) = sse_buf.finish() {
                            let line = raw_line.trim();
                            if !line.is_empty() {
                                let events = parse_gemini_chunk_to_anthropic_events(
                                    line,
                                    &mut sse_state,
                                    &req_id_str,
                                    &model_str,
                                );
                                for event in events {
                                    yield Ok::<_, std::io::Error>(axum::body::Bytes::from(event));
                                }
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
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = sse_state.total_output_tokens.max(1);
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    finalize_stream_log(
                        &db_clone,
                        &log_sender_clone,
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
            &headers,
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

    if req.model.trim().is_empty() {
        state.stats.active_requests.fetch_sub(1, Ordering::Relaxed);
        state.stats.error_count.fetch_add(1, Ordering::Relaxed);
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": "Missing required parameter: 'model'",
                    "type": "invalid_request_error"
                }
            })),
        )
            .into_response();
    }

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
            &headers,
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
                        let mut sse_buf = SseLineBuffer::new();
                        let mut stream_aborted = false;

                        for await chunk_res in byte_stream {
                            match chunk_res {
                                Ok(bytes) => {
                                    for raw_line in sse_buf.push_and_read_lines(&bytes) {
                                        let line = raw_line.trim();
                                        if !line.is_empty() {
                                            let events = sse_state.process_line(line);
                                            for ev in events {
                                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    stream_aborted = true;
                                    break;
                                }
                            }
                        }

                        if !stream_aborted {
                            if let Some(raw_line) = sse_buf.finish() {
                                let line = raw_line.trim();
                                if !line.is_empty() {
                                    for ev in sse_state.process_line(line) {
                                        yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                                    }
                                }
                            }
                            let trailing = sse_state.finish();
                            for ev in trailing {
                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(ev));
                            }
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
    finalize_log: bool,
) -> Result<DispatchResult, (StatusCode, String)> {
    let client = &state.http_client;
    let estimated_prompt = estimate_openai_req_prompt_tokens(req);
    let timeout = resolve_dispatch_timeout(cand, estimated_prompt);
    let upstream_url = "https://chatgpt.com/backend-api/codex/responses";

    let mut responses_body = responses::convert_chat_req_to_responses(req, &cand.target_model);
    responses_body["store"] = json!(false);
    responses_body["stream"] = json!(true);

    const MAX_ACCOUNT_ATTEMPTS: usize = 4;
    let mut excluded_accounts: Vec<String> = Vec::new();
    let mut last_upstream_err: Option<(StatusCode, String)> = None;
    let mut res_opt = None;

    let send_with_token = |token: &str| {
        client
            .post(upstream_url)
            .timeout(timeout)
            .header("authorization", format!("Bearer {}", token))
            .header("user-agent", crate::oauth::codex_user_agent())
            .header("content-type", "application/json")
            .json(&responses_body)
            .send()
    };

    for attempt_idx in 0..MAX_ACCOUNT_ATTEMPTS {
        let account = match crate::oauth::acquire_subscription_account_for_model(
            &state.db,
            client,
            &cand.provider.id,
            Some(&cand.target_model),
            &excluded_accounts,
        )
        .await
        {
            Ok(acc) => acc,
            Err(e) => {
                if attempt_idx > 0 && last_upstream_err.is_some() {
                    break;
                }
                return Err(e);
            }
        };

        let mut res = match send_with_token(&account.access_token).await {
            Ok(r) => r,
            Err(e) => {
                let is_req_timeout = e.is_timeout() && !e.is_connect();
                let (err_status, err_msg) = map_upstream_request_error(&e, timeout);
                if !is_req_timeout && attempt_idx + 1 < MAX_ACCOUNT_ATTEMPTS {
                    crate::oauth::mark_account_cooldown(&state.db, &account.id, 30);
                    excluded_accounts.push(account.id.clone());
                    tracing::warn!(
                        "ChatGPT subscription connection error ({}), trying next account in pool...",
                        err_msg
                    );
                    last_upstream_err = Some((err_status, err_msg));
                    continue;
                }
                return Err((err_status, err_msg));
            }
        };

        // 上游返回 401 时使用单飞锁刷新 Token 并立即重试当前账号一次
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            match crate::oauth::ensure_or_refresh_account_token(
                &state.db,
                client,
                &account.id,
                Some(&account.access_token),
            )
            .await
            {
                Ok(refreshed) => match send_with_token(&refreshed.access_token).await {
                    Ok(retried) => {
                        res = retried;
                        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
                            let _ = state.db.set_account_auth_invalid(&refreshed.id, true);
                        }
                    }
                    Err(retry_err) => {
                        let (_, err_msg) = map_upstream_request_error(&retry_err, timeout);
                        tracing::warn!(
                            "Upstream ChatGPT retry after 401 refresh hit network error for {}: {}",
                            account.id,
                            err_msg
                        );
                        crate::oauth::mark_account_cooldown(&state.db, &account.id, 30);
                        excluded_accounts.push(account.id.clone());
                        last_upstream_err = Some((
                            StatusCode::BAD_GATEWAY,
                            format!("Upstream retry network error: {}", err_msg),
                        ));
                        continue;
                    }
                },
                Err(refresh_err) => {
                    tracing::warn!(
                        "Upstream ChatGPT returned 401 and refresh failed for {}: {}",
                        account.id,
                        refresh_err
                    );
                    if !crate::oauth::is_token_revoked_or_invalid(&refresh_err) {
                        crate::oauth::mark_account_cooldown(&state.db, &account.id, 30);
                    }
                    excluded_accounts.push(account.id.clone());
                    last_upstream_err = Some((StatusCode::UNAUTHORIZED, refresh_err));
                    continue;
                }
            }
        }

        if res.status().is_success() {
            res_opt = Some(res);
            break;
        }

        let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let text = res.text().await.unwrap_or_default();

        if status == StatusCode::UNAUTHORIZED {
            let _ = state.db.set_account_auth_invalid(&account.id, true);
            excluded_accounts.push(account.id.clone());
            last_upstream_err = Some((status, text));
            continue;
        }

        if crate::oauth::is_model_unsupported_for_account(status, &text) {
            tracing::warn!(
                "ChatGPT account {} does not support model '{}', pruning and failing over...",
                account.id,
                cand.target_model
            );
            crate::oauth::handle_account_model_unsupported(
                &state.db,
                client,
                &account,
                &cand.target_model,
                &cand.provider.base_url,
            );
            excluded_accounts.push(account.id.clone());
            last_upstream_err = Some((status, text));
            continue;
        }

        if status == StatusCode::TOO_MANY_REQUESTS {
            crate::oauth::mark_account_cooldown(&state.db, &account.id, 300);
            if attempt_idx + 1 < MAX_ACCOUNT_ATTEMPTS {
                excluded_accounts.push(account.id.clone());
                last_upstream_err = Some((status, text));
                continue;
            }
        } else if status == StatusCode::SERVICE_UNAVAILABLE {
            crate::oauth::mark_account_cooldown(&state.db, &account.id, 60);
            if attempt_idx + 1 < MAX_ACCOUNT_ATTEMPTS {
                excluded_accounts.push(account.id.clone());
                last_upstream_err = Some((status, text));
                continue;
            }
        }

        return Err((status, text));
    }

    let res = match res_opt {
        Some(r) => r,
        None => {
            return Err(last_upstream_err.unwrap_or_else(|| {
                (
                    StatusCode::BAD_GATEWAY,
                    "No response from ChatGPT upstream".to_string(),
                )
            }))
        }
    };

    if is_stream {
        let req_id_str = req_id.to_string();
        let byte_stream = res.bytes_stream();
        let model_str = cand.target_model.clone();
        let db_clone = state.db.clone();
        let log_sender_clone = state.log_sender.clone();
        let stats_clone = state.stats.clone();
        let start_time_clone = Instant::now();
        let initial_prompt = estimated_prompt;

        let sse_stream = stream! {
            let mut parser = responses::ResponsesToChatSseParser::new(&req_id_str, &model_str);
            let mut sse_buf = SseLineBuffer::new();
            let mut stream_aborted = false;
            let mut final_prompt_tokens = initial_prompt;
            let mut final_completion_tokens = 0u32;
            let mut generated_chunks = 0u32;

            let mut collect_chunk_stats = |cl: &str| {
                let trimmed = cl.trim();
                if trimmed.starts_with("data:") && !trimmed.contains("[DONE]") {
                    if let Ok(val) = serde_json::from_str::<Value>(trimmed.trim_start_matches("data:").trim()) {
                        if let Some(u) = val.get("usage") {
                            if let Some(pt) = u.get("prompt_tokens").and_then(Value::as_u64) {
                                if pt > 0 { final_prompt_tokens = pt as u32; }
                            }
                            if let Some(ct) = u.get("completion_tokens").and_then(Value::as_u64) {
                                if ct > 0 { final_completion_tokens = ct as u32; }
                            }
                        }
                        if val["choices"][0]["delta"]["content"].as_str().is_some()
                            || val["choices"][0]["delta"]["tool_calls"].is_array()
                        {
                            generated_chunks += 1;
                        }
                    }
                }
            };

            for await chunk_res in byte_stream {
                match chunk_res {
                    Ok(bytes) => {
                        for raw_line in sse_buf.push_and_read_lines(&bytes) {
                            let chat_lines = parser.process_line(raw_line.trim());
                            for cl in chat_lines {
                                collect_chunk_stats(&cl);
                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(cl));
                            }
                        }
                    }
                    Err(_) => {
                        stream_aborted = true;
                        break;
                    }
                }
            }

            if !stream_aborted {
                if let Some(raw_line) = sse_buf.finish() {
                    for cl in parser.process_line(raw_line.trim()) {
                        collect_chunk_stats(&cl);
                        yield Ok::<_, std::io::Error>(axum::body::Bytes::from(cl));
                    }
                }
                let finish_lines = parser.finish();
                for fl in finish_lines {
                    yield Ok::<_, std::io::Error>(axum::body::Bytes::from(fl));
                }
            }

            if finalize_log {
                if final_completion_tokens == 0 {
                    final_completion_tokens = generated_chunks.max(1);
                }
                let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                finalize_stream_log(
                    &db_clone,
                    &log_sender_clone,
                    &req_id_str,
                    final_prompt_tokens as i32,
                    final_completion_tokens as i32,
                    stream_latency,
                );
                if final_prompt_tokens > initial_prompt {
                    stats_clone.total_prompt_tokens.fetch_add((final_prompt_tokens - initial_prompt) as u64, Ordering::Relaxed);
                }
                stats_clone.total_completion_tokens.fetch_add(final_completion_tokens as u64, Ordering::Relaxed);
            }
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
        let byte_stream = res.bytes_stream();
        let req_id_str = req_id.to_string();
        let model_str = cand.target_model.clone();

        let mut parser = responses::ResponsesToChatSseParser::new(&req_id_str, &model_str);
        let mut sse_buf = SseLineBuffer::new();
        let mut full_content = String::new();
        let mut tool_calls_map: std::collections::BTreeMap<u64, (String, String, String)> =
            std::collections::BTreeMap::new();
        let mut finish_reason = "stop".to_string();
        let mut prompt_tokens = estimated_prompt;
        let mut completion_tokens = 0u32;
        let mut stream_abort_err: Option<String> = None;

        let mut consume_chat_lines = |chat_lines: Vec<String>| {
            for cl in chat_lines {
                let trimmed = cl.trim();
                if trimmed.starts_with("data:") && !trimmed.contains("[DONE]") {
                    if let Ok(val) = serde_json::from_str::<Value>(trimmed.trim_start_matches("data:").trim()) {
                        if let Some(choice) = val.get("choices").and_then(Value::as_array).and_then(|a| a.first()) {
                            if let Some(c) = choice["delta"]["content"].as_str() {
                                full_content.push_str(c);
                            }
                            if let Some(tc_arr) = choice["delta"]["tool_calls"].as_array() {
                                for tc in tc_arr {
                                    let idx = tc.get("index").and_then(Value::as_u64).unwrap_or(0);
                                    let entry = tool_calls_map.entry(idx).or_insert_with(|| {
                                        (String::new(), String::new(), String::new())
                                    });
                                    if let Some(id) = tc.get("id").and_then(Value::as_str) {
                                        if !id.is_empty() {
                                            entry.0 = id.to_string();
                                        }
                                    }
                                    if let Some(func) = tc.get("function") {
                                        if let Some(name) = func.get("name").and_then(Value::as_str) {
                                            if !name.is_empty() {
                                                entry.1 = name.to_string();
                                            }
                                        }
                                        if let Some(args) = func.get("arguments").and_then(Value::as_str) {
                                            entry.2.push_str(args);
                                        }
                                    }
                                }
                            }
                            if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
                                finish_reason = fr.to_string();
                            }
                        }
                        if let Some(u) = val.get("usage") {
                            prompt_tokens = u["prompt_tokens"].as_u64().unwrap_or(prompt_tokens as u64) as u32;
                            completion_tokens = u["completion_tokens"].as_u64().unwrap_or(completion_tokens as u64) as u32;
                        }
                    }
                }
            }
        };

        use tokio_stream::StreamExt;
        tokio::pin!(byte_stream);
        while let Some(chunk_res) = byte_stream.next().await {
            match chunk_res {
                Ok(bytes) => {
                    for raw_line in sse_buf.push_and_read_lines(&bytes) {
                        consume_chat_lines(parser.process_line(raw_line.trim()));
                    }
                }
                Err(e) => {
                    stream_abort_err = Some(e.to_string());
                    break;
                }
            }
        }
        if stream_abort_err.is_none() {
            if let Some(raw_line) = sse_buf.finish() {
                consume_chat_lines(parser.process_line(raw_line.trim()));
            }
            consume_chat_lines(parser.finish());
        }
        drop(consume_chat_lines);

        if let Some(err) = stream_abort_err {
            if full_content.is_empty() && tool_calls_map.is_empty() {
                return Err((StatusCode::BAD_GATEWAY, format!("Upstream stream interrupted: {}", err)));
            }
        }

        let tool_calls_json: Vec<Value> = tool_calls_map
            .into_iter()
            .map(|(_, (id, name, arguments))| {
                json!({
                    "id": if id.is_empty() { format!("call_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]) } else { id },
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": arguments
                    }
                })
            })
            .collect();

        let mut msg_obj = json!({
            "role": "assistant",
            "content": if !tool_calls_json.is_empty() && full_content.is_empty() {
                Value::Null
            } else {
                Value::String(full_content)
            }
        });
        if !tool_calls_json.is_empty() {
            msg_obj["tool_calls"] = Value::Array(tool_calls_json);
            finish_reason = "tool_calls".to_string();
        }

        let resp_json = json!({
            "id": format!("chatcmpl_{}", req_id_str.replace('-', "")),
            "object": "chat.completion",
            "created": chrono::Utc::now().timestamp(),
            "model": model_str,
            "choices": [{
                "index": 0,
                "message": msg_obj,
                "finish_reason": finish_reason
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
    headers: &HeaderMap,
) -> Result<DispatchResult, (StatusCode, String)> {
    let client = &state.http_client;
    let mut req_body = req.clone();
    req_body.model = cand.target_model.clone();
    normalize_openai_req_for_upstream(&mut req_body);

    let estimated_prompt = estimate_openai_req_prompt_tokens(&req_body);
    let timeout = resolve_dispatch_timeout(cand, estimated_prompt);
    let effective_key = cand.api_key.as_deref().or(client_key);
    let forward_ua = resolve_forward_user_agent(client_ua, client_type);
    let session_hint = req_body.messages.first().map(|m| format!("{:?}", m.content));

    match cand.provider.protocol.as_str() {
        "chatgpt" => {
            dispatch_chatgpt_request(state, &req_body, cand, req_id, is_stream, true).await
        }
        "openai" | "openrouter" | "ollama" => {
            let url = resolve_upstream_endpoint(&cand.provider.base_url, "/chat/completions");

            let mut req_builder = client
                .post(&url)
                .timeout(timeout)
                .header("user-agent", &forward_ua)
                .header("content-type", "application/json");

            req_builder = apply_upstream_headers(
                req_builder,
                &cand.provider,
                headers,
                session_hint.as_deref(),
                req_id,
            );

            if let Some(key) = effective_key {
                req_builder = req_builder.header("authorization", format!("Bearer {}", key));
            }

            let res = req_builder
                .json(&req_body)
                .send()
                .await
                .map_err(|e| map_upstream_request_error(&e, timeout))?;

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
                let log_sender_clone = state.log_sender.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut sse_buf = SseLineBuffer::new();
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;
                    let mut generated_tokens = 0u32;

                    let mut parse_oai_line = |raw_line: &str| {
                        let line = raw_line.trim();
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
                    };

                    for await chunk_res in byte_stream {
                        if let Ok(bytes) = chunk_res {
                            for raw_line in sse_buf.push_and_read_lines(&bytes) {
                                parse_oai_line(&raw_line);
                            }
                            yield Ok::<_, std::io::Error>(bytes);
                        } else {
                            break;
                        }
                    }

                    if let Some(trailing) = sse_buf.finish() {
                        parse_oai_line(&trailing);
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = generated_tokens.max(1);
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    finalize_stream_log(
                        &db_clone,
                        &log_sender_clone,
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
                openai_to_anthropic::convert_openai_req_to_anthropic(&req_body, &cand.target_model);
            let url = resolve_upstream_endpoint(&cand.provider.base_url, "/messages");

            let mut req_builder = client
                .post(&url)
                .timeout(timeout)
                .header("user-agent", &forward_ua)
                .header("content-type", "application/json")
                .header("anthropic-version", "2023-06-01");

            req_builder = apply_upstream_headers(
                req_builder,
                &cand.provider,
                headers,
                session_hint.as_deref(),
                req_id,
            );

            if let Some(key) = effective_key {
                req_builder = req_builder
                    .header("x-api-key", key)
                    .header("authorization", format!("Bearer {}", key));
            }

            let res = req_builder
                .json(&anthropic_req)
                .send()
                .await
                .map_err(|e| map_upstream_request_error(&e, timeout))?;

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
                let log_sender_clone = state.log_sender.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut sse_buf = SseLineBuffer::new();
                    let mut anth_state = AnthropicToOpenAiSseState::default();
                    let mut stream_aborted = false;
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;
                    let mut generated_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        match chunk_res {
                            Ok(bytes) => {
                                for raw_line in sse_buf.push_and_read_lines(&bytes) {
                                    let line = raw_line.trim();
                                    if !line.is_empty() {
                                        if line.starts_with("data:") {
                                            let payload = line.trim_start_matches("data:").trim();
                                            if let Ok(val) = serde_json::from_str::<Value>(payload) {
                                                if let Some(msg) = val.get("message") {
                                                    if let Some(u) = msg.get("usage") {
                                                        let it = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0)
                                                            + u.get("cache_read_input_tokens").and_then(Value::as_u64).unwrap_or(0)
                                                            + u.get("cache_creation_input_tokens").and_then(Value::as_u64).unwrap_or(0);
                                                        if it > 0 { final_prompt_tokens = it as u32; }
                                                    }
                                                }
                                                if let Some(u) = val.get("usage") {
                                                    let it = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0)
                                                        + u.get("cache_read_input_tokens").and_then(Value::as_u64).unwrap_or(0)
                                                        + u.get("cache_creation_input_tokens").and_then(Value::as_u64).unwrap_or(0);
                                                    if it > 0 { final_prompt_tokens = it as u32; }
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

                                        let chunks = parse_anthropic_event_to_openai_chunks_with_state(
                                            line,
                                            &req_id_str,
                                            &model_str,
                                            &mut anth_state,
                                        );
                                        for c in chunks {
                                            yield Ok::<_, std::io::Error>(axum::body::Bytes::from(c));
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                stream_aborted = true;
                                break;
                            }
                        }
                    }

                    if !stream_aborted {
                        if let Some(raw_line) = sse_buf.finish() {
                            let line = raw_line.trim();
                            if !line.is_empty() {
                                let chunks = parse_anthropic_event_to_openai_chunks_with_state(
                                    line,
                                    &req_id_str,
                                    &model_str,
                                    &mut anth_state,
                                );
                                for c in chunks {
                                    yield Ok::<_, std::io::Error>(axum::body::Bytes::from(c));
                                }
                            }
                        }
                    }

                    if final_completion_tokens == 0 {
                        final_completion_tokens = generated_tokens.max(1);
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    finalize_stream_log(
                        &db_clone,
                        &log_sender_clone,
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

            let req_body_val = if is_subscription_pool {
                json!({
                    "project": "aicode-consumers",
                    "model": cand.target_model,
                    "request": serde_json::to_value(&gemini_req).unwrap_or(json!({}))
                })
            } else {
                serde_json::to_value(&gemini_req).unwrap_or(json!({}))
            };

            let res = send_gemini_upstream_request(
                state,
                cand,
                &req_body_val,
                is_subscription_pool,
                is_stream,
                false,
                timeout,
                &forward_ua,
                headers,
                session_hint.as_deref(),
                req_id,
                cand.api_key.as_deref().or(client_key),
            )
            .await?;

            if is_stream {
                let req_id_str = req_id.to_string();
                let model_str = req.model.clone();
                let byte_stream = res.bytes_stream();
                let db_clone = state.db.clone();
                let log_sender_clone = state.log_sender.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut sse_buf = SseLineBuffer::new();
                    let mut sse_state = SseState::new();
                    let mut anth_state = AnthropicToOpenAiSseState::default();
                    let mut stream_aborted = false;
                    sse_state.total_input_tokens = estimated_prompt;
                    let mut final_prompt_tokens = estimated_prompt;
                    let mut final_completion_tokens = 0u32;

                    for await chunk_res in byte_stream {
                        match chunk_res {
                            Ok(bytes) => {
                                for raw_line in sse_buf.push_and_read_lines(&bytes) {
                                    let line = raw_line.trim();
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
                                            line,
                                            &mut sse_state,
                                            &req_id_str,
                                            &model_str,
                                        );
                                        for a_evt in anthropic_events {
                                            let oai_chunks = parse_anthropic_event_to_openai_chunks_with_state(
                                                &a_evt,
                                                &req_id_str,
                                                &model_str,
                                                &mut anth_state,
                                            );
                                            for o_chunk in oai_chunks {
                                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(o_chunk));
                                            }
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                stream_aborted = true;
                                break;
                            }
                        }
                    }

                    if !stream_aborted {
                        if let Some(raw_line) = sse_buf.finish() {
                            let line = raw_line.trim();
                            if !line.is_empty() {
                                let anthropic_events = parse_gemini_chunk_to_anthropic_events(
                                    line,
                                    &mut sse_state,
                                    &req_id_str,
                                    &model_str,
                                );
                                for a_evt in anthropic_events {
                                    let oai_chunks = parse_anthropic_event_to_openai_chunks_with_state(
                                        &a_evt,
                                        &req_id_str,
                                        &model_str,
                                        &mut anth_state,
                                    );
                                    for o_chunk in oai_chunks {
                                        yield Ok::<_, std::io::Error>(axum::body::Bytes::from(o_chunk));
                                    }
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
                            let oai_chunks = parse_anthropic_event_to_openai_chunks_with_state(
                                &a_evt,
                                &req_id_str,
                                &model_str,
                                &mut anth_state,
                            );
                            for o_chunk in oai_chunks {
                                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(o_chunk));
                            }
                        }
                    } else if final_completion_tokens == 0 {
                        final_completion_tokens = sse_state.total_output_tokens.max(1);
                    }
                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    finalize_stream_log(
                        &db_clone,
                        &log_sender_clone,
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
            &headers,
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
    method: &str,
    client_key: Option<&str>,
    client_ua: Option<&str>,
    client_type: &str,
    headers: &HeaderMap,
) -> Result<DispatchResult, (StatusCode, String)> {
    let timeout = resolve_dispatch_timeout(cand, 0);
    let forward_ua = resolve_forward_user_agent(client_ua, client_type);
    let is_count_tokens = method == "countTokens";
    let session_hint = req_body
        .get("contents")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .map(|v| v.to_string());

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

            let req_body_val = if is_subscription_pool {
                json!({
                    "project": "aicode-consumers",
                    "model": cand.target_model,
                    "request": sanitized_req
                })
            } else {
                sanitized_req
            };

            let res = send_gemini_upstream_request(
                state,
                cand,
                &req_body_val,
                is_subscription_pool,
                is_stream,
                is_count_tokens,
                timeout,
                &forward_ua,
                headers,
                session_hint.as_deref(),
                req_id,
                cand.api_key.as_deref().or(client_key),
            )
            .await?;

            if is_stream {
                let req_id_str = req_id.to_string();
                let byte_stream = res.bytes_stream();
                let db_clone = state.db.clone();
                let log_sender_clone = state.log_sender.clone();
                let stats_clone = state.stats.clone();
                let start_time_clone = Instant::now();

                let sse_stream = stream! {
                    let mut sse_buf = SseLineBuffer::new();
                    let mut final_prompt_tokens = 0u32;
                    let mut final_completion_tokens = 0u32;

                    let mut handle_gemini_line = |raw_line: &str| -> Option<String> {
                        let line = raw_line.trim();
                        if line.is_empty() {
                            return None;
                        }
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
                                    return Some(format!("data: {}\n\n", serde_json::to_string(actual_val).unwrap_or_default()));
                                }
                            }
                            return Some(format!("{}\n\n", line));
                        }
                        Some(format!("{}\n", line))
                    };

                    for await chunk_res in byte_stream {
                        match chunk_res {
                            Ok(bytes) => {
                                for raw_line in sse_buf.push_and_read_lines(&bytes) {
                                    if let Some(out) = handle_gemini_line(&raw_line) {
                                        yield Ok::<_, std::io::Error>(axum::body::Bytes::from(out));
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                    }

                    if let Some(trailing) = sse_buf.finish() {
                        if let Some(out) = handle_gemini_line(&trailing) {
                            yield Ok::<_, std::io::Error>(axum::body::Bytes::from(out));
                        }
                    }

                    let stream_latency = start_time_clone.elapsed().as_millis() as i64;
                    finalize_stream_log(
                        &db_clone,
                        &log_sender_clone,
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
    if let Some(ref sender) = state.log_sender {
        let _ = sender.send(log.clone());
    }
}

fn finalize_stream_log(
    db: &Database,
    log_sender: &Option<tokio::sync::broadcast::Sender<crate::db::RequestLog>>,
    req_id: &str,
    prompt_tokens: i32,
    completion_tokens: i32,
    stream_latency: i64,
) {
    if let Ok(Some(updated_log)) =
        db.update_request_log_tokens(req_id, prompt_tokens, completion_tokens, stream_latency)
    {
        if let Some(ref sender) = log_sender {
            let _ = sender.send(updated_log);
        }
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

        // 5. reasoning_effort on unsupported models (e.g. deepseek-chat, gpt-4o) stripped
        let mut non_reasoning_req = OpenAIChatRequest {
            model: "deepseek-chat".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            max_completion_tokens: None,
            reasoning_effort: Some("high".to_string()),
            tools: None,
            tool_choice: None,
            stream: None,
            stream_options: None,
        };
        normalize_openai_req_for_upstream(&mut non_reasoning_req);
        assert_eq!(non_reasoning_req.reasoning_effort, None);

        // 6. reasoning_effort on supported models (e.g. o3-mini, o4-mini) preserved and normalized
        let mut reasoning_req = OpenAIChatRequest {
            model: "o3-mini".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            max_completion_tokens: Some(2048),
            reasoning_effort: Some("ultra".to_string()),
            tools: None,
            tool_choice: None,
            stream: None,
            stream_options: None,
        };
        normalize_openai_req_for_upstream(&mut reasoning_req);
        assert_eq!(reasoning_req.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(reasoning_req.max_completion_tokens, Some(2048));
        assert_eq!(reasoning_req.max_tokens, None);

        let mut o4_req = OpenAIChatRequest {
            model: "o4-mini".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: Some(1024),
            max_completion_tokens: None,
            reasoning_effort: Some("medium".to_string()),
            tools: None,
            tool_choice: None,
            stream: None,
            stream_options: None,
        };
        normalize_openai_req_for_upstream(&mut o4_req);
        assert_eq!(o4_req.max_completion_tokens, Some(1024));
        assert_eq!(o4_req.max_tokens, None);
        assert_eq!(o4_req.reasoning_effort.as_deref(), Some("medium"));
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

        // Legacy 60s providers are elevated to at least 300s to prevent premature cutoff during thinking/long-context
        assert_eq!(resolve_dispatch_timeout(&cand, 1000), Duration::from_secs(300));

        // Anthropic protocol shares the same minimum 300s floor and dynamic context scaling
        cand.provider.protocol = "anthropic".to_string();
        assert_eq!(resolve_dispatch_timeout(&cand, 1000), Duration::from_secs(300));
        assert_eq!(resolve_dispatch_timeout(&cand, 200_000), Duration::from_secs(360));

        // Explicit higher timeout_secs (e.g. 420s) is respected
        cand.provider.timeout_secs = 420;
        assert_eq!(resolve_dispatch_timeout(&cand, 1000), Duration::from_secs(420));

        // Gemini protocol: base timeout defaults to at least 300s and scales up to 600s cap
        cand.provider.timeout_secs = 60;
        cand.provider.protocol = "gemini".to_string();
        assert_eq!(resolve_dispatch_timeout(&cand, 1000), Duration::from_secs(300));
        assert_eq!(resolve_dispatch_timeout(&cand, 500_000), Duration::from_secs(450));
        assert_eq!(resolve_dispatch_timeout(&cand, 2_000_000), Duration::from_secs(600));
    }

    #[tokio::test]
    async fn test_map_upstream_request_error_distinguishes_timeout_and_connection() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((_socket, _)) = listener.accept().await {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let timeout_err = client
            .post(format!("http://{}/v1/messages", addr))
            .timeout(Duration::from_millis(50))
            .send()
            .await
            .unwrap_err();
        let (status, msg) = map_upstream_request_error(&timeout_err, Duration::from_secs(180));
        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
        assert!(msg.contains("Upstream request timed out after 180s"), "unexpected msg: {}", msg);

        // Closed port triggers connection refused -> 502 BAD_GATEWAY with underlying cause
        let dead_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead_listener.local_addr().unwrap();
        drop(dead_listener);
        let conn_err = client
            .post(format!("http://{}/v1/messages", dead_addr))
            .send()
            .await
            .unwrap_err();
        let (conn_status, conn_msg) = map_upstream_request_error(&conn_err, Duration::from_secs(180));
        assert_eq!(conn_status, StatusCode::BAD_GATEWAY);
        assert!(conn_msg.starts_with("Upstream connection failed:"), "unexpected msg: {}", conn_msg);
        assert!(conn_msg.len() > "Upstream connection failed: error sending request for url ()".len());
    }

    #[test]
    fn test_is_opencode_provider() {
        let mut p = crate::db::Provider {
            id: "prov_1".to_string(),
            name: "My Custom".to_string(),
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
        };
        assert!(!is_opencode_provider(&p));

        p.preset_id = Some("opencode".to_string());
        assert!(is_opencode_provider(&p));

        p.preset_id = None;
        p.base_url = "https://opencode.ai/zen/go/v1".to_string();
        assert!(is_opencode_provider(&p));

        p.base_url = "https://api.openai.com".to_string();
        p.name = "OpenCode Go".to_string();
        assert!(is_opencode_provider(&p));
    }

    #[test]
    fn test_derive_opencode_session_id() {
        use axum::http::HeaderMap;

        // 1. Direct valid UUID in header
        let mut headers = HeaderMap::new();
        let valid_uuid = "123e4567-e89b-12d3-a456-426614174000";
        headers.insert("x-opencode-session", valid_uuid.parse().unwrap());
        assert_eq!(derive_opencode_session_id(&headers, None, "req-1"), valid_uuid);

        // 2. Arbitrary string session header converted to stable valid UUID
        let mut headers2 = HeaderMap::new();
        headers2.insert("x-session-id", "my-custom-session-string".parse().unwrap());
        let derived = derive_opencode_session_id(&headers2, None, "req-1");
        assert!(uuid::Uuid::parse_str(&derived).is_ok());
        // Stable across calls with same seed
        assert_eq!(derived, derive_opencode_session_id(&headers2, None, "req-2"));

        // 3. Fallback to session_hint when header is missing
        let empty_headers = HeaderMap::new();
        let derived_from_hint = derive_opencode_session_id(&empty_headers, Some("Hello AI"), "req-1");
        assert!(uuid::Uuid::parse_str(&derived_from_hint).is_ok());
        assert_eq!(derived_from_hint, derive_opencode_session_id(&empty_headers, Some("Hello AI"), "req-2"));

        // 4. Fallback to valid req_id or random UUID
        let derived_from_req = derive_opencode_session_id(&empty_headers, None, valid_uuid);
        assert_eq!(derived_from_req, valid_uuid);
    }

    #[test]
    fn test_normalize_openai_req_tools_clamping_and_prioritization() {
        use crate::gateway::types::{OpenAIFunction, OpenAITool, OpenAIToolCall, OpenAIFunctionCall};

        // Construct 150 tools
        let mut tools = Vec::new();
        for i in 0..150 {
            tools.push(OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunction {
                    name: format!("tool_{}", i),
                    description: Some(format!("Description for tool {}", i)),
                    parameters: json!({
                        "$schema": "http://json-schema.org/draft-07/schema#",
                        "type": "object",
                        "properties": {
                            "arg": { "type": "string" }
                        }
                    }),
                },
            });
        }

        // Add a duplicate tool to test deduplication
        tools.push(OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIFunction {
                name: "tool_0".to_string(),
                description: Some("duplicate tool_0".to_string()),
                parameters: json!({ "type": "object" }),
            },
        });

        // tool_145 is deep in the list (> 128) and specified as tool_choice
        // tool_140 was called in recent message tool_calls
        let mut req = OpenAIChatRequest {
            model: "gpt-4o".to_string(),
            messages: vec![
                OpenAIMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![OpenAIToolCall {
                        index: Some(0),
                        id: "call_1".to_string(),
                        call_type: "function".to_string(),
                        function: OpenAIFunctionCall {
                            name: "tool_140".to_string(),
                            arguments: "{}".to_string(),
                        },
                    }]),
                    tool_call_id: None,
                    reasoning_content: None,
                    name: None,
                },
                OpenAIMessage {
                    role: "tool".to_string(),
                    content: Some(json!("result")),
                    tool_calls: None,
                    tool_call_id: Some("call_1".to_string()),
                    reasoning_content: None,
                    name: Some("tool_140".to_string()),
                },
            ],
            temperature: None,
            max_tokens: None,
            max_completion_tokens: None,
            reasoning_effort: None,
            tools: Some(tools),
            tool_choice: Some(json!({
                "type": "function",
                "function": { "name": "tool_145" }
            })),
            stream: None,
            stream_options: None,
        };

        normalize_openai_req_for_upstream(&mut req);

        let final_tools = req.tools.as_ref().unwrap();
        // Capped at exactly 128 items
        assert_eq!(final_tools.len(), 128);

        // tool_145 (required by tool_choice) MUST be preserved in the 128 items!
        assert!(final_tools.iter().any(|t| t.function.name == "tool_145"));

        // tool_140 (referenced in messages) MUST be preserved in the 128 items!
        assert!(final_tools.iter().any(|t| t.function.name == "tool_140"));

        // $schema stripped from parameters
        assert!(final_tools[0].function.parameters.get("$schema").is_none());

        // tool_choice remains valid because tool_145 is present
        assert_eq!(
            req.tool_choice.as_ref().unwrap()["function"]["name"],
            "tool_145"
        );
    }

    #[test]
    fn test_sanitize_openai_tool_parameters_for_js_and_combinators() {
        // 1. js tool with empty schema {}
        let mut js_empty = json!({});
        sanitize_openai_tool_parameters(&mut js_empty, "js");
        assert_eq!(js_empty["type"], "object");
        assert!(js_empty["properties"].get("code").is_some());
        assert_eq!(js_empty["required"], json!(["code"]));

        // 2. tool with top-level anyOf, enum, const, not
        let mut tool_with_anyof = json!({
            "anyOf": [
                {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "required": ["query"]
                }
            ],
            "enum": ["some_value"],
            "const": "test",
            "not": { "type": "null" }
        });
        sanitize_openai_tool_parameters(&mut tool_with_anyof, "search");
        assert_eq!(tool_with_anyof["type"], "object");
        assert!(tool_with_anyof["properties"].get("query").is_some());
        assert_eq!(tool_with_anyof["required"], json!(["query"]));
        assert!(tool_with_anyof.get("anyOf").is_none());
        assert!(tool_with_anyof.get("enum").is_none());
        assert!(tool_with_anyof.get("const").is_none());
        assert!(tool_with_anyof.get("not").is_none());

        // 3. tool with invalid required keys (keys that don't exist in properties)
        let mut tool_invalid_req = json!({
            "type": "object",
            "properties": {
                "foo": { "type": "string" }
            },
            "required": ["foo", "non_existent_key"]
        });
        sanitize_openai_tool_parameters(&mut tool_invalid_req, "foo_tool");
        assert_eq!(tool_invalid_req["required"], json!(["foo"]));
    }
}
