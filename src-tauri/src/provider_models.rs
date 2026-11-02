use crate::db::{presets, Provider};
use reqwest::{redirect::Policy, Url};
use serde_json::Value;
use std::collections::BTreeSet;
use std::time::Duration;

pub fn validate_base_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value.trim()).map_err(|_| "请输入有效的接口地址".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("接口地址必须以 http:// 或 https:// 开头".into());
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("地址不能包含账号、密码、查询参数或锚点".into());
    }
    if value.contains("YOUR_ACCOUNT_ID") {
        return Err("请填写接口地址中的账号 ID".into());
    }
    let path = url.path().trim_end_matches('/');
    if ["/chat/completions", "/messages", "/models"]
        .iter()
        .any(|suffix| path.ends_with(suffix))
    {
        return Err("请填写基础接口地址，不要包含聊天或模型列表请求路径".into());
    }
    Ok(url)
}

fn validate_protocol(protocol: &str) -> Result<(), String> {
    if !["openai", "anthropic", "gemini", "openrouter", "ollama", "chatgpt"].contains(&protocol) {
        return Err("不支持的服务商协议".into());
    }
    Ok(())
}

pub fn validate_provider(provider: &mut Provider) -> Result<(), String> {
    provider.name = provider.name.trim().to_string();
    if provider.id.trim().is_empty() || provider.name.is_empty() {
        return Err("请输入服务商名称".into());
    }
    validate_protocol(&provider.protocol)?;
    validate_base_url(&provider.base_url)?;
    provider.base_url = provider.base_url.trim().trim_end_matches('/').to_string();
    if let Some(id) = &provider.preset_id {
        let preset = presets::get_default_presets()
            .into_iter()
            .find(|p| &p.id == id)
            .ok_or("预设不存在，请改为自定义配置")?;
        let editable_url =
            preset.category == "开源 / 本地部署" || preset.base_url.contains("YOUR_ACCOUNT_ID");
        if provider.protocol != preset.protocol
            || (!editable_url && provider.base_url != preset.base_url.trim_end_matches('/'))
        {
            return Err("预设的协议和接口地址不能修改，请改为自定义配置".into());
        }
    }
    let mut seen = BTreeSet::new();
    provider.api_keys = provider
        .api_keys
        .iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty() && seen.insert(v.clone()))
        .collect();
    if provider
        .api_keys
        .iter()
        .any(|key| key.chars().any(char::is_control))
    {
        return Err("API 密钥不能包含换行或控制字符".into());
    }
    seen.clear();
    provider.models = provider
        .models
        .iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty() && seen.insert(v.clone()))
        .collect();
    if provider.models.is_empty() {
        return Err("请至少选择一个模型".into());
    }
    if provider.models.iter().any(|m| {
        m.chars()
            .any(|c| c.is_whitespace() || c == ',' || c == '，')
    }) {
        return Err("模型 ID 不能包含空格或逗号，请分别添加模型".into());
    }
    provider.default_model = provider
        .default_model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string);
    if let Some(model) = &provider.default_model {
        if model.chars().any(|c| c.is_whitespace() || c == ',' || c == '，') {
            return Err("默认模型 ID 不能包含空格或逗号".into());
        }
    }
    Ok(())
}

fn models_url(protocol: &str, base: &str) -> Result<Url, String> {
    validate_protocol(protocol)?;
    let mut url = validate_base_url(base)?;
    let path = url.path().trim_end_matches('/');
    // Use the same API base as the gateway's chat requests, including custom version paths.
    let path = if (protocol == "anthropic" || protocol == "openai") && path.is_empty() {
        "/v1/models".to_string()
    } else {
        format!("{path}/models")
    };
    url.set_path(&path);
    Ok(url)
}

fn clean_model_id(raw_id: &str) -> String {
    let trimmed = raw_id.trim();
    // 检查是否为第三方代理针对 Claude 客户端混淆编码的模型名：claude-fable-*-dd-<reversed_name>
    if let Some(rest) = trimmed.strip_prefix("claude-fable-") {
        if let Some(pos) = rest.find("-dd-") {
            let encoded = &rest[pos + 4..];
            if !encoded.is_empty() {
                let decoded: String = encoded.chars().rev().collect();
                return decoded;
            }
        }
    }
    trimmed.to_string()
}

