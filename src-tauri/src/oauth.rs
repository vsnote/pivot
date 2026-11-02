use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub const ANTIGRAVITY_CLIENT_ID: &str =
    "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
pub const ANTIGRAVITY_CLIENT_SECRET: &str = "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";
pub const ANTIGRAVITY_REDIRECT_URI: &str = "http://localhost:51121/oauth-callback";
pub const OAUTH_PORT: u16 = 51121;

pub const OPENAI_CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const OPENAI_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
pub const OPENAI_OAUTH_PORT: u16 = 1455;

pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

pub fn generate_pkce() -> PkcePair {
    // 随机 64 字节转 base64url 无填充
    let raw = format!(
        "{}{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let hash = hasher.finalize();
    let challenge = URL_SAFE_NO_PAD.encode(hash);
    PkcePair {
        verifier: raw,
        challenge,
    }
}

fn url_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

fn url_decode(s: &str) -> String {
    let mut res = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                res.push(val);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            res.push(b' ');
            i += 1;
            continue;
        }
        res.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&res).to_string()
}

pub fn build_auth_url(pkce: &PkcePair, state: &str) -> String {
    let scopes = [
        "https://www.googleapis.com/auth/cloud-platform",
        "https://www.googleapis.com/auth/userinfo.email",
        "openid",
    ]
    .join(" ");

    format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent&state={}",
        url_encode(ANTIGRAVITY_CLIENT_ID),
        url_encode(ANTIGRAVITY_REDIRECT_URI),
        url_encode(&scopes),
        url_encode(&pkce.challenge),
        url_encode(state),
    )
}

pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
    pub email: Option<String>,
}

/// 监听本地 51121 端口等待 OAuth 回调，超时时间默认 120 秒
pub async fn listen_for_code(expected_state: &str, timeout_secs: u64) -> Result<String, String> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", OAUTH_PORT))
        .await
        .map_err(|e| format!("无法绑定本地 OAuth 端口 {}: {}", OAUTH_PORT, e))?;

    let receive_fut = async {
        loop {
            let (mut socket, _) = listener
                .accept()
                .await
                .map_err(|e| format!("接收 OAuth 连接失败: {}", e))?;

            let mut buf = [0u8; 4096];
            let n = socket
                .read(&mut buf)
                .await
                .map_err(|e| format!("读取请求失败: {}", e))?;
            let request = String::from_utf8_lossy(&buf[..n]);

            // 解析 GET 请求中的 query 参数
            if let Some(first_line) = request.lines().next() {
                let parts: Vec<&str> = first_line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let path = parts[1];
                    if path.starts_with("/oauth-callback") {
                        let query = path.split_once('?').map(|x| x.1).unwrap_or("");
                        let mut code = None;
                        let mut state = None;

                        for pair in query.split('&') {
                            if let Some((k, v)) = pair.split_once('=') {
                                if k == "code" {
                                    code = Some(url_decode(v));
                                } else if k == "state" {
                                    state = Some(url_decode(v));
                                }
                            }
                        }

                        let html_body = r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Pivot 授权成功</title></head>
<body style="font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,Helvetica,Arial,sans-serif;display:flex;flex-direction:column;align-items:center;justify-content:center;height:80vh;background:#fafafa;color:#18181b;">
  <div style="background:#fff;padding:40px;border-radius:12px;box-shadow:0 4px 20px rgba(0,0,0,0.08);text-align:center;max-width:400px;">
    <div style="width:48px;height:48px;background:#10b981;border-radius:50%;display:flex;align-items:center;justify-content:center;margin:0 auto 16px;color:#fff;font-size:24px;">✓</div>
    <h2 style="margin:0 0 8px;font-size:20px;font-weight:600;">授权成功</h2>
    <p style="margin:0 0 20px;color:#71717a;font-size:14px;">您的 Google Antigravity 订阅账号已成功连接至 Pivot，现在可以关闭此标签页。</p>
  </div>
</body>
</html>"#;

                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            html_body.len(),
                            html_body
                        );

                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = socket.flush().await;

                        if let Some(s) = state {
                            if s != expected_state {
                                return Err("OAuth State 校验失败，可能存在 CSRF 风险".to_string());
                            }
                        }

                        if let Some(c) = code {
                            return Ok(c);
                        } else {
                            return Err("OAuth 回调中未找到授权码 code".to_string());
                        }
                    }
                }
            }
        }
    };

    tokio::time::timeout(Duration::from_secs(timeout_secs), receive_fut)
        .await
        .map_err(|_| "OAuth 登录超时，请在浏览器中完成授权并重试".to_string())?
}

