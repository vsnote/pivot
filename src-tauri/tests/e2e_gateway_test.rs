use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Router,
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use pivot_lib::db::{Database, Provider, RouteRule};
use pivot_lib::gateway::router::RoutingEngine;
use pivot_lib::gateway::server::{create_gateway_router, GatewayState, GatewayStatsInternal};

static MOCK_CALL_COUNT_A: AtomicUsize = AtomicUsize::new(0);
static MOCK_CALL_COUNT_B: AtomicUsize = AtomicUsize::new(0);

fn mock_openai_stream_response(id: &str, model: &str, text: &str) -> axum::response::Response {
    let role_chunk = json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": 1700000000,
        "model": model,
        "choices": [{ "index": 0, "delta": { "role": "assistant" }, "finish_reason": null }]
    });
    let text_chunk = json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": 1700000000,
        "model": model,
        "choices": [{ "index": 0, "delta": { "content": text }, "finish_reason": null }]
    });
    let finish_chunk = json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": 1700000000,
        "model": model,
        "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 5, "completion_tokens": 2, "total_tokens": 7 }
    });
    let body = format!(
        "data: {role}\n\ndata: {text}\n\ndata: {finish}\n\ndata: [DONE]\n\n",
        role = role_chunk,
        text = text_chunk,
        finish = finish_chunk,
    );
    axum::response::Response::builder()
        .header("content-type", "text/event-stream")
        .body(axum::body::Body::from(body))
        .unwrap()
}

// Mock Upstream A: returns 429 on first call to trigger fallback
async fn mock_upstream_a_handler(Json(body): Json<Value>) -> (StatusCode, axum::response::Response) {
    let count = MOCK_CALL_COUNT_A.fetch_add(1, Ordering::SeqCst);
    let model = body.get("model").and_then(|v| v.as_str()).unwrap_or("");

    if model.contains("fail") || count == 0 {
        // Return 429 Rate Limit
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": { "message": "Rate limit reached on Primary", "type": "rate_limit_error" } })).into_response()
        )
    } else {
        if body.get("stream").and_then(Value::as_bool).unwrap_or(false) {
            return (
                StatusCode::OK,
                mock_openai_stream_response("chatcmpl-stream-a", model, "Hello from stream A!"),
            );
        }
        (
            StatusCode::OK,
            Json(json!({
                "id": "chatcmpl-mock-a",
                "object": "chat.completion",
                "created": 1700000000,
                "model": model,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello from Primary Provider A!"
                    },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 15, "completion_tokens": 10, "total_tokens": 25 }
            })).into_response()
        )
    }
}

// Mock Upstream B: Fallback provider that succeeds
async fn mock_upstream_b_handler(Json(body): Json<Value>) -> (StatusCode, axum::response::Response) {
    MOCK_CALL_COUNT_B.fetch_add(1, Ordering::SeqCst);
    let model = body.get("model").and_then(|v| v.as_str()).unwrap_or("");

    if body.get("stream").and_then(Value::as_bool).unwrap_or(false) {
        return (
            StatusCode::OK,
            mock_openai_stream_response("chatcmpl-stream-b", model, "Hello from stream B!"),
        );
    }

    (
        StatusCode::OK,
        Json(json!({
            "id": "chatcmpl-mock-b",
            "object": "chat.completion",
            "created": 1700000000,
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello from Fallback Provider B!",
                    "reasoning_content": "Thinking process successfully completed."
                },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 20, "completion_tokens": 12, "total_tokens": 32 }
        })).into_response()
    )
}