fn parse_page(protocol: &str, body: &Value) -> Result<(Vec<String>, Option<String>), String> {
    let items = body[if protocol == "gemini" {
        "models"
    } else {
        "data"
    }]
    .as_array()
    .ok_or("服务商未返回兼容的模型列表")?;
    let mut models = Vec::new();
    for item in items {
        if protocol == "gemini"
            && !item["supportedGenerationMethods"]
                .as_array()
                .is_some_and(|methods| {
                    methods
                        .iter()
                        .any(|m| m.as_str() == Some("generateContent"))
                })
        {
            continue;
        }
        let id = item[if protocol == "gemini" { "name" } else { "id" }]
            .as_str()
            .ok_or("模型列表中缺少模型 ID")?;
        let id = if protocol == "gemini" {
            id.strip_prefix("models/").unwrap_or(id)
        } else {
            id
        };
        let cleaned = clean_model_id(id);
        if !cleaned.trim().is_empty() {
            models.push(cleaned);
        }
    }
    let next = if protocol == "gemini" {
        body["nextPageToken"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    } else if body["has_more"].as_bool() == Some(true) {
        Some(
            body["last_id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("模型列表分页信息不完整")?
                .to_string(),
        )
    } else {
        None
    };
    Ok((models, next))
}

pub async fn fetch_models(
    protocol: &str,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<String>, String> {
    let url = models_url(protocol, base_url)?;
    // Never forward a draft credential to a redirected host. Avoid echoing request URLs or bodies in errors.
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "无法创建模型请求")?;
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut models = BTreeSet::new();
        let mut cursor: Option<String> = None;
        let mut visited = BTreeSet::new();
        for _ in 0..100 {
            let mut request = client.get(url.clone());
            if protocol == "anthropic" {
                request = request
                    .header("anthropic-version", "2023-06-01")
                    .query(&[("limit", "1000")]);
                if !api_key.trim().is_empty() {
                    request = request.header("x-api-key", api_key.trim());
                }
            } else if protocol == "gemini" {
                request = request.query(&[("pageSize", "1000")]);
                if !api_key.trim().is_empty() {
                    request = request.header("x-goog-api-key", api_key.trim());
                }
            } else if !api_key.trim().is_empty() {
                request = request.bearer_auth(api_key.trim());
            }
            if let Some(next) = &cursor {
                request = request.query(&[(
                    if protocol == "gemini" {
                        "pageToken"
                    } else {
                        "after_id"
                    },
                    next,
                )]);
            }
            let mut response = request.send().await.map_err(|e| {
                if e.is_timeout() {
                    "获取模型超时，请重试"
                } else {
                    "无法连接服务商，请检查地址、网络和密钥格式"
                }
            })?;
            match response.status().as_u16() {
                200..=299 => {}
                401 | 403 => return Err("认证失败或没有模型列表权限，请检查 API 密钥".into()),
                404 | 405 | 501 => {
                    return Err("此地址不支持模型获取，请检查基础地址或手动添加模型".into())
                }
                429 => return Err("服务商请求频率受限，请稍后重试".into()),
                300..=399 => return Err("接口返回重定向，请直接填写最终服务地址后重试".into()),
                code => return Err(format!("服务商获取模型失败（HTTP {code}）")),
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| "读取模型列表失败")?
            {
                if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
                    return Err("模型列表响应过大".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            let body: Value =
                serde_json::from_slice(&bytes).map_err(|_| "服务商返回了无法识别的模型列表")?;
            let (page, next) = parse_page(protocol, &body)?;
            models.extend(page);
            match next {
                None => return Ok(models.into_iter().collect()),
                Some(next) if visited.insert(next.clone()) => cursor = Some(next),
                Some(_) => return Err("服务商返回了重复的分页信息，请稍后重试".into()),
            }
        }
        Err("模型列表分页过多，请缩小服务商返回范围或手动添加".into())
    })
    .await
    .map_err(|_| "获取模型超时，请重试".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        extract::Query,
        http::{HeaderMap, StatusCode},
        response::Redirect,
        routing::get,
        Json, Router,
    };
    use serde_json::json;
    use std::collections::HashMap;

    #[tokio::test]
    async fn discovery_handles_auth_pagination_errors_and_redirects() {
        async fn anthropic(
            headers: HeaderMap,
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<Value> {
            assert_eq!(headers["x-api-key"], "test-key");
            assert_eq!(headers["anthropic-version"], "2023-06-01");
            assert!(!headers.contains_key("authorization"));
            if query.contains_key("after_id") {
                Json(json!({"data":[{"id":"claude-b"},{"id":"claude-a"}],"has_more":false}))
            } else {
                Json(json!({"data":[{"id":"claude-a"}],"has_more":true,"last_id":"claude-a"}))
            }
        }
        async fn gemini(
            headers: HeaderMap,
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<Value> {
            assert_eq!(headers["x-goog-api-key"], "test-key");
            assert!(!query.contains_key("key"));
            if query.contains_key("pageToken") {
                Json(
                    json!({"models":[{"name":"models/gemini-b","supportedGenerationMethods":["generateContent"]}]}),
                )
            } else {
                Json(
                    json!({"models":[{"name":"models/gemini-a","supportedGenerationMethods":["generateContent"]},{"name":"models/embedding","supportedGenerationMethods":["embedContent"]}],"nextPageToken":"next"}),
                )
            }
        }
        let app = Router::new()
            .route("/v1/models", get(anthropic))
            .route("/v1beta/models", get(gemini))
            .route(
                "/compat/models",
                get(|headers: HeaderMap| async move {
                    assert_eq!(headers["authorization"], "Bearer test-key");
                    Json(json!({"data":[{"id":"model-a"}]}))
                }),
            )
            .route(
                "/denied/models",
                get(|| async { (StatusCode::UNAUTHORIZED, "secret-upstream-body") }),
            )
            .route(
                "/redirect/models",
                get(|| async { Redirect::temporary("/compat/models") }),
            )
            .route(
                "/broken/models",
                get(|| async { Json(json!({"wrong": []})) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        assert_eq!(
            fetch_models("anthropic", &format!("{base}/v1"), "test-key")
                .await
                .unwrap(),
            ["claude-a", "claude-b"]
        );
        assert_eq!(
            fetch_models("gemini", &format!("{base}/v1beta"), "test-key")
                .await
                .unwrap(),
            ["gemini-a", "gemini-b"]
        );
        for protocol in ["openai", "openrouter", "ollama"] {
            assert_eq!(
                fetch_models(protocol, &format!("{base}/compat"), "test-key")
                    .await
                    .unwrap(),
                ["model-a"]
            );
        }
        let error = fetch_models("openai", &format!("{base}/denied"), "test-key")
            .await
            .unwrap_err();
        assert!(error.contains("认证失败") && !error.contains("secret"));
        assert!(
            fetch_models("openai", &format!("{base}/redirect"), "test-key")
                .await
                .unwrap_err()
                .contains("重定向")
        );
        assert!(fetch_models("openai", &format!("{base}/broken"), "")
            .await
            .is_err());
        server.abort();
    }

    #[test]
    fn preset_fields_are_locked_and_credentials_are_normalized() {
        let mut provider: Provider = serde_json::from_value(json!({
            "id":"prov-test", "presetId":"openai", "name":"Test", "protocol":"openai",
            "baseUrl":"https://api.openai.com/v1", "apiKeys":[" test ","test"], "models":["org/model:tag"],
            "enabled":false, "priority":72, "weight":3, "timeoutSecs":180
        })).unwrap();
        validate_provider(&mut provider).unwrap();
        assert_eq!(provider.api_keys, ["test"]);
        assert!(!provider.enabled);
        assert_eq!(provider.timeout_secs, 180);
        provider.base_url = "https://other.example/v1".into();
        assert!(validate_provider(&mut provider)
            .unwrap_err()
            .contains("自定义"));
        provider.preset_id = None;
        validate_provider(&mut provider).unwrap();
        provider.models = vec!["one,two".into()];
        assert!(validate_provider(&mut provider).is_err());

        let mut chatgpt_prov: Provider = serde_json::from_value(json!({
            "id":"prov-chatgpt", "presetId":"chatgpt_subscription", "name":"ChatGPT (官方订阅)", "protocol":"chatgpt",
            "baseUrl":"https://chatgpt.com/backend-api", "apiKeys":[], "models":["gpt-5"],
            "enabled":true, "priority":10, "weight":1, "timeoutSecs":60
        })).unwrap();
        validate_provider(&mut chatgpt_prov).unwrap();
    }

    #[test]
    fn rejects_invalid_addresses_and_preserves_model_ids() {
        for address in [
            "file:///tmp/key",
            "https://user:pass@example.com/v1",
            "https://example.com?key=secret",
            "https://example.com/v1/chat/completions",
        ] {
            assert!(validate_base_url(address).is_err());
        }
        assert_eq!(
            models_url("openai", "https://example.com/compatible/v1/")
                .unwrap()
                .as_str(),
            "https://example.com/compatible/v1/models"
        );
        assert_eq!(
            parse_page("openai", &json!({"data":[{"id":"org/model:tag"}]}))
                .unwrap()
                .0,
            ["org/model:tag"]
        );
        assert!(parse_page("anthropic", &json!({"data":[],"has_more":true})).is_err());
    }

    #[test]
    fn test_clean_model_id() {
        assert_eq!(
            clean_model_id("claude-fable-5-dd-hgih-hsalf-8.3-inimeg"),
            "gemini-3.8-flash-high"
        );
        assert_eq!(
            clean_model_id("claude-fable-5-dd-muidem-b021-sso-tpg"),
            "gpt-oss-120b-medium"
        );
        assert_eq!(
            clean_model_id("claude-sonnet-4-6"),
            "claude-sonnet-4-6"
        );
    }
}
