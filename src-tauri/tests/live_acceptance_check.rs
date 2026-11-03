//! Opt-in acceptance run against real upstreams (a live local proxy plus a real
//! hosted API). It configures providers only — no routing rule — and walks the
//! five acceptance cases from the task.
//!
//! Run it explicitly, with credentials in the environment (never in a file that
//! is committed):
//!
//! ```text
//! PIVOT_LIVE_PROXY_URL=http://127.0.0.1:8317/v1 \
//! PIVOT_LIVE_PROXY_KEY=... \
//! PIVOT_LIVE_PROXY_MODEL=gemini-3.8-flash-high \
//! PIVOT_LIVE_EXACT_URL=https://api.deepseek.com/v1 \
//! PIVOT_LIVE_EXACT_KEY=... \
//! PIVOT_LIVE_EXACT_MODEL=deepseek-v4-pro \
//! cargo test --test live_acceptance_check -- --ignored --nocapture
//! ```
//!
//! Every request costs real quota, so payloads are deliberately tiny.

use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use pivot_lib::db::{Database, Provider};
use pivot_lib::gateway::router::RoutingEngine;
use pivot_lib::gateway::server::{create_gateway_router, GatewayState, GatewayStatsInternal};

const GATEWAY_PORT: u16 = 16897;
const PROXY_ID: &str = "live-proxy";
const EXACT_ID: &str = "live-exact";
const FLAKY_ID: &str = "live-flaky";

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn live_provider(id: &str, name: &str, url: &str, key: &str, models: &[String], priority: i32) -> Provider {
    Provider {
        id: id.to_string(),
        name: name.to_string(),
        protocol: "openai".to_string(),
        base_url: url.to_string(),
        preset_id: None,
        auth_type: None,
        api_keys: vec![key.to_string()],
        models: models.to_vec(),
        default_model: None,
        is_default: false,
        enabled: true,
        priority,
        weight: 1,
        timeout_secs: 20,
        latency_ms: None,
        created_at: None,
        updated_at: None,
    }
}

async fn messages(client: &reqwest::Client, model: &str, stream: bool) -> reqwest::Response {
    client
        .post(format!("http://127.0.0.1:{}/v1/messages", GATEWAY_PORT))
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": model,
            "messages": [{ "role": "user", "content": "say hi" }],
            "max_tokens": 8,
            "stream": stream
        }))
        .send()
        .await
        .expect("gateway request failed")
}

/// Holds credentials, so the file must not survive the run — including a panic.
struct TempDatabase {
    path: std::path::PathBuf,
}

