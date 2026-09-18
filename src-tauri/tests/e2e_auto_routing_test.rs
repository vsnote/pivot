//! End-to-end coverage for the automatic model routing: users configure
//! providers, the gateway decides model names, providers and fallbacks.
//! No routing rule is configured in these tests, mirroring the acceptance run.

use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tauri_app_lib::db::{Database, Provider};
use tauri_app_lib::gateway::router::RoutingEngine;
use tauri_app_lib::gateway::server::{create_gateway_router, GatewayState, GatewayStatsInternal};

const ANTIGRAVITY_MODELS: &[&str] = &["gemini-3.8-flash-high"];
const DEEPSEEK_MODELS: &[&str] = &["deepseek-v4-pro"];

fn openai_completion(model: &str, text: &str) -> Value {
    json!({
        "id": "chatcmpl-mock",
        "object": "chat.completion",
        "created": 1700000000,
        "model": model,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": text },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 7, "completion_tokens": 3, "total_tokens": 10 }
    })
}

fn openai_stream(model: &str, text: &str) -> axum::response::Response {
    let chunk = |delta: Value| {
        json!({
            "id": "chatcmpl-mock",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": model,
            "choices": [{ "index": 0, "delta": delta, "finish_reason": null }]
        })
    };
    let finish = json!({
        "id": "chatcmpl-mock",
        "object": "chat.completion.chunk",
        "created": 1700000000,
        "model": model,
        "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 7, "completion_tokens": 3, "total_tokens": 10 }
    });
    let body = format!(
        "data: {role}\n\ndata: {text}\n\ndata: {finish}\n\ndata: [DONE]\n\n",
        role = chunk(json!({ "role": "assistant" })),
        text = chunk(json!({ "content": text })),
    );
    axum::response::Response::builder()
        .header("content-type", "text/event-stream")
        .body(axum::body::Body::from(body))
        .unwrap()
}

/// Behaves like a real upstream: a model it does not serve is a 400, and the
/// answer names the model it was asked for.
fn serve_mock(body: &Value, supported: &[&str], label: &str) -> (StatusCode, axum::response::Response) {
    let model = body.get("model").and_then(Value::as_str).unwrap_or("");
    if !supported.contains(&model) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": { "message": format!("model '{}' is not served by {}", model, label) } }))
                .into_response(),
        );
    }
    let text = format!("{} answered {}", label, model);
    if body.get("stream").and_then(Value::as_bool).unwrap_or(false) {
        return (StatusCode::OK, openai_stream(model, &text));
    }
    (StatusCode::OK, Json(openai_completion(model, &text)).into_response())
}

async fn mock_antigravity(Json(body): Json<Value>) -> (StatusCode, axum::response::Response) {
    serve_mock(&body, ANTIGRAVITY_MODELS, "antigravity")
}

async fn mock_deepseek(Json(body): Json<Value>) -> (StatusCode, axum::response::Response) {
    serve_mock(&body, DEEPSEEK_MODELS, "deepseek")
}

async fn mock_offline(Json(_body): Json<Value>) -> (StatusCode, axum::response::Response) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "error": { "message": "cli-proxy is not running" } })).into_response(),
    )
}

async fn serve(port: u16, app: Router) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap_or_else(|error| panic!("port {} is unavailable: {}", port, error));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
}

async fn start_gateway(port: u16, db: Database) {
    let state = GatewayState {
        db,
        router: Arc::new(RoutingEngine::new()),
        http_client: reqwest::Client::new(),
        stats: Arc::new(GatewayStatsInternal::default()),
        app_handle: None,
    };
    serve(port, create_gateway_router(state)).await;
    tokio::time::sleep(Duration::from_millis(120)).await;
}

fn provider(id: &str, name: &str, port: u16, models: &[&str], priority: i32) -> Provider {
    Provider {
        id: id.to_string(),
        name: name.to_string(),
        protocol: "openai".to_string(),
        base_url: format!("http://127.0.0.1:{}", port),
        preset_id: None,
        api_keys: vec!["sk-test".to_string()],
        models: models.iter().map(|m| m.to_string()).collect(),
        default_model: None,
        is_default: false,
        enabled: true,
        priority,
        weight: 1,
        timeout_secs: 10,
        latency_ms: None,
        created_at: None,
        updated_at: None,
    }
}

fn as_default(mut provider: Provider, default_model: &str) -> Provider {
    provider.is_default = true;
    provider.default_model = Some(default_model.to_string());
    provider
}

struct TempDatabase {
    path: std::path::PathBuf,
}

impl TempDatabase {
    fn new(label: &str) -> Self {
        Self {
            path: std::env::temp_dir().join(format!("omni-e2e-{}-{}.db", label, uuid::Uuid::new_v4())),
        }
    }
}

impl Drop for TempDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

/// A database that only holds what the test declares. The seeded presets are
/// removed on purpose: they ship enabled with the preset model list, which would
/// otherwise win the exact-match step (and call the real Anthropic API).
fn temp_db(label: &str) -> Database {
    let temp = TempDatabase::new(label);
    let db = Database::new(temp.path.clone()).unwrap();
    for preset in db.get_providers().unwrap() {
        db.delete_provider(&preset.id).unwrap();
    }
    assert!(db.get_providers().unwrap().is_empty());
    // Routing rules are not seeded any more; these tests add none.
    assert!(db.get_routes().unwrap().is_empty());
    db
}