/// 用授权码向 Google 换取 Token 并获取邮箱
pub async fn exchange_code_for_tokens(
    client: &reqwest::Client,
    code: &str,
    pkce_verifier: &str,
) -> Result<TokenResponse, String> {
    let params = [
        ("client_id", ANTIGRAVITY_CLIENT_ID),
        ("client_secret", ANTIGRAVITY_CLIENT_SECRET),
        ("code", code),
        ("code_verifier", pkce_verifier),
        ("grant_type", "authorization_code"),
        ("redirect_uri", ANTIGRAVITY_REDIRECT_URI),
    ];

    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("请求 Google Token 端点失败: {}", e))?;

    if !resp.status().is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        return Err(format!("换取 Token 失败: {}", err_body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 Token 响应失败: {}", e))?;

    let access_token = json["access_token"]
        .as_str()
        .ok_or_else(|| "响应中缺少 access_token".to_string())?
        .to_string();

    let refresh_token = json["refresh_token"].as_str().map(str::to_string);
    let expires_in = json["expires_in"].as_i64().unwrap_or(3600);

    // 获取用户邮箱
    let mut email = None;
    if let Ok(user_resp) = client
        .get("https://www.googleapis.com/oauth2/v1/userinfo?alt=json")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
    {
        if let Ok(user_json) = user_resp.json::<serde_json::Value>().await {
            email = user_json["email"].as_str().map(str::to_string);
        }
    }

    Ok(TokenResponse {
        access_token,
        refresh_token,
        expires_in,
        email,
    })
}

/// 刷新 Token
pub async fn refresh_antigravity_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<(String, i64), String> {
    let params = [
        ("client_id", ANTIGRAVITY_CLIENT_ID),
        ("client_secret", ANTIGRAVITY_CLIENT_SECRET),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];

    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("刷新 Google Token 请求失败: {}", e))?;

    if !resp.status().is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        return Err(format!("刷新 Token 失败: {}", err_body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析刷新 Token 响应失败: {}", e))?;

    let access_token = json["access_token"]
        .as_str()
        .ok_or_else(|| "刷新响应缺少 access_token".to_string())?
        .to_string();

    let expires_in = json["expires_in"].as_i64().unwrap_or(3600);

    Ok((access_token, expires_in))
}

/// 生成 OpenAI ChatGPT Codex OAuth 授权 URL
pub fn build_chatgpt_auth_url(pkce: &PkcePair, state: &str) -> String {
    let scopes = "openid profile email offline_access api.connectors.read api.connectors.invoke";

    format!(
        "https://auth.openai.com/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        url_encode(OPENAI_CODEX_CLIENT_ID),
        url_encode(OPENAI_REDIRECT_URI),
        url_encode(scopes),
        url_encode(&pkce.challenge),
        url_encode(state),
    )
}

/// 监听本地 1455 端口等待 OpenAI OAuth 回调，超时时间默认 120 秒
pub async fn listen_for_chatgpt_code(expected_state: &str, timeout_secs: u64) -> Result<String, String> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", OPENAI_OAUTH_PORT))
        .await
        .map_err(|e| format!("无法绑定本地 OpenAI 回调端口 {}: {}", OPENAI_OAUTH_PORT, e))?;

    let receive_fut = async {
        loop {
            let (mut socket, _) = listener
                .accept()
                .await
                .map_err(|e| format!("接收 OAuth 连接失败: {}", e))?;

            let mut buf = [0u8; 4096];
            let n = socket
                .read(&mut buf)
                .await
                .map_err(|e| format!("读取请求失败: {}", e))?;
            let request = String::from_utf8_lossy(&buf[..n]);

            if let Some(first_line) = request.lines().next() {
                let parts: Vec<&str> = first_line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let path = parts[1];
                    if path.starts_with("/auth/callback") {
                        let query = path.split_once('?').map(|x| x.1).unwrap_or("");
                        let mut code = None;
                        let mut state = None;

                        for pair in query.split('&') {
                            if let Some((k, v)) = pair.split_once('=') {
                                if k == "code" {
                                    code = Some(url_decode(v));
                                } else if k == "state" {
                                    state = Some(url_decode(v));
                                }
                            }
                        }

                        let html_body = r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Pivot 授权成功</title></head>
<body style="font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,Helvetica,Arial,sans-serif;display:flex;flex-direction:column;align-items:center;justify-content:center;height:80vh;background:#fafafa;color:#18181b;">
  <div style="background:#fff;padding:40px;border-radius:12px;box-shadow:0 4px 20px rgba(0,0,0,0.08);text-align:center;max-width:400px;">
    <div style="width:48px;height:48px;background:#10b981;border-radius:50%;display:flex;align-items:center;justify-content:center;margin:0 auto 16px;color:#fff;font-size:24px;">✓</div>
    <h2 style="margin:0 0 8px;font-size:20px;font-weight:600;">授权成功</h2>
    <p style="margin:0 0 20px;color:#71717a;font-size:14px;">您的 OpenAI ChatGPT 订阅账号已成功连接至 Pivot，现在可以关闭此标签页。</p>
  </div>
</body>
</html>"#;

                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            html_body.len(),
                            html_body
                        );

                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = socket.flush().await;

                        if let Some(s) = state {
                            if s != expected_state {
                                return Err("OAuth State 校验失败，可能存在 CSRF 风险".to_string());
                            }
                        }

                        if let Some(c) = code {
                            return Ok(c);
                        } else {
                            return Err("OAuth 回调中未找到授权码 code".to_string());
                        }
                    }
                }
            }
        }
    };

    tokio::time::timeout(Duration::from_secs(timeout_secs), receive_fut)
        .await
        .map_err(|_| "OAuth 登录超时，请在浏览器中完成授权并重试".to_string())?
}

