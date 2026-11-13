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

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn url_decode(s: &str) -> String {
    let mut res = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                res.push((hi << 4) | lo);
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

                        let state_valid = state.as_deref() == Some(expected_state);
                        let valid_code = code.filter(|c| !c.trim().is_empty());

                        if !state_valid {
                            let err_html = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Pivot 授权失败</title></head><body><h2>授权校验失败</h2><p>State 缺失或不匹配，请回到 Pivot 重新发起授权。</p></body></html>";
                            let response = format!(
                                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                err_html.len(),
                                err_html
                            );
                            let _ = socket.write_all(response.as_bytes()).await;
                            let _ = socket.flush().await;
                            return Err("OAuth State 缺失或校验失败，可能存在 CSRF 风险".to_string());
                        }

                        if valid_code.is_none() {
                            let err_html = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Pivot 授权失败</title></head><body><h2>授权失败</h2><p>回调中未找到授权码，请重新发起授权。</p></body></html>";
                            let response = format!(
                                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                err_html.len(),
                                err_html
                            );
                            let _ = socket.write_all(response.as_bytes()).await;
                            let _ = socket.flush().await;
                            return Err("OAuth 回调中未找到授权码 code".to_string());
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

                        return Ok(valid_code.unwrap());
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

/// 刷新 Token，同时返回上游轮换下发的新 refresh_token（若有）
pub async fn refresh_antigravity_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<(String, Option<String>, i64), String> {
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

    let new_refresh_token = json["refresh_token"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let expires_in = json["expires_in"].as_i64().unwrap_or(3600);

    Ok((access_token, new_refresh_token, expires_in))
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

                        let state_valid = state.as_deref() == Some(expected_state);
                        let valid_code = code.filter(|c| !c.trim().is_empty());

                        if !state_valid {
                            let err_html = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Pivot 授权失败</title></head><body><h2>授权校验失败</h2><p>State 缺失或不匹配，请回到 Pivot 重新发起授权。</p></body></html>";
                            let response = format!(
                                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                err_html.len(),
                                err_html
                            );
                            let _ = socket.write_all(response.as_bytes()).await;
                            let _ = socket.flush().await;
                            return Err("OAuth State 缺失或校验失败，可能存在 CSRF 风险".to_string());
                        }

                        if valid_code.is_none() {
                            let err_html = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Pivot 授权失败</title></head><body><h2>授权失败</h2><p>回调中未找到授权码，请重新发起授权。</p></body></html>";
                            let response = format!(
                                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                err_html.len(),
                                err_html
                            );
                            let _ = socket.write_all(response.as_bytes()).await;
                            let _ = socket.flush().await;
                            return Err("OAuth 回调中未找到授权码 code".to_string());
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

                        return Ok(valid_code.unwrap());
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
        .header("user-agent", codex_user_agent())
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

/// 刷新 OpenAI Token，并返回上游 Refresh Token Rotation 下发的新 refresh_token（若有）
pub async fn refresh_chatgpt_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<(String, Option<String>, i64), String> {
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

    let new_refresh_token = json["refresh_token"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let expires_in = json["expires_in"].as_i64().unwrap_or(86400 * 7);

    Ok((access_token, new_refresh_token, expires_in))
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
        .header("user-agent", codex_user_agent())
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

pub fn codex_client_version() -> String {
    if let Some(home) = dirs::home_dir().or_else(|| std::env::var("HOME").ok().map(std::path::PathBuf::from)) {
        let cache_path = home.join(".codex").join("models_cache.json");
        if let Ok(content) = std::fs::read_to_string(&cache_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(ver) = v["client_version"].as_str() {
                    if !ver.is_empty() {
                        return ver.to_string();
                    }
                }
            }
        }
    }
    "0.166.0".to_string()
}

pub fn codex_user_agent() -> String {
    format!("codex-cli/{}", codex_client_version())
}

/// 尝试从本地 ~/.codex/models_cache.json 直接读取并解析模型列表（按 priority 排序并过滤掉内部审查机器人）
pub fn get_local_codex_cached_models() -> Option<Vec<String>> {
    let home = dirs::home_dir().or_else(|| std::env::var("HOME").ok().map(std::path::PathBuf::from))?;
    let cache_path = home.join(".codex").join("models_cache.json");
    let content = std::fs::read_to_string(cache_path).ok()?;
    let v = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    let arr = v["models"].as_array()?;
    let mut entries = Vec::new();
    for m in arr {
        if let Some(slug) = m["slug"].as_str() {
            if slug == "codex-auto-review" {
                continue;
            }
            let priority = m["priority"].as_i64().unwrap_or(999);
            entries.push((priority, slug.to_string()));
        }
    }
    if entries.is_empty() {
        return None;
    }
    entries.sort_by_key(|e| e.0);
    Some(entries.into_iter().map(|e| e.1).collect())
}

/// 严格从上游 ChatGPT 接口拉取当前账号真实可用的模型列表（不回退本地缓存或硬编码兜底）
pub async fn fetch_chatgpt_account_models(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<Vec<String>, String> {
    let client_version = codex_client_version();
    let url = format!(
        "https://chatgpt.com/backend-api/codex/models?client_version={}",
        client_version
    );
    let ua = codex_user_agent();

    let resp = client
        .get(&url)
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", &ua)
        .send()
        .await
        .map_err(|e| format!("请求 ChatGPT 可用模型失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_body = resp.text().await.unwrap_or_default();
        return Err(format!("ChatGPT 返回错误 ({}): {}", status, err_body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 ChatGPT 模型列表失败: {}", e))?;

    let arr = json["models"]
        .as_array()
        .ok_or_else(|| "ChatGPT 模型列表响应缺少 models 字段".to_string())?;

    let mut entries = Vec::new();
    for m in arr {
        if let Some(slug) = m["slug"].as_str() {
            let trimmed = slug.trim();
            if trimmed.is_empty() || trimmed == "codex-auto-review" {
                continue;
            }
            let priority = m["priority"].as_i64().unwrap_or(999);
            entries.push((priority, trimmed.to_string()));
        }
    }

    if entries.is_empty() {
        return Err("ChatGPT 未返回任何可用模型".to_string());
    }

    entries.sort_by_key(|e| e.0);
    Ok(entries.into_iter().map(|e| e.1).collect())
}

/// 获取 ChatGPT 可用模型列表（带本地缓存与预设兜底）
pub async fn fetch_chatgpt_models(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<Vec<String>, String> {
    if let Ok(models) = fetch_chatgpt_account_models(client, access_token).await {
        return Ok(models);
    }

    if let Some(local) = get_local_codex_cached_models() {
        return Ok(local);
    }

    Ok(vec![
        "gpt-6.1-sol".to_string(),
        "gpt-6-astra".to_string(),
        "gpt-6.1-terra".to_string(),
        "gpt-6-sol".to_string(),
        "gpt-reserve".to_string(),
        "gpt-6-luna".to_string(),
        "gpt-5.6-sol".to_string(),
        "gpt-5.6-terra".to_string(),
        "gpt-5.6-luna".to_string(),
    ])
}

/// 规范化 ChatGPT 订阅套餐类型标识
pub fn format_chatgpt_plan_type(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = match trimmed.to_lowercase().as_str() {
        "pro" => "Pro".to_string(),
        "prolite" | "pro_lite" | "pro-lite" => "Pro Lite".to_string(),
        "plus" => "Plus".to_string(),
        "team" => "Team".to_string(),
        "enterprise" => "Enterprise".to_string(),
        "edu" => "Edu".to_string(),
        "free" => "Free".to_string(),
        _ => trimmed.to_string(),
    };
    Some(normalized)
}

/// 探测 ChatGPT 账号当前的订阅套餐类型
pub async fn fetch_chatgpt_plan_type(
    client: &reqwest::Client,
    access_token: &str,
) -> Option<String> {
    if let Ok(res) = client
        .get("https://chatgpt.com/backend-api/wham/usage")
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", codex_user_agent())
        .header("accept", "application/json")
        .send()
        .await
    {
        if res.status().is_success() {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(pt) = json["plan_type"].as_str().and_then(format_chatgpt_plan_type) {
                    return Some(pt);
                }
            }
        }
    }

    if let Ok(res) = client
        .get("https://chatgpt.com/backend-api/accounts/check/v4-2023-04-27")
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", codex_user_agent())
        .send()
        .await
    {
        if res.status().is_success() {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(accs) = json["accounts"].as_object() {
                    if let Some(first) = accs.values().next() {
                        if let Some(pt) = first["account"]["plan_type"]
                            .as_str()
                            .and_then(format_chatgpt_plan_type)
                        {
                            return Some(pt);
                        }
                    }
                }
            }
        }
    }

    None
}

/// 解析 ChatGPT / Codex 官方 /wham/usage 额度响应
pub fn parse_chatgpt_usage_json(json: &serde_json::Value) -> Vec<QuotaBucket> {
    let mut buckets = Vec::new();

    let plan_label = json["plan_type"]
        .as_str()
        .and_then(format_chatgpt_plan_type)
        .unwrap_or_default();

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
        // 仅按当前窗口自身剩余比例判定是否耗尽，避免 5h 窗口触顶时连坐将未耗尽的 7d 周窗口标记为耗尽
        let is_exhausted = remaining_fraction <= 0.005;

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

    if limit_reached && !buckets.is_empty() && !buckets.iter().any(|b| b.is_exhausted) {
        if let Some((min_idx, _)) = buckets
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.remaining_fraction.total_cmp(&b.remaining_fraction))
        {
            buckets[min_idx].is_exhausted = true;
        }
    }

    buckets
}

/// 获取 ChatGPT 订阅状态、配额、限流重置卡与套餐等级
pub async fn fetch_chatgpt_quota(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<(Vec<QuotaBucket>, Vec<ResetCredit>, Option<String>), String> {
    // 1. 尝试直接请求 Codex 官方额度接口 /wham/usage 获取真实可用额度与套餐类型
    let usage_resp = client
        .get("https://chatgpt.com/backend-api/wham/usage")
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", codex_user_agent())
        .header("accept", "application/json")
        .send()
        .await;

    let mut buckets = Vec::new();
    let mut detected_plan_type = None;
    if let Ok(res) = usage_resp {
        if res.status().is_success() {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                detected_plan_type = json["plan_type"]
                    .as_str()
                    .and_then(format_chatgpt_plan_type);
                buckets = parse_chatgpt_usage_json(&json);
            }
        }
    }

    // 2. 请求限流重置卡详情 /wham/rate-limit-reset-credits
    let mut reset_credits = Vec::new();
    let credits_resp = client
        .get("https://chatgpt.com/backend-api/wham/rate-limit-reset-credits")
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", codex_user_agent())
        .header("accept", "application/json")
        .send()
        .await;

    if let Ok(res) = credits_resp {
        if res.status().is_success() {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(arr) = json["credits"].as_array() {
                    for c in arr {
                        let status = c["status"].as_str().unwrap_or("available");
                        if status == "available" {
                            reset_credits.push(ResetCredit {
                                id: c["id"].as_str().unwrap_or("").to_string(),
                                status: status.to_string(),
                                expires_at: c["expires_at"].as_str().unwrap_or("").to_string(),
                                granted_at: c["granted_at"].as_str().map(ToString::to_string),
                            });
                        }
                    }
                }
            }
        }
    }

    // 按过期时间从小到大排序（最先过期的排最前）
    reset_credits.sort_by(|a, b| a.expires_at.cmp(&b.expires_at));

    if buckets.is_empty() || detected_plan_type.is_none() {
        // 兜底回退：通过账号状态检查接口读取套餐类型
        let resp = client
            .get("https://chatgpt.com/backend-api/accounts/check/v4-2023-04-27")
            .header("authorization", format!("Bearer {}", access_token))
            .header("user-agent", codex_user_agent())
            .send()
            .await;

        let mut plan_name = "ChatGPT有效".to_string();
        if let Ok(res) = resp {
            if res.status().is_success() {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    if let Some(accs) = json["accounts"].as_object() {
                        if let Some(first) = accs.values().next() {
                            if let Some(pt_norm) = first["account"]["plan_type"]
                                .as_str()
                                .and_then(format_chatgpt_plan_type)
                            {
                                plan_name = format!("ChatGPT {} 订阅 (有效)", pt_norm);
                                if detected_plan_type.is_none() {
                                    detected_plan_type = Some(pt_norm);
                                }
                            }
                        }
                    }
                }
            }
        }

        if buckets.is_empty() {
            buckets.push(QuotaBucket {
                id: "chatgpt_subscription_status".to_string(),
                label: plan_name,
                remaining_fraction: 1.0,
                reset_time: None,
                is_exhausted: false,
            });
        }
    }

    Ok((buckets, reset_credits, detected_plan_type))
}

/// 消耗一张限流重置卡立即重置限流
pub async fn consume_chatgpt_reset_credit(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<serde_json::Value, String> {
    let req_id = uuid::Uuid::new_v4().to_string();
    let resp = client
        .post("https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume")
        .header("authorization", format!("Bearer {}", access_token))
        .header("user-agent", codex_user_agent())
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "redeem_request_id": req_id
        }))
        .send()
        .await
        .map_err(|e| format!("请求重置卡接口失败: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("使用重置卡失败 ({}): {}", status, err_text));
    }

    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("解析重置响应失败: {}", e))
}

/// 判断某个订阅账号是否支持指定模型；尚未探测的旧数据（supported_models 为 None）视为默认兼容
pub fn account_supports_model(account: &crate::db::SubscriptionAccount, target_model: &str) -> bool {
    let target = target_model.trim();
    if target.is_empty() {
        return true;
    }
    let Some(ref supported) = account.supported_models else {
        return true;
    };

    if supported.iter().any(|m| m.eq_ignore_ascii_case(target)) {
        return true;
    }

    let target_candidates = crate::gateway::router::normalize_model_candidates(target);
    for candidate in &target_candidates {
        if supported.iter().any(|m| m.eq_ignore_ascii_case(candidate)) {
            return true;
        }
    }

    for declared in supported {
        for candidate in crate::gateway::router::normalize_model_candidates(declared) {
            if candidate.eq_ignore_ascii_case(target) {
                return true;
            }
        }
    }

    false
}

static REFRESH_LOCKS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>,
> = std::sync::OnceLock::new();

fn account_refresh_lock(account_id: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let map_mutex = REFRESH_LOCKS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut map = map_mutex.lock().unwrap();
    map.entry(account_id.to_string())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

static FAMILY_COOLDOWNS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, i64>>,
> = std::sync::OnceLock::new();

pub fn model_family_key(model: &str) -> Option<&'static str> {
    let lower = model.trim().to_lowercase();
    if lower.contains("claude") {
        Some("claude")
    } else if lower.contains("gemini") {
        Some("gemini")
    } else {
        None
    }
}

pub fn mark_account_family_cooldown_until(account_id: &str, family: &str, until_sec: i64) {
    let lock = FAMILY_COOLDOWNS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut map = lock.lock().unwrap();
    let key = format!("{}:{}", account_id, family);
    let prev = map.get(&key).copied().unwrap_or(0);
    if until_sec > prev {
        map.insert(key, until_sec);
    }
}

pub fn clear_account_family_cooldown(account_id: &str, family: &str) {
    if let Some(lock) = FAMILY_COOLDOWNS.get() {
        let mut map = lock.lock().unwrap();
        map.remove(&format!("{}:{}", account_id, family));
    }
}

pub fn mark_account_family_cooldown(account_id: &str, model: &str, cooldown_seconds: i64) {
    if let Some(family) = model_family_key(model) {
        let until = chrono::Utc::now().timestamp() + cooldown_seconds;
        mark_account_family_cooldown_until(account_id, family, until);
    }
}

pub fn is_account_family_cooled_down(account_id: &str, model: &str, now_sec: i64) -> bool {
    let Some(family) = model_family_key(model) else {
        return false;
    };
    let Some(lock) = FAMILY_COOLDOWNS.get() else {
        return false;
    };
    let map = lock.lock().unwrap();
    let key = format!("{}:{}", account_id, family);
    map.get(&key).copied().unwrap_or(0) > now_sec
}

/// 按账号加单飞锁检查/刷新 Token，保留上游轮换下发的新 refresh_token，不清除已有配额冷却
pub async fn ensure_or_refresh_account_token(
    db: &crate::db::Database,
    http_client: &reqwest::Client,
    account_id: &str,
    force_if_token_matches: Option<&str>,
) -> Result<crate::db::SubscriptionAccount, String> {
    let lock = account_refresh_lock(account_id);
    let _guard = lock.lock().await;

    let mut account = db
        .get_subscription_account_by_id(account_id)
        .map_err(|e| format!("查询账号失败: {}", e))?
        .ok_or_else(|| "未找到指定账号".to_string())?;

    let now_sec = chrono::Utc::now().timestamp();
    if let Some(failed_tok) = force_if_token_matches {
        // 并发请求已先完成刷新，直接复用最新 Token
        if account.access_token != failed_tok && !account.auth_invalid {
            return Ok(account);
        }
    } else if account.expires_at - now_sec >= 300 && !account.auth_invalid {
        return Ok(account);
    }

    let refresh_token = account
        .refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let Some(rt) = refresh_token else {
        if force_if_token_matches.is_some() || account.expires_at <= now_sec {
            let _ = db.set_account_auth_invalid(&account.id, true);
            return Err("该账号无 refresh_token 凭据，无法自动续期".to_string());
        }
        return Ok(account);
    };

    let refresh_res = if account.account_type == "chatgpt" {
        refresh_chatgpt_token(http_client, &rt).await
    } else {
        refresh_antigravity_token(http_client, &rt).await
    };

    match refresh_res {
        Ok((new_token, maybe_new_rt, expires_in)) => {
            let new_expires_at = now_sec + expires_in;
            let effective_rt = maybe_new_rt
                .as_deref()
                .or(account.refresh_token.as_deref());
            let _ = db.update_account_tokens(&account.id, &new_token, effective_rt, new_expires_at);
            account.access_token = new_token;
            if let Some(new_rt) = maybe_new_rt {
                account.refresh_token = Some(new_rt);
            }
            account.expires_at = new_expires_at;
            account.auth_invalid = false;
            Ok(account)
        }
        Err(e) => {
            tracing::warn!(
                "刷新订阅账号 {} ({:?}) Token 失败: {}",
                account.id,
                account.email,
                e
            );
            if is_token_revoked_or_invalid(&e) {
                let _ = db.set_account_auth_invalid(&account.id, true);
                account.auth_invalid = true;
            } else if account.expires_at <= now_sec {
                // 瞬时网络失败且 Token 已过期时短暂冷却 60 秒，避免高频重试
                let _ = db.set_account_cooldown(&account.id, account.cooldown_until.max(now_sec + 60));
            }
            if force_if_token_matches.is_some() || account.expires_at <= now_sec || account.auth_invalid {
                Err(e)
            } else {
                Ok(account)
            }
        }
    }
}

/// 从指定 provider 获取一个可用的订阅账号（支持按目标模型过滤、排除失败账号、自动轮询、检查冷却、单飞续期 Token）
pub async fn acquire_subscription_account_for_model(
    db: &crate::db::Database,
    http_client: &reqwest::Client,
    provider_id: &str,
    target_model: Option<&str>,
    exclude_account_ids: &[String],
) -> Result<crate::db::SubscriptionAccount, (axum::http::StatusCode, String)> {
    let accounts = db.get_subscription_accounts(provider_id).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("读取订阅账号失败: {}", e),
        )
    })?;

    let enabled_accounts: Vec<_> = accounts.into_iter().filter(|a| a.enabled).collect();
    if enabled_accounts.is_empty() {
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "当前订阅账号池中没有可用账号（尚未绑定或全部已禁用）".to_string(),
        ));
    }

    let model_trimmed = target_model.map(str::trim).filter(|m| !m.is_empty());
    let model_matched: Vec<_> = if let Some(model) = model_trimmed {
        let matched: Vec<_> = enabled_accounts
            .into_iter()
            .filter(|a| account_supports_model(a, model))
            .collect();
        if matched.is_empty() {
            return Err((
                axum::http::StatusCode::FORBIDDEN,
                format!(
                    "当前已启用的订阅账号等级均不支持模型 '{}'，请连接支持该模型的订阅账号或在客户端切换其他模型",
                    model
                ),
            ));
        }
        matched
    } else {
        enabled_accounts
    };

    let after_exclude: Vec<_> = model_matched
        .into_iter()
        .filter(|a| !exclude_account_ids.iter().any(|ex| ex == &a.id))
        .collect();
    if after_exclude.is_empty() {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "当前订阅账号池中无更多支持该模型的候选账号".to_string(),
        ));
    }

    let valid_auth: Vec<_> = after_exclude
        .into_iter()
        .filter(|a| !a.auth_invalid)
        .collect();
    if valid_auth.is_empty() {
        return Err((
            axum::http::StatusCode::UNAUTHORIZED,
            "订阅账号授权已失效，请在服务商设置中重新授权".to_string(),
        ));
    }

    let now_sec = chrono::Utc::now().timestamp();
    let available: Vec<_> = valid_auth
        .into_iter()
        .filter(|a| {
            a.cooldown_until <= now_sec
                && !model_trimmed
                    .map(|m| is_account_family_cooled_down(&a.id, m, now_sec))
                    .unwrap_or(false)
        })
        .collect();

    if available.is_empty() {
        let msg = if let Some(model) = model_trimmed {
            format!(
                "支持模型 '{}' 的订阅账号当前均在配额冷却中，请稍后重试",
                model
            )
        } else {
            "当前订阅账号池中没有可用账号（全部在冷却中或尚未绑定账号）".to_string()
        };
        return Err((axum::http::StatusCode::TOO_MANY_REQUESTS, msg));
    }

    static SUB_RR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let start_idx = SUB_RR.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % available.len();
    let mut last_err = String::new();

    // 循环遍历可用账号，若某个账号刷新失败或已过期，自动尝试池中的下一个账号
    for i in 0..available.len() {
        let candidate = &available[(start_idx + i) % available.len()];
        match ensure_or_refresh_account_token(db, http_client, &candidate.id, None).await {
            Ok(refreshed) => return Ok(refreshed),
            Err(e) => {
                last_err = format!(
                    "订阅账号 {} Token 刷新失败: {}",
                    candidate.email.as_deref().unwrap_or(&candidate.id),
                    e
                );
            }
        }
    }

    Err((
        axum::http::StatusCode::UNAUTHORIZED,
        if last_err.is_empty() {
            "当前订阅账号池中所有账号 Token 均已过期且无法刷新".to_string()
        } else {
            last_err
        },
    ))
}