#[tokio::test]
async fn test_e2e_gateway_routing_and_failover() {
    // 1. Launch Mock Upstream Servers on ports 17881 and 17882
    let mock_a_router = Router::new()
        .route("/chat/completions", post(mock_a_upstream_route))
        .route("/v1/chat/completions", post(mock_a_upstream_route));
    async fn mock_a_upstream_route(body: Json<Value>) -> impl IntoResponse {
        mock_upstream_a_handler(body).await
    }
    let listener_a = tokio::net::TcpListener::bind("127.0.0.1:17881").await.unwrap();
    tokio::spawn(async move {
        axum::serve(listener_a, mock_a_router).await.unwrap();
    });

    let mock_b_router = Router::new()
        .route("/chat/completions", post(mock_b_upstream_route))
        .route("/v1/chat/completions", post(mock_b_upstream_route));
    async fn mock_b_upstream_route(body: Json<Value>) -> impl IntoResponse {
        mock_upstream_b_handler(body).await
    }
    let listener_b = tokio::net::TcpListener::bind("127.0.0.1:17882").await.unwrap();
    tokio::spawn(async move {
        axum::serve(listener_b, mock_b_router).await.unwrap();
    });

    // 2. Setup SQLite database with Primary and Fallback providers
    let db_path = std::env::temp_dir().join(format!("test_pivot_{}.db", uuid::Uuid::new_v4()));
    let db = Database::new(db_path).unwrap();

    let primary_provider = Provider {
        id: "provider_primary".to_string(),
        name: "Primary OpenAI Mock".to_string(),
        protocol: "openai".to_string(),
        base_url: "http://127.0.0.1:17881".to_string(),
        preset_id: None,
        auth_type: None,
        api_keys: vec!["sk-test-key-a".to_string()],
        models: vec!["gpt-4o".to_string(), "claude-3-7-sonnet".to_string()],
        default_model: None,
        is_default: false,
        enabled: true,
        priority: 10,
        weight: 1,
        timeout_secs: 10,
        latency_ms: None,
        created_at: None,
        updated_at: None,
    };

    let fallback_provider = Provider {
        id: "provider_fallback".to_string(),
        name: "Fallback Provider B".to_string(),
        protocol: "openai".to_string(),
        base_url: "http://127.0.0.1:17882".to_string(),
        preset_id: None,
        auth_type: None,
        api_keys: vec!["sk-test-key-b".to_string()],
        models: vec!["deepseek-chat".to_string()],
        // The primary is asked for "gpt-4o"; this provider only knows deepseek-chat,
        // so it is reached through its own default model instead of a 400.
        default_model: Some("deepseek-chat".to_string()),
        is_default: false,
        enabled: true,
        priority: 5,
        weight: 1,
        timeout_secs: 10,
        latency_ms: None,
        created_at: None,
        updated_at: None,
    };

    db.save_provider(&primary_provider).unwrap();
    db.save_provider(&fallback_provider).unwrap();

    let route_rule = RouteRule {
        id: "rule_claude_to_primary".to_string(),
        pattern: "claude-*".to_string(),
        target_provider_id: "provider_primary".to_string(),
        target_model: Some("gpt-4o".to_string()),
        fallback_provider_ids: vec!["provider_fallback".to_string()],
        enabled: true,
        priority: 100,
        description: Some("Test fallback chain".to_string()),
    };

    db.save_routes(&[route_rule]).unwrap();

    // 3. Start Pivot Gateway on port 16899
    let router = Arc::new(RoutingEngine::new());
    let http_client = reqwest::Client::new();
    let stats = Arc::new(GatewayStatsInternal::default());

    let gw_state = GatewayState {
        db: db.clone(),
        router,
        http_client: http_client.clone(),
        stats,
        app_handle: None,
    };

    let gw_app = create_gateway_router(gw_state);
    let gw_listener = tokio::net::TcpListener::bind("127.0.0.1:16899").await.unwrap();
    tokio::spawn(async move {
        axum::serve(gw_listener, gw_app).await.unwrap();
    });

    // Give servers a moment to bind
    tokio::time::sleep(Duration::from_millis(150)).await;

    // 4. Test Health Check endpoint
    let health_res = http_client
        .get("http://127.0.0.1:16899/health")
        .send()
        .await
        .unwrap();
    assert_eq!(health_res.status(), 200);

    // 5. Send Claude Code format request to `/v1/messages`
    // Because Primary Provider A returns 429 on count 0, the gateway MUST automatically
    // failover to Fallback Provider B and return 200 OK!
    let claude_req = json!({
        "model": "claude-3-7-sonnet",
        "messages": [
            { "role": "user", "content": "Write a Rust hello world" }
        ],
        "max_tokens": 1024,
        "temperature": 0.5
    });

    let resp = http_client
        .post("http://127.0.0.1:16899/v1/messages")
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .json(&claude_req)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let resp_json: Value = resp.json().await.unwrap();

    // Verify response was transformed to Anthropic format
    assert_eq!(resp_json.get("type").and_then(|v| v.as_str()), Some("message"));
    assert_eq!(resp_json.get("role").and_then(|v| v.as_str()), Some("assistant"));

    let content_blocks = resp_json.get("content").and_then(|v| v.as_array()).unwrap();
    assert!(!content_blocks.is_empty());

    // Verify fallback provider B was called
    assert_eq!(MOCK_CALL_COUNT_B.load(Ordering::SeqCst), 1);

    // Verify request log recorded fallback
    let logs = db.get_logs(10, 0).unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].status_code, 200);
    assert!(logs[0].fallback_used);
    assert_eq!(logs[0].routed_provider_id, "provider_fallback");
    assert_eq!(logs[0].prompt_tokens, 20);
    assert_eq!(logs[0].completion_tokens, 12);

    // Both hops show up in the attempt chain, each with the model name its
    // provider was actually asked for, and the primary keeps its own error.
    assert_eq!(logs[0].attempts.len(), 2);
    assert_eq!(logs[0].attempts[0].provider_id, "provider_primary");
    assert_eq!(logs[0].attempts[0].model, "gpt-4o");
    assert_eq!(logs[0].attempts[0].status, 429);
    assert!(logs[0].attempts[0]
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("Rate limit reached on Primary"));
    assert_eq!(logs[0].attempts[1].provider_id, "provider_fallback");
    assert_eq!(logs[0].attempts[1].model, "deepseek-chat");
    assert_eq!(logs[0].attempts[1].status, 200);

    // 6. Send an OpenAI-compatible request to `/v1/chat/completions`.
    // The model is listed by the fallback provider, so it should route there
    // directly and preserve the OpenAI response shape.
    let chat_req = json!({
        "model": "deepseek-chat",
        "messages": [{ "role": "user", "content": "Say hello" }]
    });
    let chat_resp = http_client
        .post("http://127.0.0.1:16899/v1/chat/completions")
        .header("content-type", "application/json")
        .json(&chat_req)
        .send()
        .await
        .unwrap();
    assert_eq!(chat_resp.status(), 200);
    let chat_json: Value = chat_resp.json().await.unwrap();
    assert_eq!(
        chat_json.get("object").and_then(|v| v.as_str()),
        Some("chat.completion")
    );
    assert_eq!(
        chat_json["usage"]["prompt_tokens"].as_u64(),
        Some(20)
    );

    // The second request should also contribute upstream usage to gateway stats.
    let stats_resp = http_client
        .get("http://127.0.0.1:16899/api/stats")
        .send()
        .await
        .unwrap();
    let stats_json: Value = stats_resp.json().await.unwrap();
    assert_eq!(stats_json["totalPromptTokens"].as_u64(), Some(40));
    assert_eq!(stats_json["totalCompletionTokens"].as_u64(), Some(24));

    // 7. Verify OpenAI SSE is translated to Anthropic SSE for Claude clients.
    let stream_resp = http_client
        .post("http://127.0.0.1:16899/v1/messages")
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": "claude-3-7-sonnet",
            "messages": [{ "role": "user", "content": "Stream hello" }],
            "max_tokens": 128,
            "stream": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(stream_resp.status(), 200);
    assert_eq!(
        stream_resp
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let stream_body = stream_resp.text().await.unwrap();
    assert!(stream_body.contains("event: message_start"));
    assert!(stream_body.contains("event: content_block_delta"));
    assert!(stream_body.contains("Hello from stream A!"));
    assert_eq!(stream_body.matches("event: message_stop").count(), 1);

    let final_stats: Value = http_client
        .get("http://127.0.0.1:16899/api/stats")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(final_stats["totalPromptTokens"].as_u64(), Some(68));
    assert_eq!(final_stats["totalCompletionTokens"].as_u64(), Some(26));

    let logs = db.get_logs(10, 0).unwrap();
    assert_eq!(logs.len(), 3);
    assert_eq!(logs[0].path, "/v1/messages");
    assert!(logs[0].is_stream);
    assert_eq!(logs[1].path, "/v1/chat/completions");
    assert_eq!(logs[1].prompt_tokens, 20);
    assert_eq!(logs[1].completion_tokens, 12);

    println!("E2E Gateway Routing, Fallback, Usage & Chat Test PASSED!");
}