impl TempDatabase {
    fn new(label: &str) -> Self {
        Self {
            path: std::env::temp_dir().join(format!("{}-{}.db", label, uuid::Uuid::new_v4())),
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

#[tokio::test]
#[ignore = "talks to real upstreams and spends real quota; run explicitly with the env vars in the header"]
async fn live_acceptance_with_providers_only() {
    let (Some(proxy_url), Some(proxy_key), Some(exact_url), Some(exact_key)) = (
        env("PIVOT_LIVE_PROXY_URL").or_else(|| env("OMNISWITCH_LIVE_PROXY_URL")),
        env("PIVOT_LIVE_PROXY_KEY").or_else(|| env("OMNISWITCH_LIVE_PROXY_KEY")),
        env("PIVOT_LIVE_EXACT_URL").or_else(|| env("OMNISWITCH_LIVE_EXACT_URL")),
        env("PIVOT_LIVE_EXACT_KEY").or_else(|| env("OMNISWITCH_LIVE_EXACT_KEY")),
    ) else {
        println!("live check skipped: set PIVOT_LIVE_PROXY_URL/KEY and PIVOT_LIVE_EXACT_URL/KEY");
        return;
    };
    let proxy_model = env("PIVOT_LIVE_PROXY_MODEL")
        .or_else(|| env("OMNISWITCH_LIVE_PROXY_MODEL"))
        .unwrap_or_else(|| "gemini-3.8-flash-high".to_string());
    let exact_model = env("PIVOT_LIVE_EXACT_MODEL")
        .or_else(|| env("OMNISWITCH_LIVE_EXACT_MODEL"))
        .unwrap_or_else(|| "deepseek-v4-pro".to_string());

    let temp = TempDatabase::new("pivot-live");
    let db = Database::new(temp.path.clone()).unwrap();
    assert!(db.get_providers().unwrap().is_empty(), "nothing is installed for the user");

    let mut proxy = live_provider(PROXY_ID, "Antigravity (cli-proxy)", &proxy_url, &proxy_key, &[proxy_model.clone()], 10);
    proxy.is_default = true;
    proxy.default_model = Some(proxy_model.clone());
    db.save_provider(&proxy).unwrap();
    db.save_provider(&live_provider(EXACT_ID, "DeepSeek", &exact_url, &exact_key, &[exact_model.clone()], 5))
        .unwrap();
    // Used for the failover case: it takes the catch-all slot later but refuses
    // connections, so the same model name has to take over.
    let mut flaky = live_provider(FLAKY_ID, "Offline aggregator", "http://127.0.0.1:9/v1", "unused", &[], 20);
    flaky.is_default = true;
    flaky.default_model = Some(proxy_model.clone());
    db.save_provider(&flaky).unwrap();

    let state = GatewayState {
        db: db.clone(),
        router: Arc::new(RoutingEngine::new()),
        http_client: reqwest::Client::new(),
        stats: Arc::new(GatewayStatsInternal::default()),
        app_handle: None,
    };
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", GATEWAY_PORT)).await.unwrap();
    tokio::spawn(async move {
        axum::serve(listener, create_gateway_router(state)).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(150)).await;

    let client = reqwest::Client::new();

    // 1 + 2: Claude Code model names, including the small background model.
    for model in ["claude-3-7-sonnet-20250219", "claude-3-5-haiku-20241022"] {
        let response = messages(&client, model, false).await;
        let status = response.status();
        let body: Value = response.json().await.unwrap();
        println!("{} -> {} ({})", model, status, body["content"][0]["text"].as_str().unwrap_or(""));
        assert!(status.is_success(), "{} answered {}", model, body);
    }

    // 2 (streaming): Claude Code reads SSE for the same model name.
    let streamed = messages(&client, "claude-3-5-haiku-20241022", true).await;
    assert!(streamed.status().is_success());
    let sse = streamed.text().await.unwrap();
    assert!(sse.contains("event: message_start"), "{}", sse);

    // 3 + 4: exact model names keep going to the provider that declares them.
    for (model, expected) in [(&proxy_model, PROXY_ID), (&exact_model, EXACT_ID)] {
        let response = messages(&client, model, false).await;
        assert!(response.status().is_success(), "{} answered {}", model, response.status());
        let logs = db.get_logs(1, 0).unwrap();
        println!("{} -> {}({})", model, logs[0].routed_provider_id, logs[0].routed_model);
        assert_eq!(logs[0].routed_provider_id, expected);
        assert_eq!(logs[0].routed_model, *model);
    }

    // 5: the default provider is offline, so the same model name takes over.
    db.save_provider(&flaky).unwrap();
    let response = messages(&client, "claude-3-7-sonnet-20250219", false).await;
    assert!(response.status().is_success(), "failover answered {}", response.status());
    let logs = db.get_logs(1, 0).unwrap();
    let chain: Vec<(String, i32)> = logs[0]
        .attempts
        .iter()
        .map(|attempt| (attempt.provider_id.clone(), attempt.status))
        .collect();
    println!("failover chain: {:?} -> {}", chain, logs[0].routed_provider_id);
    // The offline provider fails (a proxy in front of it may report 502 or 503),
    // the provider with the same model name then answers.
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].0, FLAKY_ID);
    assert!(chain[0].1 >= 400, "unexpected success: {:?}", chain);
    assert_eq!(chain[1], (PROXY_ID.to_string(), 200));
    assert_eq!(logs[0].attempts[0].model, proxy_model);
    assert_eq!(logs[0].attempts[1].model, proxy_model);

    // Without a default provider an unknown name is a clear error, not a guess.
    flaky.is_default = false;
    db.save_provider(&flaky).unwrap();
    proxy.is_default = false;
    db.save_provider(&proxy).unwrap();
    assert!(db.get_providers().unwrap().iter().all(|p| !p.is_default));
    let response = messages(&client, "claude-3-7-sonnet-20250219", false).await;
    let status = response.status();
    let body: Value = response.json().await.unwrap();
    println!("no default provider -> {} {}", status, body["error"]["message"]);
    assert_eq!(status, 502);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("no default provider"));
}

#[tokio::test]
#[ignore = "talks to real Antigravity upstream using ~/.pivot/pivot.db"]
async fn test_live_real_antigravity_and_gemini() {
    let home = dirs::home_dir().expect("no home dir");
    let db_path = home.join(".pivot").join("pivot.db");
    if !db_path.exists() {
        println!("No ~/.pivot/pivot.db found, skipping live real test.");
        return;
    }
    let db = Database::new(db_path).unwrap();
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
    let port = 16920;
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    tokio::spawn(async move {
        axum::serve(listener, gw_app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(200)).await;
    let client = reqwest::Client::new();

    // 1. Claude Code format with tools containing propertyNames and exclusiveMinimum (reproducing issue 7 & 8)
    println!("Testing real Claude Code -> Antigravity Gemini upstream...");
    let tool_schema = json!({
        "type": "object",
        "properties": {
            "cmd": {
                "type": "string",
                "propertyNames": { "pattern": "^[a-z]+$" }
            },
            "timeout": {
                "type": "integer",
                "exclusiveMinimum": 0
            }
        },
        "required": ["cmd"]
    });

    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/messages", port))
        .json(&json!({
            "model": "gemini-3.8-flash-high",
            "max_tokens": 100,
            "messages": [
                {"role": "user", "content": "Please call the Bash tool with cmd 'echo hello'."}
            ],
            "tools": [{
                "name": "Bash",
                "description": "Run bash command",
                "input_schema": tool_schema
            }],
            "stream": false
        }))
        .send()
        .await
        .unwrap();

    let status = resp.status();
    let text = resp.text().await.unwrap();
    println!("Real model response status: {}", status);
    println!("Real model response body: {}", text);
    assert!(status.is_success(), "Failed with status {}: {}", status, text);

    // 2. Stream test
    println!("Testing real streaming...");
    let stream_resp = client
        .post(format!("http://127.0.0.1:{}/v1/messages", port))
        .json(&json!({
            "model": "gemini-3.8-flash-high",
            "max_tokens": 50,
            "messages": [
                {"role": "user", "content": "Respond with 1 2 3."}
            ],
            "stream": true
        }))
        .send()
        .await
        .unwrap();

    assert!(stream_resp.status().is_success());
    let stream_body = stream_resp.text().await.unwrap();
    println!("Real stream response events length: {}", stream_body.len());
    assert!(stream_body.contains("event: message_start"));
    assert!(stream_body.contains("event: content_block_delta"));

    // 3. OpenAI format test (/v1/chat/completions -> real Antigravity upstream)
    println!("Testing real OpenAI client (/v1/chat/completions) -> Antigravity Gemini upstream...");
    let oai_resp = client
        .post(format!("http://127.0.0.1:{}/v1/chat/completions", port))
        .json(&json!({
            "model": "gemini-3.8-flash-high",
            "messages": [
                {"role": "user", "content": "Say hello in one short sentence."}
            ],
            "stream": false
        }))
        .send()
        .await
        .unwrap();

    let oai_status = oai_resp.status();
    let oai_val: Value = oai_resp.json().await.unwrap();
    println!("Real OpenAI endpoint status: {}", oai_status);
    println!("Real OpenAI response content: {:?}", oai_val["choices"][0]["message"]["content"]);
    assert!(oai_status.is_success());
    assert_eq!(oai_val["object"], "chat.completion");
    assert!(oai_val["choices"][0]["message"]["content"].is_string());

    println!("Real model live test PASSED!");
}

#[tokio::test]
async fn test_live_real_deepseek_claude_code_stream() {
    let home = dirs::home_dir().expect("no home dir");
    let db_path = home.join(".pivot").join("pivot.db");
    if !db_path.exists() {
        println!("No ~/.pivot/pivot.db found, skipping.");
        return;
    }
    let db = Database::new(db_path).unwrap();
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
    let port = 16938;
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    tokio::spawn(async move {
        axum::serve(listener, gw_app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(200)).await;
    let client = reqwest::Client::new();

    println!("Testing live DeepSeek with Claude Code streaming request...");
    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/messages", port))
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": "deepseek-flash",
            "max_tokens": 100,
            "messages": [
                {"role": "user", "content": "请用一句话打招呼"}
            ],
            "stream": true
        }))
        .send()
        .await
        .unwrap();

    let status = resp.status();
    println!("DeepSeek streaming HTTP status: {}", status);
    assert!(status.is_success(), "HTTP status failed: {}", status);

    let sse_body = resp.text().await.unwrap();
    println!("DeepSeek SSE Body received:\n{}", sse_body);

    assert!(sse_body.contains("event: message_start"), "Missing message_start");
    assert!(sse_body.contains("event: content_block_start"), "Missing content_block_start");
    assert!(sse_body.contains("event: content_block_stop"), "Missing content_block_stop");
    assert!(sse_body.contains("event: message_delta"), "Missing message_delta");
    assert!(sse_body.contains("event: message_stop"), "Missing message_stop");
    println!("Live DeepSeek real streaming test PASSED!");
}

#[tokio::test]
async fn test_live_real_deepseek_claude_code_tool_use_stream() {
    let home = dirs::home_dir().expect("no home dir");
    let db_path = home.join(".pivot").join("pivot.db");
    if !db_path.exists() {
        println!("No ~/.pivot/pivot.db found, skipping.");
        return;
    }
    let db = Database::new(db_path).unwrap();
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
    let port = 16939;
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    tokio::spawn(async move {
        axum::serve(listener, gw_app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(200)).await;
    let client = reqwest::Client::new();

    println!("Testing live DeepSeek with Claude Code tool_use streaming request...");
    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/messages", port))
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": "deepseek-flash",
            "max_tokens": 150,
            "messages": [
                {"role": "user", "content": "请务必调用 Bash 工具执行命令 'ls -la'，不要直接回答文本"}
            ],
            "tools": [{
                "name": "Bash",
                "description": "Execute a bash shell command",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "The command to run" }
                    },
                    "required": ["command"]
                }
            }],
            "stream": true
        }))
        .send()
        .await
        .unwrap();

    let status = resp.status();
    println!("DeepSeek tool_use streaming HTTP status: {}", status);
    assert!(status.is_success(), "HTTP status failed: {}", status);

    let sse_body = resp.text().await.unwrap();
    println!("DeepSeek tool_use SSE Body received:\n{}", sse_body);

    assert!(sse_body.contains("event: message_start"), "Missing message_start");
    assert!(sse_body.contains("event: content_block_start"), "Missing content_block_start");
    assert!(sse_body.contains(r#""type":"tool_use""#), "Missing tool_use block");
    assert!(sse_body.contains("event: content_block_delta"), "Missing content_block_delta");
    assert!(sse_body.contains("event: content_block_stop"), "Missing content_block_stop");
    assert!(sse_body.contains("event: message_delta"), "Missing message_delta");
    assert!(sse_body.contains(r#""stop_reason":"tool_use""#), "Missing stop_reason: tool_use");
    assert!(sse_body.contains("event: message_stop"), "Missing message_stop");
    println!("Live DeepSeek tool_use real streaming test PASSED!");
}