async fn post_messages(client: &reqwest::Client, port: u16, model: &str, stream: bool) -> reqwest::Response {
    client
        .post(format!("http://127.0.0.1:{}/v1/messages", port))
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": model,
            "messages": [{ "role": "user", "content": "ping" }],
            "max_tokens": 32,
            "stream": stream
        }))
        .send()
        .await
        .unwrap()
}

/// 验收 1 + 2: Claude Code 的模型名（包括后台小模型）在没有任何规则时也要能通。
#[tokio::test]
async fn claude_model_names_without_rules_use_the_default_provider() {
    serve(17911, Router::new().route("/chat/completions", post(mock_antigravity))).await;
    serve(17912, Router::new().route("/chat/completions", post(mock_deepseek))).await;

    let db = temp_db("claude-names");
    db.save_provider(&as_default(
        provider("antigravity", "Antigravity (cli-proxy)", 17911, ANTIGRAVITY_MODELS, 10),
        "gemini-3.8-flash-high",
    ))
    .unwrap();
    db.save_provider(&provider("deepseek", "DeepSeek", 17912, DEEPSEEK_MODELS, 5))
        .unwrap();
    start_gateway(16911, db.clone()).await;

    let client = reqwest::Client::new();

    for model in ["claude-3-7-sonnet-20250219", "claude-3-5-haiku-20241022"] {
        let response = post_messages(&client, 16911, model, false).await;
        assert_eq!(response.status(), 200, "{} should be served", model);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body.get("type").and_then(Value::as_str), Some("message"));
        // The default provider was asked for its own model, not the Claude name.
        assert!(
            body["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("gemini-3.8-flash-high"),
            "unexpected answer: {}",
            body
        );
    }

    // Claude Code streams its background model requests too.
    let streamed = post_messages(&client, 16911, "claude-3-5-haiku-20241022", true).await;
    assert_eq!(streamed.status(), 200);
    let stream_body = streamed.text().await.unwrap();
    assert!(stream_body.contains("event: message_start"), "{}", stream_body);
    assert!(stream_body.contains("gemini-3.8-flash-high"), "{}", stream_body);

    // No rule exists, and every request is explained by the attempt chain.
    assert!(db.get_routes().unwrap().is_empty());
    let logs = db.get_logs(10, 0).unwrap();
    assert_eq!(logs.len(), 3);
    for log in &logs {
        assert_eq!(log.routed_provider_id, "antigravity");
        assert_eq!(log.routed_model, "gemini-3.8-flash-high");
        assert_eq!(log.status_code, 200);
        assert!(!log.fallback_used);
        assert_eq!(log.attempts.len(), 1);
        assert_eq!(log.attempts[0].provider_id, "antigravity");
        assert_eq!(log.attempts[0].model, "gemini-3.8-flash-high");
        assert_eq!(log.attempts[0].status, 200);
    }
}