/// 用授权码向 OpenAI 换取 Token 并获取邮箱
pub async fn exchange_chatgpt_code(
    client: &reqwest::Client,
    code: &str,
    pkce_verifier: &str,
) -> Result<TokenResponse, String> {
    let params = [
        ("client_id", OPENAI_CODEX_CLIENT_ID),
        ("code", code),
        ("code_verifier", pkce_verifier),
        ("grant_type", "authorization_code"),
        ("redirect_uri", OPENAI_REDIRECT_URI),
    ];

    let resp = client
        .post("https://auth.openai.com/oauth/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("请求 OpenAI Token 端点失败: {}", e))?;

    if !resp.status().is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        return Err(format!("换取 OpenAI Token 失败: {}", err_body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 Token 响应失败: {}", e))?;

    let access_token = json["access_token"]
        .as_str()
        .ok_or_else(|| "响应中缺少 access_token".to_string())?
        .to_string();

    let refresh_token = json["refresh_token"].as_str().map(str::to_string);
    let expires_in = json["expires_in"].as_i64().unwrap_or(86400 * 7);

    // 获取用户邮箱
    let mut email = None;
    if let Ok(user_resp) = client
        .get("https://chatgpt.com/backend-api/me")
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", "codex-cli/0.154.0")
        .send()
        .await
    {
        if let Ok(user_json) = user_resp.json::<serde_json::Value>().await {
            email = user_json["email"].as_str().map(str::to_string);
        }
    }

    Ok(TokenResponse {
        access_token,
        refresh_token,
        expires_in,
        email,
    })
}

/// 刷新 OpenAI Token
pub async fn refresh_chatgpt_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<(String, i64), String> {
    let params = [
        ("client_id", OPENAI_CODEX_CLIENT_ID),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];

    let resp = client
        .post("https://auth.openai.com/oauth/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("刷新 OpenAI Token 请求失败: {}", e))?;

    if !resp.status().is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        return Err(format!("刷新 OpenAI Token 失败: {}", err_body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析刷新 Token 响应失败: {}", e))?;

    let access_token = json["access_token"]
        .as_str()
        .ok_or_else(|| "刷新响应缺少 access_token".to_string())?
        .to_string();

    let expires_in = json["expires_in"].as_i64().unwrap_or(86400 * 7);

    Ok((access_token, expires_in))
}

/// 读取本地已登录的 ~/.codex/auth.json 凭据
pub async fn import_local_codex_account(client: &reqwest::Client) -> Result<TokenResponse, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法获取用户主目录".to_string())?;
    let auth_file = home.join(".codex").join("auth.json");
    if !auth_file.exists() {
        return Err("未在 ~/.codex/auth.json 找到本地 Codex 凭证，请先在终端运行 codex login 或使用浏览器登录".to_string());
    }

    let content = std::fs::read_to_string(&auth_file)
        .map_err(|e| format!("读取 ~/.codex/auth.json 失败: {}", e))?;
    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("解析 ~/.codex/auth.json 失败: {}", e))?;

    let tokens = json.get("tokens").ok_or_else(|| "凭证文件中未找到 tokens 字段".to_string())?;
    let access_token = tokens["access_token"]
        .as_str()
        .ok_or_else(|| "缺少 access_token".to_string())?
        .to_string();
    let refresh_token = tokens["refresh_token"].as_str().map(str::to_string);

    // 获取用户邮箱
    let mut email = None;
    if let Ok(user_resp) = client
        .get("https://chatgpt.com/backend-api/me")
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", "codex-cli/0.154.0")
        .send()
        .await
    {
        if let Ok(user_json) = user_resp.json::<serde_json::Value>().await {
            email = user_json["email"].as_str().map(str::to_string);
        }
    }

    Ok(TokenResponse {
        access_token,
        refresh_token,
        expires_in: 86400 * 7,
        email,
    })
}

/// 获取 ChatGPT 可用模型列表
pub async fn fetch_chatgpt_models(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<Vec<String>, String> {
    let resp = client
        .get("https://chatgpt.com/backend-api/codex/models?client_version=0.154.0")
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", "codex-cli/0.154.0")
        .send()
        .await;

    let fallback_models = vec![
        "gpt-5.6-sol".to_string(),
        "gpt-6-astra".to_string(),
        "gpt-5.6-terra".to_string(),
        "gpt-5.6-luna".to_string(),
        "gpt-5.5".to_string(),
        "gpt-5.4".to_string(),
        "gpt-4o".to_string(),
    ];

    match resp {
        Ok(res) if res.status().is_success() => {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(arr) = json["models"].as_array() {
                    let mut models = Vec::new();
                    for m in arr {
                        if let Some(slug) = m["slug"].as_str() {
                            models.push(slug.to_string());
                        }
                    }
                    if !models.is_empty() {
                        return Ok(models);
                    }
                }
            }
            Ok(fallback_models)
        }
        _ => Ok(fallback_models),
    }
}

/// 解析 ChatGPT / Codex 官方 /wham/usage 额度响应
pub fn parse_chatgpt_usage_json(json: &serde_json::Value) -> Vec<QuotaBucket> {
    let mut buckets = Vec::new();

    let plan_label = match json["plan_type"].as_str() {
        Some("pro") => "Pro",
        Some("prolite") => "Pro Lite",
        Some("plus") => "Plus",
        Some("team") => "Team",
        Some(other) => other,
        None => "",
    };

    let rate_limit = &json["rate_limit"];
    let limit_reached = rate_limit["limit_reached"].as_bool().unwrap_or(false);

    let format_window_label = |win: &serde_json::Value, default_suffix: &str| -> String {
        let secs = win["limit_window_seconds"].as_i64().unwrap_or(0);
        let cycle_desc = if secs > 0 {
            if secs <= 10800 {
                "3小时配额".to_string()
            } else if secs <= 18000 {
                "5小时配额".to_string()
            } else if secs <= 86400 {
                "日周期配额".to_string()
            } else if secs == 604800 {
                "周周期配额".to_string()
            } else {
                format!("{}天周期配额", (secs + 43200) / 86400)
            }
        } else {
            default_suffix.to_string()
        };

        if plan_label.is_empty() {
            cycle_desc
        } else {
            format!("{} {}", plan_label, cycle_desc)
        }
    };

    let parse_window = |win: &serde_json::Value, id: &str, default_suffix: &str| -> Option<QuotaBucket> {
        if win.is_null() || !win.is_object() {
            return None;
        }
        let used_pct = win["used_percent"].as_f64()?;
        let remaining_fraction = ((100.0 - used_pct) / 100.0).clamp(0.0, 1.0);
        let is_exhausted = remaining_fraction <= 0.005 || limit_reached;

        let label = format_window_label(win, default_suffix);

        let reset_time = if let Some(ts) = win["reset_at"].as_i64() {
            chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.to_rfc3339())
        } else if let Some(secs) = win["reset_after_seconds"].as_i64() {
            Some((chrono::Utc::now() + chrono::Duration::seconds(secs)).to_rfc3339())
        } else {
            None
        };

        Some(QuotaBucket {
            id: id.to_string(),
            label,
            remaining_fraction,
            reset_time,
            is_exhausted,
        })
    };

    if let Some(primary) = parse_window(&rate_limit["primary_window"], "chatgpt_primary_window", "主配额") {
        buckets.push(primary);
    }

    if let Some(secondary) = parse_window(&rate_limit["secondary_window"], "chatgpt_secondary_window", "短周期配额") {
        buckets.push(secondary);
    }

    // 重置卡支持 (rate_limit_reset_credits)
    if let Some(credits) = json.get("rate_limit_reset_credits") {
        let available = credits["available_count"].as_u64().unwrap_or(0);
        if available > 0 {
            buckets.push(QuotaBucket {
                id: "chatgpt_reset_credits".to_string(),
                label: format!("限流重置卡 ({} 次可用)", available),
                remaining_fraction: 1.0,
                reset_time: None,
                is_exhausted: false,
            });
        }
    }

    buckets
}

/// 获取 ChatGPT 订阅状态与配额
pub async fn fetch_chatgpt_quota(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<Vec<QuotaBucket>, String> {
    // 1. 尝试直接请求 Codex 官方额度接口 /wham/usage 获取真实可用额度
    let usage_resp = client
        .get("https://chatgpt.com/backend-api/wham/usage")
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", "codex-cli/0.154.0")
        .header("accept", "application/json")
        .send()
        .await;

    if let Ok(res) = usage_resp {
        if res.status().is_success() {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                let buckets = parse_chatgpt_usage_json(&json);
                if !buckets.is_empty() {
                    return Ok(buckets);
                }
            }
        }
    }

    // 2. 兜底回退：通过账号状态检查接口读取套餐类型
    let resp = client
        .get("https://chatgpt.com/backend-api/accounts/check/v4-2023-04-27")
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", "codex-cli/0.154.0")
        .send()
        .await;

    let mut plan_name = "ChatGPT 订阅有效".to_string();
    if let Ok(res) = resp {
        if res.status().is_success() {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(accs) = json["accounts"].as_object() {
                    if let Some(first) = accs.values().next() {
                        if let Some(pt) = first["account"]["plan_type"].as_str() {
                            plan_name = match pt {
                                "pro" => "ChatGPT Pro 订阅 (有效)".to_string(),
                                "prolite" => "ChatGPT Pro Lite 订阅 (有效)".to_string(),
                                "plus" => "ChatGPT Plus 订阅 (有效)".to_string(),
                                "team" => "ChatGPT Team 订阅 (有效)".to_string(),
                                other => format!("ChatGPT {} 订阅 (有效)", other),
                            };
                        }
                    }
                }
            }
        }
    }

    Ok(vec![QuotaBucket {
        id: "chatgpt_subscription_status".to_string(),
        label: plan_name,
        remaining_fraction: 1.0,
        reset_time: None,
        is_exhausted: false,
    }])
}

/// 为指定 provider 获取一个可用的订阅账号（自动轮询、检查冷却、自动续期 Token）
pub async fn acquire_subscription_account(
    db: &crate::db::Database,
    http_client: &reqwest::Client,
    provider_id: &str,
) -> Result<crate::db::SubscriptionAccount, (axum::http::StatusCode, String)> {
    let accounts = db.get_subscription_accounts(provider_id).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("读取订阅账号失败: {}", e),
        )
    })?;

    let now_sec = chrono::Utc::now().timestamp();
    let available: Vec<_> = accounts
        .into_iter()
        .filter(|a| a.enabled && a.cooldown_until <= now_sec)
        .collect();

    if available.is_empty() {
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "当前订阅账号池中没有可用账号（全部在冷却中或尚未绑定账号）".to_string(),
        ));
    }

    static SUB_RR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let idx = SUB_RR.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % available.len();
    let mut account = available[idx].clone();

    // 检查是否临近过期（剩余不足 5 分钟），自动刷新
    if account.expires_at - now_sec < 300 {
        if let Some(ref refresh_token) = account.refresh_token {
            let refresh_res = if account.account_type == "chatgpt" {
                refresh_chatgpt_token(http_client, refresh_token).await
            } else {
                refresh_antigravity_token(http_client, refresh_token).await
            };
            match refresh_res {
                Ok((new_token, expires_in)) => {
                    let new_expires_at = now_sec + expires_in;
                    let _ = db.update_account_tokens(
                        &account.id,
                        &new_token,
                        account.refresh_token.as_deref(),
                        new_expires_at,
                    );
                    account.access_token = new_token;
                    account.expires_at = new_expires_at;
                }
                Err(e) => {
                    tracing::warn!("刷新订阅账号 {} Token 失败: {}", account.id, e);
                    if account.expires_at <= now_sec {
                        return Err((
                            axum::http::StatusCode::UNAUTHORIZED,
                            format!("订阅账号 Token 已过期且刷新失败: {}", e),
                        ));
                    }
                }
            }
        }
    }

    Ok(account)
}

