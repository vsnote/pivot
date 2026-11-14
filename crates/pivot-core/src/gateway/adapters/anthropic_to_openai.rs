use crate::gateway::types::*;
use serde_json::{json, Value};

fn anthropic_image_to_openai_part(block: &Value) -> Option<Value> {
    let source = block.get("source")?;
    let source_type = source.get("type").and_then(Value::as_str).unwrap_or("");
    match source_type {
        "base64" => {
            let data = source.get("data").and_then(Value::as_str)?;
            if data.is_empty() {
                return None;
            }
            let media_type = source
                .get("media_type")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or("image/png");
            Some(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", media_type, data)
                }
            }))
        }
        "url" => {
            let url = source.get("url").and_then(Value::as_str)?;
            if url.is_empty() {
                return None;
            }
            Some(json!({
                "type": "image_url",
                "image_url": {
                    "url": url
                }
            }))
        }
        _ => None,
    }
}

pub fn convert_anthropic_req_to_openai(
    req: &AnthropicMessageRequest,
    target_model: &str,
) -> OpenAIChatRequest {
    let mut openai_messages = Vec::new();

    // 1. Convert System prompt
    if let Some(ref sys) = req.system {
        let sys_content = match sys {
            Value::String(s) => s.clone(),
            Value::Array(arr) => {
                let mut text = String::new();
                for item in arr {
                    if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                        text.push_str(t);
                    }
                }
                text
            }
            _ => sys.to_string(),
        };
        if !sys_content.is_empty() {
            openai_messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: Some(Value::String(sys_content)),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
                name: None,
            });
        }
    }

    // 2. Convert Messages
    for msg in &req.messages {
        match &msg.content {
            Value::String(text) => {
                openai_messages.push(OpenAIMessage {
                    role: msg.role.clone(),
                    content: Some(Value::String(text.clone())),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                    name: None,
                });
            }
            Value::Array(blocks) => {
                let mut text_parts = String::new();
                let mut content_parts: Vec<Value> = Vec::new();
                let mut has_image = false;
                let mut pending_tool_images: Vec<Value> = Vec::new();
                let mut tool_calls = Vec::new();
                let mut thinking_parts = String::new();

                for block in blocks {
                    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match block_type {
                        "text" => {
                            if let Some(t) = block.get("text").and_then(|s| s.as_str()) {
                                text_parts.push_str(t);
                                content_parts.push(json!({ "type": "text", "text": t }));
                            }
                        }
                        "image" => {
                            if let Some(img_part) = anthropic_image_to_openai_part(block) {
                                content_parts.push(img_part);
                                has_image = true;
                            }
                        }
                        "thinking" => {
                            if let Some(t) = block.get("thinking").and_then(|s| s.as_str()) {
                                thinking_parts.push_str(t);
                            }
                        }
                        "tool_use" => {
                            let id = block
                                .get("id")
                                .and_then(|s| s.as_str())
                                .unwrap_or("call_default")
                                .to_string();
                            let name = block
                                .get("name")
                                .and_then(|s| s.as_str())
                                .unwrap_or("")
                                .to_string();
                            let input = block.get("input").cloned().unwrap_or(json!({}));
                            let input_str =
                                serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string());

                            tool_calls.push(OpenAIToolCall {
                                index: Some(tool_calls.len()),
                                id,
                                call_type: "function".to_string(),
                                function: OpenAIFunctionCall {
                                    name,
                                    arguments: input_str,
                                },
                            });
                        }
                        "tool_result" => {
                            let tool_use_id = block
                                .get("tool_use_id")
                                .and_then(|s| s.as_str())
                                .unwrap_or("")
                                .to_string();
                            let content_val = block.get("content");
                            let res_text = match content_val {
                                Some(Value::String(s)) => s.clone(),
                                Some(Value::Array(arr)) => {
                                    let mut s = String::new();
                                    let mut had_image = false;
                                    for item in arr {
                                        let item_type = item
                                            .get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        if item_type == "image" {
                                            if let Some(img_part) =
                                                anthropic_image_to_openai_part(item)
                                            {
                                                pending_tool_images.push(img_part);
                                                had_image = true;
                                            }
                                        } else if let Some(t) =
                                            item.get("text").and_then(|x| x.as_str())
                                        {
                                            if !s.is_empty() && !t.is_empty() {
                                                s.push('\n');
                                            }
                                            s.push_str(t);
                                        }
                                    }
                                    if s.is_empty() && had_image {
                                        "[Image content attached]".to_string()
                                    } else {
                                        s
                                    }
                                }
                                Some(other) => other.to_string(),
                                None => "".to_string(),
                            };

                            openai_messages.push(OpenAIMessage {
                                role: "tool".to_string(),
                                content: Some(Value::String(res_text)),
                                tool_calls: None,
                                tool_call_id: Some(tool_use_id),
                                reasoning_content: None,
                                name: None,
                            });
                        }
                        _ => {}
                    }
                }

                let reasoning_content = if !thinking_parts.is_empty() {
                    Some(thinking_parts)
                } else {
                    None
                };

                if !text_parts.is_empty()
                    || has_image
                    || !tool_calls.is_empty()
                    || reasoning_content.is_some()
                {
                    let content = if has_image {
                        Some(Value::Array(content_parts))
                    } else if text_parts.is_empty() {
                        None
                    } else {
                        Some(Value::String(text_parts))
                    };
                    openai_messages.push(OpenAIMessage {
                        role: msg.role.clone(),
                        content,
                        tool_calls: if tool_calls.is_empty() {
                            None
                        } else {
                            Some(tool_calls)
                        },
                        tool_call_id: None,
                        reasoning_content,
                        name: None,
                    });
                }

                // 必须在所有并行 role: "tool" 消息推完后再追加 user 图片消息，避免破坏 OpenAI 工具调用消息顺序约束
                if !pending_tool_images.is_empty() {
                    openai_messages.push(OpenAIMessage {
                        role: "user".to_string(),
                        content: Some(Value::Array(pending_tool_images)),
                        tool_calls: None,
                        tool_call_id: None,
                        reasoning_content: None,
                        name: None,
                    });
                }
            }
            _ => {}
        }
    }

    // 3. Convert Tools
    let openai_tools = req.tools.as_ref().map(|tools| {
        tools
            .iter()
            .map(|t| OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunction {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                },
            })
            .collect()
    });

    let reasoning_effort = if let Some(ref th) = req.thinking {
        // Only map reasoning_effort for models that officially support it (e.g. OpenAI o1/o3)
        // DeepSeek and standard models reject unexpected reasoning_effort values with 400 Bad Request
        let is_o_series = target_model.contains("o1") || target_model.contains("o3");
        if is_o_series && th.thinking_type == "enabled" {
            let budget = th.budget_tokens.unwrap_or(8192);
            if budget <= 3000 {
                Some("low".to_string())
            } else if budget <= 12000 {
                Some("medium".to_string())
            } else {
                Some("high".to_string())
            }
        } else {
            None
        }
    } else {
        None
    };

    let tool_choice = convert_anthropic_tool_choice_to_openai(&req.tool_choice);

    OpenAIChatRequest {
        model: target_model.to_string(),
        messages: openai_messages,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        max_completion_tokens: None,
        reasoning_effort,
        tools: openai_tools,
        tool_choice,
        stream: req.stream,
        stream_options: req
            .stream
            .filter(|s| *s)
            .map(|_| json!({ "include_usage": true })),
    }
}

