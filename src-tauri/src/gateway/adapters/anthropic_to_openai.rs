use crate::gateway::types::*;
use serde_json::{json, Value};

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
                let mut tool_calls = Vec::new();

                for block in blocks {
                    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match block_type {
                        "text" => {
                            if let Some(t) = block.get("text").and_then(|s| s.as_str()) {
                                text_parts.push_str(t);
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
                                    for item in arr {
                                        if let Some(t) = item.get("text").and_then(|x| x.as_str()) {
                                            s.push_str(t);
                                        }
                                    }
                                    s
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

                if !text_parts.is_empty() || !tool_calls.is_empty() {
                    openai_messages.push(OpenAIMessage {
                        role: msg.role.clone(),
                        content: if text_parts.is_empty() {
                            None
                        } else {
                            Some(Value::String(text_parts))
                        },
                        tool_calls: if tool_calls.is_empty() {
                            None
                        } else {
                            Some(tool_calls)
                        },
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

    OpenAIChatRequest {
        model: target_model.to_string(),
        messages: openai_messages,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        max_completion_tokens: req.max_tokens,
        tools: openai_tools,
        tool_choice: req.tool_choice.clone(),
        stream: req.stream,
        stream_options: req
            .stream
            .filter(|s| *s)
            .map(|_| json!({ "include_usage": true })),
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
        })
        .unwrap_or(AnthropicUsage {
            input_tokens: 0,
            output_tokens: 0,
        });

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
                    content: Value::String("Hello OmniSwitch".to_string()),
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
}