/// 标记账号进入冷却（如遇到 429 速率限制，默认冷却 5 分钟）
pub fn mark_account_cooldown(db: &crate::db::Database, account_id: &str, cooldown_seconds: i64) {
    let until = chrono::Utc::now().timestamp() + cooldown_seconds;
    let _ = db.set_account_cooldown(account_id, until);
}

/// 向 Google Antigravity / Cloud Code 请求当前账号可用的模型列表
pub async fn fetch_antigravity_models(
    client: &reqwest::Client,
    access_token: &str,
    base_url: &str,
) -> Result<Vec<String>, String> {
    let base = base_url.trim_end_matches('/');
    let endpoint = format!("{}/v1internal:fetchAvailableModels", base);

    let resp = client
        .post(&endpoint)
        .header("authorization", format!("Bearer {}", access_token))
        .header(
            "user-agent",
            "antigravity/cli/1.4.6 (aidev_client; os_type=darwin; arch=arm64; cl=964361259; auth_method=consumer)",
        )
        .header("content-type", "application/json")
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| format!("请求可用模型失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_body = resp.text().await.unwrap_or_default();
        return Err(format!("上游返回错误 ({}): {}", status, err_body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析模型响应失败: {}", e))?;

    let mut models = Vec::new();
    if let Some(map) = json["models"].as_object() {
        for (id, val) in map {
            let id_lower = id.to_lowercase();
            // 过滤内部补全与禁用模型
            if id_lower.starts_with("chat_")
                || id_lower.starts_with("tab_")
                || id_lower.ends_with("-disabled")
            {
                continue;
            }
            // 过滤没有有效显示名称的内部占位模型
            let display_name = val["displayName"].as_str().unwrap_or("");
            if display_name.is_empty() || display_name == "undefined" {
                continue;
            }
            models.push(id.clone());
        }
    } else if let Some(arr) = json["models"].as_array() {
        for m in arr {
            let state = m["state"].as_str().unwrap_or("AVAILABLE");
            if state == "AVAILABLE" {
                if let Some(id) = m["modelId"]
                    .as_str()
                    .or_else(|| m["name"].as_str())
                    .or_else(|| m["id"].as_str())
                {
                    let id_lower = id.to_lowercase();
                    if !id_lower.starts_with("chat_")
                        && !id_lower.starts_with("tab_")
                        && !id_lower.ends_with("-disabled")
                    {
                        models.push(id.to_string());
                    }
                }
            }
        }
    }

    if models.is_empty() {
        if let Some(arr) = json["data"].as_array() {
            for m in arr {
                if let Some(id) = m["id"].as_str() {
                    let id_lower = id.to_lowercase();
                    if !id_lower.starts_with("chat_")
                        && !id_lower.starts_with("tab_")
                        && !id_lower.ends_with("-disabled")
                    {
                        models.push(id.to_string());
                    }
                }
            }
        }
    }

    if models.is_empty() {
        return Err("上游未返回任何可用模型".to_string());
    }

    // 智能排序：优先将 Claude 与主流 Gemini 模型置于前列
    models.sort_by(|a, b| {
        let score = |id: &str| {
            let s = id.to_lowercase();
            if s.contains("claude-sonnet") {
                0
            } else if s.contains("claude-opus") {
                1
            } else if s.contains("gemini-2.5-pro") {
                2
            } else if s.contains("gemini-3-flash") {
                3
            } else if s.contains("gemini-2.5-flash-thinking") {
                4
            } else if s.contains("gemini-2.5-flash") {
                5
            } else if s.contains("gemini-3.1-flash-lite") {
                6
            } else if s.contains("gpt-oss") {
                7
            } else if s.contains("claude") {
                10
            } else if s.contains("gemini") {
                20
            } else {
                30
            }
        };
        score(a).cmp(&score(b))
    });

    Ok(models)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaBucket {
    pub id: String,
    pub label: String,
    pub remaining_fraction: f64,
    pub reset_time: Option<String>,
    pub is_exhausted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountQuota {
    pub account_id: String,
    pub updated_at: i64,
    pub buckets: Vec<QuotaBucket>,
    pub error: Option<String>,
}

/// 解析 Google Antigravity / Cloud Code 返回的额度 JSON
pub fn parse_antigravity_quota_json(json: &serde_json::Value) -> Vec<QuotaBucket> {
    let mut result = Vec::new();

    // 1. 优先解析官方标准分组汇总结构 (v1internal:retrieveUserQuotaSummary)
    if let Some(groups) = json["groups"].as_array() {
        for g in groups {
            let group_name = g["displayName"].as_str().unwrap_or("").to_lowercase();
            let is_claude_group = group_name.contains("claude") || group_name.contains("3p");
            let is_gemini_group = group_name.contains("gemini");

            if let Some(buckets) = g["buckets"].as_array() {
                for b in buckets {
                    let bucket_id = b["bucketId"].as_str().unwrap_or("").to_lowercase();
                    let window = b["window"].as_str().unwrap_or("").to_lowercase();
                    let is_5h = window == "5h" || bucket_id.contains("5h");
                    let is_weekly = window == "weekly" || bucket_id.contains("weekly") || bucket_id.contains("week");

                    let label = if is_claude_group {
                        if is_5h {
                            "Claude / GPT 5小时".to_string()
                        } else if is_weekly {
                            "Claude / GPT 周配额".to_string()
                        } else {
                            "Claude / GPT 配额".to_string()
                        }
                    } else if is_gemini_group {
                        if is_5h {
                            "Gemini 5小时".to_string()
                        } else if is_weekly {
                            "Gemini 周配额".to_string()
                        } else {
                            "Gemini 配额".to_string()
                        }
                    } else {
                        b["displayName"].as_str().unwrap_or(&bucket_id).to_string()
                    };

                    let remaining = b["remainingFraction"]
                        .as_f64()
                        .unwrap_or(1.0)
                        .clamp(0.0, 1.0);

                    let reset_time = b["resetTime"].as_str().map(|s| s.to_string());
                    let is_exhausted = remaining <= 0.005;

                    result.push(QuotaBucket {
                        id: b["bucketId"].as_str().unwrap_or("bucket").to_string(),
                        label,
                        remaining_fraction: remaining,
                        reset_time,
                        is_exhausted,
                    });
                }
            }
        }

        if !result.is_empty() {
            // 排序：Claude 5h -> Gemini 5h -> Claude 周配额 -> Gemini 周配额
            result.sort_by_key(|b| {
                let id = b.id.to_lowercase();
                if id.contains("3p-5h") {
                    1
                } else if id.contains("gemini-5h") {
                    2
                } else if id.contains("3p-weekly") {
                    3
                } else if id.contains("gemini-weekly") {
                    4
                } else {
                    5
                }
            });
            return result;
        }
    }

    // 2. 兜底解析平铺桶 (v1internal:retrieveUserQuota)：按家族聚合，过滤内部补全模型，杜绝重复项
    let mut raw_items = Vec::new();
    if let Some(arr) = json["buckets"].as_array() {
        raw_items.extend(arr.iter().cloned());
    }
    if let Some(arr) = json["quotaInfo"].as_array() {
        raw_items.extend(arr.iter().cloned());
    }

    let mut claude_bucket: Option<QuotaBucket> = None;
    let mut gemini_bucket: Option<QuotaBucket> = None;
    let mut gpt_bucket: Option<QuotaBucket> = None;

    for item in raw_items {
        let id = item["modelId"]
            .as_str()
            .or_else(|| item["bucketId"].as_str())
            .unwrap_or("")
            .to_string();

        let id_lower = id.to_lowercase();
        // 过滤内部代码补全标识
        if id_lower.starts_with("chat_") || id_lower.starts_with("tab_") || id_lower.ends_with("-disabled") {
            continue;
        }

        let remaining = item["remainingFraction"]
            .as_f64()
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        let reset_time = item["resetTime"].as_str().map(|s| s.to_string());
        let is_exhausted = remaining <= 0.005;

        let entry = QuotaBucket {
            id: id.clone(),
            label: String::new(),
            remaining_fraction: remaining,
            reset_time,
            is_exhausted,
        };

        if id_lower.contains("claude") {
            if let Some(ref mut c) = claude_bucket {
                if remaining < c.remaining_fraction {
                    c.remaining_fraction = remaining;
                    c.reset_time = entry.reset_time;
                    c.is_exhausted = is_exhausted;
                }
            } else {
                let mut c = entry;
                c.label = "Claude 额度".to_string();
                claude_bucket = Some(c);
            }
        } else if id_lower.contains("gemini") {
            if let Some(ref mut g) = gemini_bucket {
                if remaining < g.remaining_fraction {
                    g.remaining_fraction = remaining;
                    g.reset_time = entry.reset_time;
                    g.is_exhausted = is_exhausted;
                }
            } else {
                let mut g = entry;
                g.label = "Gemini 额度".to_string();
                gemini_bucket = Some(g);
            }
        } else if id_lower.contains("gpt") {
            if let Some(ref mut p) = gpt_bucket {
                if remaining < p.remaining_fraction {
                    p.remaining_fraction = remaining;
                    p.reset_time = entry.reset_time;
                    p.is_exhausted = is_exhausted;
                }
            } else {
                let mut p = entry;
                p.label = "GPT-OSS 额度".to_string();
                gpt_bucket = Some(p);
            }
        }
    }

    if let Some(c) = claude_bucket {
        result.push(c);
    }
    if let Some(g) = gemini_bucket {
        result.push(g);
    }
    if let Some(p) = gpt_bucket {
        result.push(p);
    }

    result
}

/// 向 Google Antigravity / Cloud Code 请求当前账号的额度信息
pub async fn fetch_antigravity_quota(
    client: &reqwest::Client,
    access_token: &str,
    base_url: &str,
) -> Result<Vec<QuotaBucket>, String> {
    let clean_base = base_url.trim_end_matches('/');

    let mut bases = vec![clean_base.to_string()];
    if clean_base != "https://cloudcode-pa.googleapis.com" {
        bases.push("https://cloudcode-pa.googleapis.com".to_string());
    }
    if clean_base != "https://daily-cloudcode-pa.googleapis.com" {
        bases.push("https://daily-cloudcode-pa.googleapis.com".to_string());
    }

    let mut last_err = String::new();

    for b in bases {
        // 优先请求结构化汇总端点 retrieveUserQuotaSummary，获取干净的 5 小时与周配额
        let endpoints = vec![
            format!("{}/v1internal:retrieveUserQuotaSummary", b),
            format!("{}/v1internal:retrieveUserQuota", b),
        ];

        for endpoint in endpoints {
            let resp = client
                .post(&endpoint)
                .header("authorization", format!("Bearer {}", access_token))
                .header(
                    "user-agent",
                    "antigravity/cli/1.4.6 (aidev_client; os_type=darwin; arch=arm64; cl=964361259; auth_method=consumer)",
                )
                .header("content-type", "application/json")
                .json(&serde_json::json!({}))
                .send()
                .await;

            match resp {
                Ok(res) if res.status().is_success() => {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        let buckets = parse_antigravity_quota_json(&json);
                        if !buckets.is_empty() {
                            return Ok(buckets);
                        }
                    }
                }
                Ok(res) => {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    last_err = format!("上游返回错误 ({}): {}", status, body);
                }
                Err(e) => {
                    last_err = format!("网络连接失败: {}", e);
                }
            }
        }
    }

    Err(if last_err.is_empty() {
        "未能获取到有效的额度信息".to_string()
    } else {
        last_err
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_antigravity_quota_groups_real() {
        let json = serde_json::json!({
            "groups": [
                {
                    "displayName": "Gemini Models",
                    "buckets": [
                        {
                            "bucketId": "gemini-weekly",
                            "window": "weekly",
                            "resetTime": "2026-10-27T10:03:39Z",
                            "remainingFraction": 1.0
                        },
                        {
                            "bucketId": "gemini-5h",
                            "window": "5h",
                            "resetTime": "2026-10-24T20:31:22Z",
                            "remainingFraction": 0.8
                        }
                    ]
                },
                {
                    "displayName": "Claude and GPT models",
                    "buckets": [
                        {
                            "bucketId": "3p-weekly",
                            "window": "weekly",
                            "resetTime": "2026-10-31T15:31:22Z",
                            "remainingFraction": 0.95
                        },
                        {
                            "bucketId": "3p-5h",
                            "window": "5h",
                            "resetTime": "2026-10-24T20:31:22Z",
                            "remainingFraction": 0.5
                        }
                    ]
                }
            ]
        });

        let buckets = parse_antigravity_quota_json(&json);
        assert_eq!(buckets.len(), 4);
        // 排序规则: Claude 5h, Gemini 5h, Claude weekly, Gemini weekly
        assert_eq!(buckets[0].id, "3p-5h");
        assert_eq!(buckets[0].label, "Claude / GPT 5小时");
        assert_eq!(buckets[0].remaining_fraction, 0.5);

        assert_eq!(buckets[1].id, "gemini-5h");
        assert_eq!(buckets[1].label, "Gemini 5小时");
        assert_eq!(buckets[1].remaining_fraction, 0.8);

        assert_eq!(buckets[2].id, "3p-weekly");
        assert_eq!(buckets[2].label, "Claude / GPT 周配额");
        assert_eq!(buckets[2].remaining_fraction, 0.95);

        assert_eq!(buckets[3].id, "gemini-weekly");
        assert_eq!(buckets[3].label, "Gemini 周配额");
        assert_eq!(buckets[3].remaining_fraction, 1.0);
    }

    #[test]
    fn test_parse_antigravity_quota_flat_buckets_dedup() {
        let json = serde_json::json!({
            "buckets": [
                { "modelId": "chat_20706", "remainingFraction": 1.0 },
                { "modelId": "tab_flash_lite_preview", "remainingFraction": 1.0 },
                { "modelId": "claude-sonnet-4-6", "remainingFraction": 0.6, "resetTime": "2026-10-24T20:30:26Z" },
                { "modelId": "claude-opus-4-6-thinking", "remainingFraction": 0.4, "resetTime": "2026-10-24T20:30:26Z" },
                { "modelId": "gemini-2.5-pro", "remainingFraction": 0.9, "resetTime": "2026-10-24T20:30:26Z" },
                { "modelId": "gemini-2.5-flash", "remainingFraction": 0.85, "resetTime": "2026-10-24T20:30:26Z" },
                { "modelId": "gpt-oss-120b-medium", "remainingFraction": 1.0, "resetTime": "2026-10-24T20:30:26Z" }
            ]
        });

        let buckets = parse_antigravity_quota_json(&json);
        // 应该聚合为 Claude, Gemini, GPT 三个桶，过滤掉 chat_ 和 tab_，且取同家族最低余量
        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].label, "Claude 额度");
        assert_eq!(buckets[0].remaining_fraction, 0.4);

        assert_eq!(buckets[1].label, "Gemini 额度");
        assert_eq!(buckets[1].remaining_fraction, 0.85);

        assert_eq!(buckets[2].label, "GPT-OSS 额度");
        assert_eq!(buckets[2].remaining_fraction, 1.0);
    }

    #[test]
    fn test_parse_chatgpt_usage_json_real() {
        let json = serde_json::json!({
            "user_id": "user-test",
            "email": "test@example.com",
            "plan_type": "prolite",
            "rate_limit": {
                "allowed": true,
                "limit_reached": false,
                "primary_window": {
                    "used_percent": 97,
                    "limit_window_seconds": 604800,
                    "reset_after_seconds": 261977,
                    "reset_at": 1793866462
                },
                "secondary_window": null
            },
            "rate_limit_reset_credits": {
                "available_count": 3,
                "applicable_available_count": 0
            }
        });

        let buckets = parse_chatgpt_usage_json(&json);
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].id, "chatgpt_primary_window");
        assert_eq!(buckets[0].label, "Pro Lite 周周期配额");
        assert!((buckets[0].remaining_fraction - 0.03).abs() < 1e-4);
        assert_eq!(buckets[0].is_exhausted, false);
        assert!(buckets[0].reset_time.is_some());

        assert_eq!(buckets[1].id, "chatgpt_reset_credits");
        assert_eq!(buckets[1].label, "限流重置卡 (3 次可用)");
        assert_eq!(buckets[1].remaining_fraction, 1.0);
    }

    #[test]
    fn test_parse_chatgpt_usage_dual_windows() {
        let json = serde_json::json!({
            "plan_type": "plus",
            "rate_limit": {
                "allowed": false,
                "limit_reached": true,
                "primary_window": {
                    "used_percent": 100,
                    "limit_window_seconds": 18000,
                    "reset_after_seconds": 7200,
                    "reset_at": 1793866462
                },
                "secondary_window": {
                    "used_percent": 45,
                    "limit_window_seconds": 604800,
                    "reset_after_seconds": 300000,
                    "reset_at": 1794100000
                }
            }
        });

        let buckets = parse_chatgpt_usage_json(&json);
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].id, "chatgpt_primary_window");
        assert_eq!(buckets[0].label, "Plus 5小时配额");
        assert_eq!(buckets[0].remaining_fraction, 0.0);
        assert_eq!(buckets[0].is_exhausted, true);

        assert_eq!(buckets[1].id, "chatgpt_secondary_window");
        assert_eq!(buckets[1].label, "Plus 周周期配额");
        assert!((buckets[1].remaining_fraction - 0.55).abs() < 1e-4);
        assert_eq!(buckets[1].is_exhausted, true); // limit_reached: true
    }
}
