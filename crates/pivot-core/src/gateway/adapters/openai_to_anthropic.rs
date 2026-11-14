use crate::gateway::types::*;
use serde_json::{json, Value};

fn is_anthropic_thinking_supported(target_model: &str) -> bool {
    let lower = target_model.to_lowercase();
    let name = lower.split('/').last().unwrap_or(&lower);
    let is_explicit_non_thinking = (name.contains("claude-3-5")
        || name.contains("claude-3-opus")
        || name.contains("claude-3-haiku")
        || name.starts_with("claude-2"))
        && !name.contains("thinking");
    if is_explicit_non_thinking {
        return false;
    }
    name.contains("claude-3-7")
        || name.contains("opus-4")
        || name.contains("sonnet-4")
        || name.contains("thinking")
}

fn openai_image_part_to_anthropic(part: &Value) -> Option<Value> {
    let url = part
        .get("image_url")
        .and_then(|v| v.get("url").and_then(Value::as_str).or_else(|| v.as_str()))?;
    if let Some(rest) = url.strip_prefix("data:") {
        if let Some((media_type, data)) = rest.split_once(";base64,") {
            if !data.is_empty() {
                let mime = if media_type.is_empty() {
                    "image/png"
                } else {
                    media_type
                };
                return Some(json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": mime,
                        "data": data
                    }
                }));
            }
        }
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        return Some(json!({
            "type": "image",
            "source": {
                "type": "url",
                "url": url
            }
        }));
    }
    None
}

fn convert_openai_user_content_to_anthropic(content: Option<&Value>) -> Value {
    match content {
        Some(Value::Array(arr)) => {
            let blocks: Vec<Value> = arr
                .iter()
                .filter_map(|part| {
                    let ptype = part.get("type").and_then(Value::as_str).unwrap_or("");
                    match ptype {
                        "text" => part
                            .get("text")
                            .and_then(Value::as_str)
                            .map(|t| json!({ "type": "text", "text": t })),
                        "image_url" => openai_image_part_to_anthropic(part),
                        "image" | "tool_result" => Some(part.clone()),
                        _ => Some(part.clone()),
                    }
                })
                .collect();
            Value::Array(blocks)
        }
        Some(other) => other.clone(),
        None => Value::String(String::new()),
    }
}

pub fn convert_openai_req_to_anthropic(
    req: &OpenAIChatRequest,
    target_model: &str,
) -> AnthropicMessageRequest {
    let mut system_text = String::new();
    let mut anthropic_messages = Vec::new();

    for msg in &req.messages {
        match msg.role.as_str() {
            "system" | "developer" => {
                if let Some(ref c) = msg.content {
                    let text = if let Value::String(s) = c {
                        s.clone()
                    } else if let Some(arr) = c.as_array() {
                        let mut parts = Vec::new();
                        for p in arr {
                            if let Some(s) = p.as_str() {
                                parts.push(s.to_string());
                            } else if let Some(t) = p.get("text").and_then(Value::as_str) {
                                parts.push(t.to_string());
                            }
                        }
                        parts.join("\n")
                    } else {
                        String::new()
                    };
                    if !text.is_empty() {
                        if !system_text.is_empty() {
                            system_text.push('\n');
                        }
                        system_text.push_str(&text);
                    }
                }
            }
            "user" => {
                let content = convert_openai_user_content_to_anthropic(msg.content.as_ref());
                anthropic_messages.push(AnthropicMessage {
                    role: "user".to_string(),
                    content,
                });
            }
            "assistant" => {
                let mut blocks = Vec::new();
                if let Some(ref c) = msg.content {
                    if let Value::String(s) = c {
                        if !s.is_empty() {
                            blocks.push(json!({ "type": "text", "text": s }));
                        }
                    }
                }
                if let Some(ref tcs) = msg.tool_calls {
                    for tc in tcs {
                        let parsed_input: Value = serde_json::from_str(&tc.function.arguments)
                            .unwrap_or_else(|_| json!({}));
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.function.name,
                            "input": parsed_input
                        }));
                    }
                }
                if !blocks.is_empty() {
                    anthropic_messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: Value::Array(blocks),
                    });
                }
            }
            "tool" => {
                let tool_use_id = msg.tool_call_id.clone().unwrap_or_default();
                let res_str = match &msg.content {
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                    None => "".to_string(),
                };
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": tool_use_id,
                    "content": res_str
                });
                // 并行 tool 消息必须合并入同一条 user 消息中，否则 Anthropic / Gemini 会拒绝连续多条 user 消息
                let appended = if let Some(last) = anthropic_messages.last_mut() {
                    if last.role == "user" {
                        if let Some(arr) = last.content.as_array_mut() {
                            let all_tool_results = arr
                                .iter()
                                .all(|b| b.get("type").and_then(Value::as_str) == Some("tool_result"));
                            if all_tool_results {
                                arr.push(block.clone());
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };
                if !appended {
                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: Value::Array(vec![block]),
                    });
                }
            }
            _ => {}
        }
    }

    let anthropic_tools = req.tools.as_ref().map(|tools| {
        tools
            .iter()
            .map(|t| AnthropicTool {
                name: t.function.name.clone(),
                description: t.function.description.clone(),
                input_schema: t.function.parameters.clone(),
            })
            .collect()
    });

    let thinking = if is_anthropic_thinking_supported(target_model) {
        if let Some(ref effort) = req.reasoning_effort {
            match effort.as_str() {
                "none" => None,
                "low" => Some(AnthropicThinkingConfig {
                    thinking_type: "enabled".to_string(),
                    budget_tokens: Some(2048),
                }),
                "high" => Some(AnthropicThinkingConfig {
                    thinking_type: "enabled".to_string(),
                    budget_tokens: Some(16384),
                }),
                "xhigh" | "max" | "ultra" => Some(AnthropicThinkingConfig {
                    thinking_type: "enabled".to_string(),
                    budget_tokens: Some(32768),
                }),
                _ => Some(AnthropicThinkingConfig {
                    thinking_type: "enabled".to_string(),
                    budget_tokens: Some(8192),
                }),
            }
        } else if target_model.contains("thinking") {
            Some(AnthropicThinkingConfig {
                thinking_type: "enabled".to_string(),
                budget_tokens: Some(8192),
            })
        } else {
            None
        }
    } else {
        None
    };

    let (temperature, max_tokens) = if let Some(ref th) = thinking {
        let budget = th.budget_tokens.unwrap_or(8192);
        let max_tok = req
            .max_completion_tokens
            .or(req.max_tokens)
            .unwrap_or(4096)
            .max(budget + 4096);
        (None, Some(max_tok))
    } else {
        (
            req.temperature,
            req.max_completion_tokens.or(req.max_tokens).or(Some(4096)),
        )
    };

    let tool_choice = convert_openai_tool_choice_to_anthropic(&req.tool_choice);

    AnthropicMessageRequest {
        model: target_model.to_string(),
        messages: anthropic_messages,
        system: if system_text.is_empty() {
            None
        } else {
            Some(Value::String(system_text))
        },
        max_tokens,
        temperature,
        tools: anthropic_tools,
        tool_choice,
        stream: req.stream,
        thinking,
        metadata: None,
    }
}

