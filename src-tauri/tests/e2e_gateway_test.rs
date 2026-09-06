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
use tauri_app_lib::db::{Database, Provider, RouteRule};
use tauri_app_lib::gateway::router::RoutingEngine;
use tauri_app_lib::gateway::server::{create_gateway_router, GatewayState, GatewayStatsInternal};

static MOCK_CALL_COUNT_A: AtomicUsize = AtomicUsize::new(0);
static MOCK_CALL_COUNT_B: AtomicUsize = AtomicUsize::new(0);

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
    let mock_a_router = Router::new().route("/chat/completions", post(mock_a_upstream_route));
    async fn mock_a_upstream_route(body: Json<Value>) -> impl IntoResponse {
        mock_upstream_a_handler(body).await
    }
    let listener_a = tokio::net::TcpListener::bind("127.0.0.1:17881").await.unwrap();
    tokio::spawn(async move {
        axum::serve(listener_a, mock_a_router).await.unwrap();
    });

    let mock_b_router = Router::new().route("/chat/completions", post(mock_b_upstream_route));
    async fn mock_b_upstream_route(body: Json<Value>) -> impl IntoResponse {
        mock_upstream_b_handler(body).await
    }
    let listener_b = tokio::net::TcpListener::bind("127.0.0.1:17882").await.unwrap();
    tokio::spawn(async move {
        axum::serve(listener_b, mock_b_router).await.unwrap();
    });

    // 2. Setup SQLite database with Primary and Fallback providers
    let db_path = std::env::temp_dir().join(format!("test_omniswitch_{}.db", uuid::Uuid::new_v4()));
    let db = Database::new(db_path).unwrap();

    let primary_provider = Provider {
        id: "provider_primary".to_string(),
        name: "Primary OpenAI Mock".to_string(),
        protocol: "openai".to_string(),
        base_url: "http://127.0.0.1:17881".to_string(),
        api_keys: vec!["sk-test-key-a".to_string()],
        models: vec!["gpt-4o".to_string(), "claude-3-7-sonnet".to_string()],
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
        api_keys: vec!["sk-test-key-b".to_string()],
        models: vec!["deepseek-chat".to_string()],
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

    // 3. Start OmniSwitch Gateway on port 16899
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

    println!("E2E Gateway Routing & Fallback Queue Test PASSED!");
}