/// 验收 3 + 4: 精确模型名仍然匹配到声明它的服务商。
#[tokio::test]
async fn exact_model_names_route_to_the_provider_that_declares_them() {
    serve(17921, Router::new().route("/chat/completions", post(mock_antigravity))).await;
    serve(17922, Router::new().route("/chat/completions", post(mock_deepseek))).await;

    let db = temp_db("exact-names");
    db.save_provider(&as_default(
        provider("antigravity", "Antigravity (cli-proxy)", 17921, ANTIGRAVITY_MODELS, 10),
        "gemini-3.8-flash-high",
    ))
    .unwrap();
    db.save_provider(&provider("deepseek", "DeepSeek", 17922, DEEPSEEK_MODELS, 5))
        .unwrap();
    start_gateway(16921, db.clone()).await;

    let client = reqwest::Client::new();

    for (model, provider_id) in [
        ("gemini-3.8-flash-high", "antigravity"),
        ("deepseek-v4-pro", "deepseek"),
    ] {
        let response = post_messages(&client, 16921, model, false).await;
        assert_eq!(response.status(), 200, "{} should be served", model);
        let logs = db.get_logs(1, 0).unwrap();
        assert_eq!(logs[0].routed_provider_id, provider_id);
        assert_eq!(logs[0].routed_model, model);
        assert_eq!(logs[0].attempts.len(), 1);
        assert_eq!(logs[0].attempts[0].provider_id, provider_id);
    }

    // The exact match reaches the same provider through the OpenAI endpoint as well.
    let response = client
        .post("http://127.0.0.1:16921/v1/chat/completions")
        .json(&json!({
            "model": "deepseek-v4-pro",
            "messages": [{ "role": "user", "content": "ping" }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let logs = db.get_logs(1, 0).unwrap();
    assert_eq!(logs[0].routed_provider_id, "deepseek");
    assert_eq!(logs[0].status_code, 200);
}

/// 验收 5: 默认服务商不可用时按同名模型切换，attempts 记录每一次尝试。
#[tokio::test]
async fn unavailable_default_provider_falls_back_to_the_same_model_name() {
    serve(17931, Router::new().route("/chat/completions", post(mock_antigravity))).await;
    serve(17932, Router::new().route("/chat/completions", post(mock_deepseek))).await;
    // The configured default provider answers with a server error.
    serve(17933, Router::new().route("/chat/completions", post(mock_offline))).await;

    let db = temp_db("default-down");
    // The broken provider holds the catch-all slot ...
    db.save_provider(&as_default(
        provider("flaky-default", "Flaky aggregator", 17933, &[], 20),
        "gemini-3.8-flash-high",
    ))
    .unwrap();
    // ... and this one serves the very same model name.
    db.save_provider(&provider("antigravity", "Antigravity (cli-proxy)", 17931, ANTIGRAVITY_MODELS, 10))
        .unwrap();
    db.save_provider(&provider("deepseek", "DeepSeek", 17932, DEEPSEEK_MODELS, 5))
        .unwrap();
    start_gateway(16931, db.clone()).await;

    let client = reqwest::Client::new();
    let response = post_messages(&client, 16931, "claude-3-7-sonnet-20250219", false).await;
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert!(body["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("gemini-3.8-flash-high"));

    let logs = db.get_logs(1, 0).unwrap();
    assert_eq!(logs[0].status_code, 200);
    assert!(logs[0].fallback_used);
    assert_eq!(logs[0].routed_provider_id, "antigravity");
    assert_eq!(logs[0].routed_model, "gemini-3.8-flash-high");
    assert_eq!(logs[0].attempts.len(), 2, "both hops must stay visible");
    assert_eq!(logs[0].attempts[0].provider_id, "flaky-default");
    assert_eq!(logs[0].attempts[0].status, 503);
    assert!(logs[0].attempts[0]
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("cli-proxy is not running"));
    assert_eq!(logs[0].attempts[1].provider_id, "antigravity");
    assert_eq!(logs[0].attempts[1].status, 200);
    assert_eq!(logs[0].attempts[1].model, "gemini-3.8-flash-high");
    assert_eq!(logs[0].error_message, None, "a successful request keeps no error");
}

/// 无默认服务商时返回明确错误，不再静默落到优先级最高的服务商。
#[tokio::test]
async fn unknown_model_without_default_provider_reports_a_routing_error() {
    serve(17941, Router::new().route("/chat/completions", post(mock_deepseek))).await;

    let db = temp_db("no-default");
    db.save_provider(&provider("deepseek", "DeepSeek", 17941, DEEPSEEK_MODELS, 10))
        .unwrap();
    start_gateway(16941, db.clone()).await;

    let client = reqwest::Client::new();
    let response = post_messages(&client, 16941, "claude-3-7-sonnet-20250219", false).await;

    assert_eq!(response.status(), 502);
    let body: Value = response.json().await.unwrap();
    let message = body["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("claude-3-7-sonnet-20250219"), "{}", message);
    assert!(message.contains("default provider"), "{}", message);
    assert_eq!(body["error"]["type"], "routing_error");

    // The failure is visible in the log table instead of only in the response.
    let logs = db.get_logs(1, 0).unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].status_code, 502);
    assert_eq!(logs[0].routed_provider_id, "none");
    assert!(logs[0].attempts.is_empty());
    assert!(logs[0]
        .error_message
        .as_deref()
        .unwrap_or_default()
        .contains("no default provider"));
}

/// 规则仍然生效并覆盖自动解析（高级用户的逃生舱）。
#[tokio::test]
async fn an_explicit_rule_still_overrides_automatic_routing() {
    serve(17951, Router::new().route("/chat/completions", post(mock_antigravity))).await;
    serve(17952, Router::new().route("/chat/completions", post(mock_deepseek))).await;

    let db = temp_db("rule-override");
    db.save_provider(&as_default(
        provider("antigravity", "Antigravity (cli-proxy)", 17951, ANTIGRAVITY_MODELS, 10),
        "gemini-3.8-flash-high",
    ))
    .unwrap();
    db.save_provider(&provider("deepseek", "DeepSeek", 17952, DEEPSEEK_MODELS, 5))
        .unwrap();
    db.save_routes(&[tauri_app_lib::db::RouteRule {
        id: "route_deepseek".to_string(),
        pattern: "claude-3-7-*".to_string(),
        target_provider_id: "deepseek".to_string(),
        target_model: Some("deepseek-v4-pro".to_string()),
        fallback_provider_ids: vec!["antigravity".to_string()],
        enabled: true,
        priority: 100,
        description: None,
    }])
    .unwrap();
    start_gateway(16951, db.clone()).await;

    let client = reqwest::Client::new();
    let response = post_messages(&client, 16951, "claude-3-7-sonnet-20250219", false).await;
    assert_eq!(response.status(), 200);

    let logs = db.get_logs(1, 0).unwrap();
    assert_eq!(logs[0].routed_provider_id, "deepseek");
    assert_eq!(logs[0].routed_model, "deepseek-v4-pro");
    assert_eq!(logs[0].attempts[0].provider_id, "deepseek");
}