pub fn convert_openai_tool_choice_to_anthropic(choice: &Option<Value>) -> Option<Value> {
    match choice {
        Some(Value::String(s)) => match s.as_str() {
            "auto" => Some(json!({ "type": "auto" })),
            "required" => Some(json!({ "type": "any" })),
            "none" => None,
            _ => Some(json!({ "type": "auto" })),
        },
        Some(Value::Object(map)) => {
            let t = map.get("type").and_then(Value::as_str).unwrap_or("");
            if t == "function" {
                let name = map
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                Some(json!({
                    "type": "tool",
                    "name": name
                }))
            } else {
                Some(Value::Object(map.clone()))
            }
        }
        Some(other) => Some(other.clone()),
        None => None,
    }
}

pub fn convert_anthropic_resp_to_openai(
    resp: &AnthropicResponse,
    req_model: &str,
) -> OpenAIChatResponse {
    let mut text_content = String::new();
    let mut tool_calls = Vec::new();

    for block in &resp.content {
        let btype = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match btype {
            "text" => {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    text_content.push_str(t);
                }
            }
            "tool_use" => {
                let id = block
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("call_1")
                    .to_string();
                let name = block
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = block.get("input").cloned().unwrap_or(json!({}));
                let args_str = serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string());

                tool_calls.push(OpenAIToolCall {
                    index: Some(tool_calls.len()),
                    id,
                    call_type: "function".to_string(),
                    function: OpenAIFunctionCall {
                        name,
                        arguments: args_str,
                    },
                });
            }
            _ => {}
        }
    }

    let finish_reason = match resp.stop_reason.as_deref() {
        Some("end_turn") => Some("stop".to_string()),
        Some("tool_use") => Some("tool_calls".to_string()),
        Some("max_tokens") => Some("length".to_string()),
        _ => Some("stop".to_string()),
    };

    let message = OpenAIMessage {
        role: "assistant".to_string(),
        content: if text_content.is_empty() {
            None
        } else {
            Some(Value::String(text_content))
        },
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
        tool_call_id: None,
        reasoning_content: None,
        name: None,
    };

    let prompt_tokens = resp.usage.input_tokens
        + resp.usage.cache_read_input_tokens.unwrap_or(0)
        + resp.usage.cache_creation_input_tokens.unwrap_or(0);

    OpenAIChatResponse {
        id: resp.id.clone(),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp() as u64,
        model: req_model.to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message,
            finish_reason,
        }],
        usage: Some(OpenAIUsage {
            prompt_tokens,
            completion_tokens: resp.usage.output_tokens,
            total_tokens: prompt_tokens + resp.usage.output_tokens,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_openai_tool_choice_to_anthropic() {
        assert_eq!(
            convert_openai_tool_choice_to_anthropic(&Some(json!("none"))),
            None
        );
        assert_eq!(
            convert_openai_tool_choice_to_anthropic(&Some(json!("auto"))),
            Some(json!({"type": "auto"}))
        );
        assert_eq!(
            convert_openai_tool_choice_to_anthropic(&Some(json!("required"))),
            Some(json!({"type": "any"}))
        );
        assert_eq!(
            convert_openai_tool_choice_to_anthropic(&Some(json!({"type": "function", "function": {"name": "Bash"}}))),
            Some(json!({"type": "tool", "name": "Bash"}))
        );
        assert_eq!(
            convert_openai_tool_choice_to_anthropic(&None),
            None
        );
    }

    #[test]
    fn test_convert_openai_req_to_anthropic_thinking_filter() {
        let req = OpenAIChatRequest {
            model: "any".to_string(),
            messages: vec![],
            temperature: Some(0.7),
            max_tokens: Some(4096),
            max_completion_tokens: None,
            reasoning_effort: Some("high".to_string()),
            tools: None,
            tool_choice: None,
            stream: None,
            stream_options: None,
        };

        // claude-3-5-sonnet: thinking stripped
        let ant_35 = convert_openai_req_to_anthropic(&req, "claude-3-5-sonnet-20241022");
        assert!(ant_35.thinking.is_none());
        assert_eq!(ant_35.temperature, Some(0.7));

        // claude-3-7-sonnet: thinking mapped and temperature cleared
        let ant_37 = convert_openai_req_to_anthropic(&req, "claude-3-7-sonnet-20250219");
        assert!(ant_37.thinking.is_some());
        assert_eq!(ant_37.thinking.unwrap().budget_tokens, Some(16384));
        assert_eq!(ant_37.temperature, None);
    }

    #[test]
    fn test_merge_consecutive_tool_messages_into_single_user_message() {
        let req: OpenAIChatRequest = serde_json::from_value(json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {"role": "user", "content": "Check files"},
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {"id": "call_1", "type": "function", "function": {"name": "read", "arguments": "{}"}},
                        {"id": "call_2", "type": "function", "function": {"name": "list", "arguments": "{}"}}
                    ]
                },
                {"role": "tool", "tool_call_id": "call_1", "content": "result_one"},
                {"role": "tool", "tool_call_id": "call_2", "content": "result_two"}
            ]
        }))
        .unwrap();

        let ant = convert_openai_req_to_anthropic(&req, "claude-sonnet-4-6");
        assert_eq!(ant.messages.len(), 3);
        assert_eq!(ant.messages[2].role, "user");
        let blocks = ant.messages[2].content.as_array().unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "tool_result");
        assert_eq!(blocks[0]["tool_use_id"], "call_1");
        assert_eq!(blocks[1]["tool_use_id"], "call_2");
    }

    #[test]
    fn test_convert_openai_image_url_and_cached_usage() {
        let req: OpenAIChatRequest = serde_json::from_value(json!({
            "model": "claude-sonnet-4-6",
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": "What is this?" },
                    { "type": "image_url", "image_url": { "url": "data:image/png;base64,QUFB" } },
                    { "type": "image_url", "image_url": { "url": "https://example.com/pic.webp" } }
                ]
            }]
        }))
        .unwrap();

        let ant = convert_openai_req_to_anthropic(&req, "claude-sonnet-4-6");
        let blocks = ant.messages[0].content.as_array().unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1]["type"], "image");
        assert_eq!(blocks[1]["source"]["type"], "base64");
        assert_eq!(blocks[1]["source"]["media_type"], "image/png");
        assert_eq!(blocks[1]["source"]["data"], "QUFB");
        assert_eq!(blocks[2]["source"]["type"], "url");
        assert_eq!(blocks[2]["source"]["url"], "https://example.com/pic.webp");

        let ant_resp = AnthropicResponse {
            id: "msg_1".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            content: vec![json!({ "type": "text", "text": "ok" })],
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: AnthropicUsage {
                input_tokens: 1000,
                output_tokens: 50,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: Some(4000),
            },
        };
        let oai_resp = convert_anthropic_resp_to_openai(&ant_resp, "claude-sonnet-4-6");
        let u = oai_resp.usage.unwrap();
        assert_eq!(u.prompt_tokens, 5000);
        assert_eq!(u.total_tokens, 5050);
    }
}