/// 为指定 provider 获取一个可用的订阅账号（兼容无模型过滤的调用方）
pub async fn acquire_subscription_account(
    db: &crate::db::Database,
    http_client: &reqwest::Client,
    provider_id: &str,
) -> Result<crate::db::SubscriptionAccount, (axum::http::StatusCode, String)> {
    acquire_subscription_account_for_model(db, http_client, provider_id, None, &[]).await
}

/// 当上游返回 401 Unauthorized 时，强制尝试使用 refresh_token 刷新账号凭据（带单飞保护）
pub async fn try_refresh_subscription_account(
    db: &crate::db::Database,
    http_client: &reqwest::Client,
    account_id: &str,
) -> Result<crate::db::SubscriptionAccount, String> {
    let existing = db
        .get_subscription_account_by_id(account_id)
        .map_err(|e| format!("查询账号失败: {}", e))?
        .ok_or_else(|| "未找到指定账号".to_string())?;
    ensure_or_refresh_account_token(db, http_client, account_id, Some(&existing.access_token)).await
}

/// 判断 Token 刷新或上游鉴权错误是否属于授权撤销/失效（需要用户重新授权）
pub fn is_token_revoked_or_invalid(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("invalid_grant")
        || lower.contains("invalid_refresh_token")
        || lower.contains("invalid refresh token")
        || lower.contains("token_revoked")
        || lower.contains("token has been revoked")
        || lower.contains("token has been expired or revoked")
        || lower.contains("refresh_token_reused")
        || lower.contains("account_deactivated")
        || lower.contains("unauthenticated")
        || lower.contains("401 unauthorized")
        || lower.contains("unauthorized")
        || lower.contains("invalid_token")
        || lower.contains("invalid token")
        || lower.contains("token_expired")
        || lower.contains("token expired")
        || lower.contains("token is expired")
}