#[tokio::test]
async fn test_e2e_gateway_gemini_cross_protocol_and_cli() {
    let mock_gemini_app = Router::new()
        .route(
            "/models/gemini-2.5-pro:generateContent",
            post(|| async {
                Json(json!({
                    "candidates": [{
                        "content": {
                            "role": "model",
                            "parts": [{ "text": "Gemini response text" }]
                        },
                        "finishReason": "STOP"
                    }],
                    "usageMetadata": {
                        "promptTokenCount": 10,
                        "candidatesTokenCount": 6
                    }
                }))
            }),
        )
        .route(
            "/models/gemini-2.5-pro:streamGenerateContent",
            post(|| async {
                let stream_body = concat!(
                    "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hello stream from Gemini!\"}]}}]}\n\n",
                    "data: {\"candidates\":[{\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"candidatesTokenCount\":5}}\n\n"
                );
                axum::response::Response::builder()
                    .header("content-type", "text/event-stream")
                    .body(axum::body::Body::from(stream_body))
                    .unwrap()
            }),
        );

    let gemini_listener = tokio::net::TcpListener::bind("127.0.0.1:16911").await.unwrap();
    tokio::spawn(async move {
        axum::serve(gemini_listener, mock_gemini_app).await.unwrap();
    });

    let db_path = "/tmp/pivot_test_gemini_gw.db";
    let _ = std::fs::remove_file(db_path);
    let db = Database::new(db_path.into()).unwrap();

    let gemini_prov = Provider {
        id: "prov-gemini".to_string(),
        name: "Google Gemini".to_string(),
        protocol: "gemini".to_string(),
        base_url: "http://127.0.0.1:16911".to_string(),
        preset_id: None,
        auth_type: None,
        api_keys: vec!["dummy-key".to_string()],
        models: vec!["gemini-2.5-pro".to_string()],
        default_model: None,
        is_default: false,
        enabled: true,
        priority: 1,
        weight: 1,
        timeout_secs: 30,
        latency_ms: None,
        created_at: None,
        updated_at: None,
    };
    db.save_provider(&gemini_prov).unwrap();

    let stats = Arc::new(GatewayStatsInternal::default());
    let router_engine = Arc::new(RoutingEngine::new());
    let gw_state = GatewayState {
        db: db.clone(),
        router: router_engine,
        http_client: reqwest::Client::new(),
        app_handle: None,
        stats: stats.clone(),
    };

    let gw_app = create_gateway_router(gw_state);
    let gw_listener = tokio::net::TcpListener::bind("127.0.0.1:16912").await.unwrap();
    tokio::spawn(async move {
        axum::serve(gw_listener, gw_app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    let client = reqwest::Client::new();

    // 1. OpenAI client calls /v1/chat/completions -> routed to Gemini upstream (Non-stream)
    let oai_resp = client
        .post("http://127.0.0.1:16912/v1/chat/completions")
        .json(&json!({
            "model": "gemini-2.5-pro",
            "messages": [{"role": "user", "content": "hi"}],
            "stream": false
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(oai_resp.status(), 200);
    let oai_val: Value = oai_resp.json().await.unwrap();
    assert_eq!(oai_val["object"], "chat.completion");
    assert_eq!(
        oai_val["choices"][0]["message"]["content"],
        "Gemini response text"
    );

    // 2. OpenAI client calls /v1/chat/completions -> routed to Gemini upstream (Stream)
    let stream_resp = client
        .post("http://127.0.0.1:16912/v1/chat/completions")
        .json(&json!({
            "model": "gemini-2.5-pro",
            "messages": [{"role": "user", "content": "hi stream"}],
            "stream": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(stream_resp.status(), 200);
    let stream_text = stream_resp.text().await.unwrap();
    assert!(stream_text.contains("Hello stream from Gemini!"));
    assert!(stream_text.contains("[DONE]"));

    // 3. Gemini CLI calls GET /v1beta/models
    let models_resp = client
        .get("http://127.0.0.1:16912/v1beta/models")
        .send()
        .await
        .unwrap();
    assert_eq!(models_resp.status(), 200);
    let models_val: Value = models_resp.json().await.unwrap();
    let models_arr = models_val["models"].as_array().unwrap();
    assert!(models_arr.iter().any(|m| m["name"] == "models/gemini-2.5-pro"));

    // 4. Gemini CLI calls POST /v1beta/models/gemini-2.5-pro:generateContent
    let gemini_cli_resp = client
        .post("http://127.0.0.1:16912/v1beta/models/gemini-2.5-pro:generateContent")
        .json(&json!({
            "contents": [{"parts": [{"text": "Hello Gemini CLI"}]}]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(gemini_cli_resp.status(), 200);
    let gemini_cli_val: Value = gemini_cli_resp.json().await.unwrap();
    assert_eq!(
        gemini_cli_val["candidates"][0]["content"]["parts"][0]["text"],
        "Gemini response text"
    );
}
