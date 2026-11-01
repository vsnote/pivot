use crate::gateway::types::*;
use serde_json::{json, Value};

pub fn convert_openai_req_to_anthropic(
    req: &OpenAIChatRequest,
    target_model: &str,
) -> AnthropicMessageRequest {
    let mut system_text = String::new();
    let mut anthropic_messages = Vec::new();

    for msg in &req.messages {
        match msg.role.as_str() {
            "system" => {
                if let Some(ref c) = msg.content {
                    if let Value::String(s) = c {
                        if !system_text.is_empty() {
                            system_text.push('\n');
                        }
                        system_text.push_str(s);
                    }
                }
            }
            "user" => {
                let content = msg.content.clone().unwrap_or(Value::String("".to_string()));
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
                anthropic_messages.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: Value::Array(vec![block]),
                });
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

    let thinking = if let Some(ref effort) = req.reasoning_effort {
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
            "xhigh" | "max" => Some(AnthropicThinkingConfig {
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
            prompt_tokens: resp.usage.input_tokens,
            completion_tokens: resp.usage.output_tokens,
            total_tokens: resp.usage.input_tokens + resp.usage.output_tokens,
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
}