/// 标记账号进入冷却（如遇到 429 速率限制，默认冷却 5 分钟）
pub fn mark_account_cooldown(db: &crate::db::Database, account_id: &str, cooldown_seconds: i64) {
    let until = chrono::Utc::now().timestamp() + cooldown_seconds;
    let _ = db.set_account_cooldown(account_id, until);
}

/// 识别上游错误是否属于“当前账号订阅等级不支持/无权使用该模型”
pub fn is_model_unsupported_for_account(status: axum::http::StatusCode, body: &str) -> bool {
    if !matches!(
        status,
        axum::http::StatusCode::BAD_REQUEST
            | axum::http::StatusCode::FORBIDDEN
            | axum::http::StatusCode::NOT_FOUND
    ) {
        return false;
    }

    let lower = body.to_lowercase();
    // 排除限流、容量耗尽、上下文超长与认证失效错误，避免误删账号模型权限
    for excluded in [
        "rate_limit",
        "quota",
        "resource_exhausted",
        "capacity_exhausted",
        "context_length",
        "too many tokens",
        "maximum context",
        "unauthenticated",
        "invalid_grant",
        "invalid_refresh_token",
    ] {
        if lower.contains(excluded) {
            return false;
        }
    }

    if lower.contains("model_not_found")
        || lower.contains("unsupported_model")
        || lower.contains("model is not supported")
        || lower.contains("invalid model")
        || lower.contains("unknown model")
        || lower.contains("do not have access")
        || lower.contains("does not have access")
        || lower.contains("not authorized to use")
    {
        return true;
    }

    if lower.contains("plan")
        && (lower.contains("not supported")
            || lower.contains("not available")
            || lower.contains("upgrade")
            || lower.contains("only available"))
    {
        return true;
    }

    if lower.contains("tier")
        && (lower.contains("not supported")
            || lower.contains("not available")
            || lower.contains("upgrade")
            || lower.contains("entitle"))
    {
        return true;
    }

    if status == axum::http::StatusCode::NOT_FOUND
        && lower.contains("model")
        && (lower.contains("not found")
            || lower.contains("not_found")
            || lower.contains("does not exist"))
    {
        return true;
    }

    if status == axum::http::StatusCode::FORBIDDEN
        && lower.contains("permission_denied")
        && lower.contains("model")
    {
        return true;
    }

    false
}