pub fn convert_anthropic_tool_choice_to_openai(choice: &Option<Value>) -> Option<Value> {
    match choice {
        Some(Value::Object(map)) => {
            let t = map.get("type").and_then(Value::as_str).unwrap_or("");
            match t {
                "auto" => Some(json!("auto")),
                "any" => Some(json!("required")),
                "tool" => {
                    let name = map.get("name").and_then(Value::as_str).unwrap_or("");
                    Some(json!({
                        "type": "function",
                        "function": { "name": name }
                    }))
                }
                _ => Some(Value::Object(map.clone())),
            }
        }
        Some(Value::String(s)) => Some(Value::String(s.clone())),
        Some(other) => Some(other.clone()),
        None => None,
    }
}

pub fn convert_openai_resp_to_anthropic(
    resp: &OpenAIChatResponse,
    req_model: &str,
) -> AnthropicResponse {
    let mut content_blocks = Vec::new();
    let mut stop_reason = Some("end_turn".to_string());

    if let Some(choice) = resp.choices.first() {
        if let Some(reasoning) = &choice.message.reasoning_content {
            if !reasoning.is_empty() {
                content_blocks.push(json!({
                    "type": "thinking",
                    "thinking": reasoning,
                    "signature": ""
                }));
            }
        }

        if let Some(Value::String(ref text)) = choice.message.content {
            if !text.is_empty() {
                content_blocks.push(json!({
                    "type": "text",
                    "text": text
                }));
            }
        }

        if let Some(ref tool_calls) = choice.message.tool_calls {
            for tc in tool_calls {
                let parsed_input: Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or_else(|_| json!({}));
                content_blocks.push(json!({
                    "type": "tool_use",
                    "id": tc.id,
                    "name": tc.function.name,
                    "input": parsed_input
                }));
            }
            stop_reason = Some("tool_use".to_string());
        }

        if let Some(ref fr) = choice.finish_reason {
            match fr.as_str() {
                "stop" => stop_reason = Some("end_turn".to_string()),
                "tool_calls" => stop_reason = Some("tool_use".to_string()),
                "length" => stop_reason = Some("max_tokens".to_string()),
                _ => stop_reason = Some("end_turn".to_string()),
            }
        }
    }

    let usage = resp
        .usage
        .as_ref()
        .map(|u| AnthropicUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            ..Default::default()
        })
        .unwrap_or_default();

    AnthropicResponse {
        id: format!("msg_{}", resp.id),
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        model: req_model.to_string(),
        content: content_blocks,
        stop_reason,
        stop_sequence: None,
        usage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_to_openai_request() {
        let req = AnthropicMessageRequest {
            model: "claude-3-7-sonnet".to_string(),
            messages: vec![
                AnthropicMessage {
                    role: "user".to_string(),
                    content: Value::String("Hello Pivot".to_string()),
                },
                AnthropicMessage {
                    role: "assistant".to_string(),
                    content: Value::Array(vec![json!({
                        "type": "tool_use",
                        "id": "call_123",
                        "name": "bash",
                        "input": { "command": "ls -la" }
                    })]),
                },
                AnthropicMessage {
                    role: "user".to_string(),
                    content: Value::Array(vec![json!({
                        "type": "tool_result",
                        "tool_use_id": "call_123",
                        "content": "file1.txt\nfile2.txt"
                    })]),
                },
            ],
            system: Some(Value::String(
                "You are a helpful coding assistant.".to_string(),
            )),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            tools: Some(vec![AnthropicTool {
                name: "bash".to_string(),
                description: Some("Run a shell command".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" }
                    }
                }),
            }]),
            tool_choice: None,
            stream: Some(true),
            thinking: None,
            metadata: None,
        };

        let oai_req = convert_anthropic_req_to_openai(&req, "deepseek-chat");
        assert_eq!(oai_req.model, "deepseek-chat");
        assert_eq!(oai_req.messages[0].role, "system");
        assert_eq!(oai_req.messages[1].role, "user");
        assert_eq!(oai_req.messages[2].role, "assistant");
        assert!(oai_req.messages[2].tool_calls.is_some());
        assert_eq!(oai_req.messages[3].role, "tool");
        assert_eq!(
            oai_req.messages[3].tool_call_id,
            Some("call_123".to_string())
        );
        assert_eq!(oai_req.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_anthropic_tool_choice_to_openai() {
        assert_eq!(
            convert_anthropic_tool_choice_to_openai(&Some(json!({"type": "auto"}))),
            Some(json!("auto"))
        );
        assert_eq!(
            convert_anthropic_tool_choice_to_openai(&Some(json!({"type": "any"}))),
            Some(json!("required"))
        );
        assert_eq!(
            convert_anthropic_tool_choice_to_openai(&Some(json!({"type": "tool", "name": "Bash"}))),
            Some(json!({"type": "function", "function": {"name": "Bash"}}))
        );
        assert_eq!(
            convert_anthropic_tool_choice_to_openai(&None),
            None
        );
    }

    #[test]
    fn test_anthropic_to_openai_preserves_thinking_block() {
        let req = AnthropicMessageRequest {
            model: "claude-3-7-sonnet".to_string(),
            messages: vec![
                AnthropicMessage {
                    role: "user".to_string(),
                    content: Value::String("Please think".to_string()),
                },
                AnthropicMessage {
                    role: "assistant".to_string(),
                    content: Value::Array(vec![
                        json!({
                            "type": "thinking",
                            "thinking": "Deep internal reasoning...",
                            "signature": "sig123"
                        }),
                        json!({
                            "type": "text",
                            "text": "Final answer"
                        }),
                    ]),
                },
            ],
            system: None,
            max_tokens: Some(2048),
            temperature: None,
            tools: None,
            tool_choice: None,
            stream: None,
            thinking: None,
            metadata: None,
        };

        let oai_req = convert_anthropic_req_to_openai(&req, "deepseek-reasoner");
        assert_eq!(oai_req.messages.len(), 2);
        assert_eq!(oai_req.messages[1].role, "assistant");
        assert_eq!(
            oai_req.messages[1].reasoning_content,
            Some("Deep internal reasoning...".to_string())
        );
        assert_eq!(
            oai_req.messages[1].content,
            Some(Value::String("Final answer".to_string()))
        );
        // Only max_tokens should be set, not max_completion_tokens
        assert_eq!(oai_req.max_tokens, Some(2048));
        assert_eq!(oai_req.max_completion_tokens, None);
    }

    #[test]
    fn test_anthropic_to_openai_image_and_parallel_tool_images() {
        let b64 = "AAAA";
        let req = AnthropicMessageRequest {
            model: "gpt-6-astra".to_string(),
            messages: vec![
                AnthropicMessage {
                    role: "user".to_string(),
                    content: json!([
                        { "type": "text", "text": "See image:" },
                        { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": b64 } }
                    ]),
                },
                AnthropicMessage {
                    role: "user".to_string(),
                    content: json!([
                        {
                            "type": "tool_result",
                            "tool_use_id": "call_1",
                            "content": [
                                { "type": "text", "text": "file text" },
                                { "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": b64 } }
                            ]
                        },
                        {
                            "type": "tool_result",
                            "tool_use_id": "call_2",
                            "content": "second tool text"
                        }
                    ]),
                },
            ],
            system: None,
            max_tokens: None,
            temperature: None,
            tools: None,
            tool_choice: None,
            stream: None,
            thinking: None,
            metadata: None,
        };

        let oai = convert_anthropic_req_to_openai(&req, "gpt-6-astra");
        // msg 0: user multimodal array
        let m0 = oai.messages[0].content.as_ref().unwrap().as_array().unwrap();
        assert_eq!(m0[0]["type"], "text");
        assert_eq!(m0[1]["type"], "image_url");
        assert_eq!(m0[1]["image_url"]["url"], "data:image/png;base64,AAAA");

        // msg 1 & msg 2: tool messages stay text-only and contiguous; msg 3: appended user image message
        assert_eq!(oai.messages.len(), 4);
        assert_eq!(oai.messages[1].role, "tool");
        assert_eq!(oai.messages[1].content, Some(Value::String("file text".to_string())));
        assert_eq!(oai.messages[2].role, "tool");
        assert_eq!(oai.messages[2].content, Some(Value::String("second tool text".to_string())));
        assert_eq!(oai.messages[3].role, "user");
        let m3 = oai.messages[3].content.as_ref().unwrap().as_array().unwrap();
        assert_eq!(m3[0]["type"], "image_url");
        assert_eq!(m3[0]["image_url"]["url"], "data:image/jpeg;base64,AAAA");
    }
}
