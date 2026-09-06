use crate::gateway::types::*;
use serde_json::{json, Value};

pub fn convert_anthropic_req_to_gemini(req: &AnthropicMessageRequest) -> GeminiRequest {
    let mut contents = Vec::new();

    for msg in &req.messages {
        let role = if msg.role == "assistant" { "model" } else { "user" };
        let mut parts = Vec::new();

        match &msg.content {
            Value::String(s) => {
                parts.push(GeminiPart {
                    text: Some(s.clone()),
                    function_call: None,
                    function_response: None,
                });
            }
            Value::Array(blocks) => {
                for block in blocks {
                    let btype = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match btype {
                        "text" => {
                            if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                                parts.push(GeminiPart {
                                    text: Some(t.to_string()),
                                    function_call: None,
                                    function_response: None,
                                });
                            }
                        }
                        "tool_use" => {
                            let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let args = block.get("input").cloned().unwrap_or(json!({}));
                            parts.push(GeminiPart {
                                text: None,
                                function_call: Some(json!({
                                    "name": name,
                                    "args": args
                                })),
                                function_response: None,
                            });
                        }
                        "tool_result" => {
                            let content_val = block.get("content");
                            let res_text = match content_val {
                                Some(Value::String(s)) => s.clone(),
                                Some(other) => other.to_string(),
                                None => "".to_string(),
                            };
                            parts.push(GeminiPart {
                                text: None,
                                function_call: None,
                                function_response: Some(json!({
                                    "name": block.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("tool"),
                                    "response": { "result": res_text }
                                })),
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        if !parts.is_empty() {
            contents.push(GeminiContent {
                role: role.to_string(),
                parts,
            });
        }
    }

    let system_instruction = req.system.as_ref().map(|sys| {
        let text = match sys {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart {
                text: Some(text),
                function_call: None,
                function_response: None,
            }],
        }
    });

    let tools = req.tools.as_ref().map(|tools| {
        let declarations: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema
                })
            })
            .collect();
        vec![json!({ "functionDeclarations": declarations })]
    });

    let mut gen_config = json!({});
    if let Some(t) = req.temperature {
        gen_config["temperature"] = json!(t);
    }
    if let Some(mt) = req.max_tokens {
        gen_config["maxOutputTokens"] = json!(mt);
    }

    GeminiRequest {
        contents,
        system_instruction,
        tools,
        generation_config: Some(gen_config),
    }
}

pub fn convert_gemini_resp_to_anthropic(resp: &GeminiResponse, req_model: &str) -> AnthropicResponse {
    let mut content_blocks = Vec::new();
    let mut stop_reason = Some("end_turn".to_string());

    if let Some(candidates) = &resp.candidates {
        if let Some(cand) = candidates.first() {
            if let Some(ref content) = cand.content {
                for part in &content.parts {
                    if let Some(ref t) = part.text {
                        content_blocks.push(json!({
                            "type": "text",
                            "text": t
                        }));
                    }
                    if let Some(ref fc) = part.function_call {
                        let name = fc.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let args = fc.get("args").cloned().unwrap_or(json!({}));
                        content_blocks.push(json!({
                            "type": "tool_use",
                            "id": format!("call_{}", uuid::Uuid::new_v4()),
                            "name": name,
                            "input": args
                        }));
                        stop_reason = Some("tool_use".to_string());
                    }
                }
            }
        }
    }

    let usage = resp.usage_metadata.as_ref().map(|u| AnthropicUsage {
        input_tokens: u.prompt_token_count.unwrap_or(0),
        output_tokens: u.candidates_token_count.unwrap_or(0),
    }).unwrap_or(AnthropicUsage {
        input_tokens: 0,
        output_tokens: 0,
    });

    AnthropicResponse {
        id: format!("msg_{}", uuid::Uuid::new_v4()),
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        model: req_model.to_string(),
        content: content_blocks,
        stop_reason,
        stop_sequence: None,
        usage,
    }
}