/// 当上游判定某账号无权调用指定模型时，动态从该账号的支持列表中移除该模型（不冷却整个账号）
pub fn handle_account_model_unsupported(
    db: &crate::db::Database,
    http_client: &reqwest::Client,
    account: &crate::db::SubscriptionAccount,
    unsupported_model: &str,
    base_url: &str,
) {
    if account.supported_models.is_some() {
        let _ = db.remove_model_from_account(&account.id, unsupported_model);
        return;
    }

    // 存量未探测账号（supported_models 为 None）：后台拉取真实模型列表并剔除失败模型；
    // 若探测失败或结果为空，回退从所属服务商声明列表中剔除该模型后精确落库，彻底阻断自愈死循环
    let db_clone = db.clone();
    let client_clone = http_client.clone();
    let acc_clone = account.clone();
    let model_clone = unsupported_model.to_string();
    let base_clone = base_url.to_string();

    tokio::spawn(async move {
        let fetched_res = if acc_clone.account_type == "chatgpt" {
            fetch_chatgpt_account_models(&client_clone, &acc_clone.access_token).await
        } else {
            fetch_antigravity_models(&client_clone, &acc_clone.access_token, &base_clone).await
        };
        let mut models = match fetched_res {
            Ok(list) => list,
            Err(_) => db_clone
                .get_providers()
                .ok()
                .and_then(|ps| {
                    ps.into_iter()
                        .find(|p| p.id == acc_clone.provider_id)
                        .map(|p| p.models)
                })
                .unwrap_or_default(),
        };
        models.retain(|m| !m.eq_ignore_ascii_case(&model_clone));
        let _ = db_clone.set_account_supported_models_exact(&acc_clone.id, &models);
    });
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
pub struct ResetCredit {
    pub id: String,
    pub status: String,
    pub expires_at: String,
    pub granted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountQuota {
    pub account_id: String,
    pub updated_at: i64,
    pub buckets: Vec<QuotaBucket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_credits: Option<Vec<ResetCredit>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
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
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].id, "chatgpt_primary_window");
        assert_eq!(buckets[0].label, "Pro Lite 周周期配额");
        assert!((buckets[0].remaining_fraction - 0.03).abs() < 1e-4);
        assert!(!buckets[0].is_exhausted);
        assert!(buckets[0].reset_time.is_some());
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
        assert!(buckets[0].is_exhausted);

        assert_eq!(buckets[1].id, "chatgpt_secondary_window");
        assert_eq!(buckets[1].label, "Plus 周周期配额");
        assert!((buckets[1].remaining_fraction - 0.55).abs() < 1e-4);
        assert!(!buckets[1].is_exhausted); // 5h 耗尽不连坐剩余 55% 的周配额
    }

    #[test]
    fn test_is_token_revoked_or_invalid() {
        assert!(is_token_revoked_or_invalid("刷新 OpenAI Token 失败: { \"error\": { \"message\": \"Invalid refresh token.\", \"type\": \"invalid_request_error\", \"param\": null, \"code\": \"invalid_refresh_token\" } }"));
        assert!(is_token_revoked_or_invalid("Token has been revoked"));
        assert!(is_token_revoked_or_invalid("invalid_grant: bad credentials"));
        assert!(is_token_revoked_or_invalid("401 Unauthorized"));
        assert!(is_token_revoked_or_invalid("UNAUTHENTICATED: Request had invalid authentication credentials"));
        assert!(!is_token_revoked_or_invalid("Connection timed out after 5000 ms"));
        assert!(!is_token_revoked_or_invalid("刷新 OpenAI Token 请求失败: TLS invalid peer certificate"));
        assert!(!is_token_revoked_or_invalid("502 Bad Gateway <html>error 40123</html>"));
    }

    #[test]
    fn test_url_decode_does_not_panic_on_utf8_after_percent() {
        assert_eq!(url_decode("abc%中文"), "abc%中文");
        assert_eq!(url_decode("a+b%20c%2F"), "a b c/");
    }

    #[test]
    fn test_chatgpt_usage_5h_limit_does_not_exhaust_7d_window() {
        let json = serde_json::json!({
            "plan_type": "plus",
            "rate_limit": {
                "limit_reached": true,
                "primary_window": {
                    "used_percent": 20.0,
                    "limit_window_seconds": 604800,
                    "reset_after_seconds": 500000
                },
                "secondary_window": {
                    "used_percent": 100.0,
                    "limit_window_seconds": 18000,
                    "reset_after_seconds": 3600
                }
            }
        });
        let buckets = parse_chatgpt_usage_json(&json);
        assert_eq!(buckets.len(), 2);
        assert!(!buckets[0].is_exhausted, "7天周窗口剩余80%不应被连坐标记为耗尽");
        assert!(buckets[1].is_exhausted, "5小时窗口应标记为耗尽");
    }

    #[test]
    fn test_account_supports_model_and_normalization() {
        let mut acc = crate::db::SubscriptionAccount {
            id: "sub_1".to_string(),
            provider_id: "chatgpt".to_string(),
            account_type: "chatgpt".to_string(),
            email: Some("user@example.com".to_string()),
            access_token: "tok".to_string(),
            refresh_token: None,
            expires_at: 9_999_999_999,
            cooldown_until: 0,
            enabled: true,
            plan_type: Some("Plus".to_string()),
            supported_models: None,
            auth_invalid: false,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };

        // 尚未探测的旧数据（supported_models = None）兼容所有模型
        assert!(account_supports_model(&acc, "gpt-6-astra"));

        acc.supported_models = Some(vec![
            "gpt-5.6-luna".to_string(),
            "claude-3-7-sonnet".to_string(),
        ]);
        assert!(account_supports_model(&acc, "gpt-5.6-luna"));
        assert!(account_supports_model(&acc, "GPT-5.6-LUNA"));
        assert!(account_supports_model(&acc, "claude-3-7-sonnet-20250219"));
        assert!(!account_supports_model(&acc, "gpt-6-astra"));
    }

    #[test]
    fn test_is_model_unsupported_for_account() {
        use axum::http::StatusCode;
        assert!(is_model_unsupported_for_account(
            StatusCode::NOT_FOUND,
            r#"{"error":{"code":"model_not_found","message":"The model gpt-6-astra does not exist or you do not have access to it."}}"#
        ));
        assert!(is_model_unsupported_for_account(
            StatusCode::FORBIDDEN,
            r#"{"error":{"message":"Your current plan does not support this model. Please upgrade to Pro."}}"#
        ));
        assert!(is_model_unsupported_for_account(
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"This model is not supported for your account tier."}}"#
        ));
        // 限流、容量、鉴权失败绝不能误判为模型不支持
        assert!(!is_model_unsupported_for_account(
            StatusCode::TOO_MANY_REQUESTS,
            r#"{"error":{"code":"rate_limit_exceeded","message":"model_not_found"}}"#
        ));
        assert!(!is_model_unsupported_for_account(
            StatusCode::FORBIDDEN,
            r#"{"error":{"message":"Quota exceeded for model gpt-6-astra"}}"#
        ));
        assert!(!is_model_unsupported_for_account(
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"maximum context length exceeded for this model"}}"#
        ));
    }

    #[tokio::test]
    async fn test_acquire_subscription_account_for_model_tiered_pool() {
        let dir = std::env::temp_dir().join(format!("pivot-oauth-tier-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let db = crate::db::Database::new(db_path.clone()).unwrap();
        let client = reqwest::Client::new();
        let future_exp = chrono::Utc::now().timestamp() + 3600;

        // Plus 账号：仅支持基础模型
        db.save_subscription_account(&crate::db::SubscriptionAccount {
            id: "acc_plus".to_string(),
            provider_id: "chatgpt_subscription".to_string(),
            account_type: "chatgpt".to_string(),
            email: Some("plus@example.com".to_string()),
            access_token: "tok_plus".to_string(),
            refresh_token: None,
            expires_at: future_exp,
            cooldown_until: 0,
            enabled: true,
            plan_type: Some("Plus".to_string()),
            supported_models: Some(vec!["gpt-5.6-luna".to_string()]),
            auth_invalid: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        })
        .unwrap();

        // Pro 账号：支持基础模型 + 高阶独享模型
        db.save_subscription_account(&crate::db::SubscriptionAccount {
            id: "acc_pro".to_string(),
            provider_id: "chatgpt_subscription".to_string(),
            account_type: "chatgpt".to_string(),
            email: Some("pro@example.com".to_string()),
            access_token: "tok_pro".to_string(),
            refresh_token: None,
            expires_at: future_exp,
            cooldown_until: 0,
            enabled: true,
            plan_type: Some("Pro".to_string()),
            supported_models: Some(vec![
                "gpt-5.6-luna".to_string(),
                "gpt-6-astra".to_string(),
            ]),
            auth_invalid: false,
            created_at: "2026-01-02T00:00:00Z".to_string(),
            updated_at: "2026-01-02T00:00:00Z".to_string(),
        })
        .unwrap();

        // 1. 请求高阶独享模型 -> 100% 命中 Pro 账号
        for _ in 0..4 {
            let picked = acquire_subscription_account_for_model(
                &db,
                &client,
                "chatgpt_subscription",
                Some("gpt-6-astra"),
                &[],
            )
            .await
            .unwrap();
            assert_eq!(picked.id, "acc_pro");
        }

        // 2. 请求共有基础模型 -> 两个账号平等轮询
        let mut seen_ids = std::collections::HashSet::new();
        for _ in 0..6 {
            let picked = acquire_subscription_account_for_model(
                &db,
                &client,
                "chatgpt_subscription",
                Some("gpt-5.6-luna"),
                &[],
            )
            .await
            .unwrap();
            seen_ids.insert(picked.id);
        }
        assert_eq!(seen_ids.len(), 2);

        // 3. 排除 acc_pro 时，请求独享模型应返回 SERVICE_UNAVAILABLE 而非派给 Plus
        let (status, _) = acquire_subscription_account_for_model(
            &db,
            &client,
            "chatgpt_subscription",
            Some("gpt-6-astra"),
            &["acc_pro".to_string()],
        )
        .await
        .unwrap_err();
        assert_eq!(status, axum::http::StatusCode::SERVICE_UNAVAILABLE);

        // 4. Pro 账号冷却时，请求独享模型返回 429，绝不误派给 Plus 账号
        db.set_account_cooldown("acc_pro", chrono::Utc::now().timestamp() + 300)
            .unwrap();
        let (status, msg) = acquire_subscription_account_for_model(
            &db,
            &client,
            "chatgpt_subscription",
            Some("gpt-6-astra"),
            &[],
        )
        .await
        .unwrap_err();
        assert_eq!(status, axum::http::StatusCode::TOO_MANY_REQUESTS);
        assert!(msg.contains("gpt-6-astra"));

        // 5. 禁用 Pro 账号后，池中仅剩 Plus 账号 -> 请求高阶模型直接返回 403
        let mut pro_acc = db.get_subscription_account_by_id("acc_pro").unwrap().unwrap();
        pro_acc.enabled = false;
        db.save_subscription_account(&pro_acc).unwrap();
        let (status, msg) = acquire_subscription_account_for_model(
            &db,
            &client,
            "chatgpt_subscription",
            Some("gpt-6-astra"),
            &[],
        )
        .await
        .unwrap_err();
        assert_eq!(status, axum::http::StatusCode::FORBIDDEN);
        assert!(msg.contains("gpt-6-astra"));

        drop(db);
        let _ = std::fs::remove_file(db_path);
    }
}
