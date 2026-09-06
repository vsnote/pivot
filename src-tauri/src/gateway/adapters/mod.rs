pub mod anthropic_to_openai;
pub mod anthropic_to_gemini;
pub mod openai_to_anthropic;

use crate::gateway::types::*;
use serde_json::{json, Value};

#[derive(Default)]
pub struct SseState {
    pub message_started: bool,
    pub current_block_index: usize,
    pub in_thinking_block: bool,
    pub in_text_block: bool,
    pub in_tool_block: bool,
    pub current_tool_id: Option<String>,
    pub current_tool_name: Option<String>,
    pub total_output_tokens: u32,
}

impl SseState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Convert an OpenAI SSE line (`data: {...}`) to Anthropic SSE events
pub fn parse_openai_chunk_to_anthropic_events(
    line: &str,
    state: &mut SseState,
    msg_id: &str,
    model: &str,
) -> Vec<String> {
    let mut events = Vec::new();
    let trimmed = line.trim();

    if !trimmed.starts_with("data:") {
        return events;
    }

    let payload = trimmed.trim_start_matches("data:").trim();
    if payload == "[DONE]" {
        if state.in_thinking_block {
            events.push(format!(
                "event: content_block_stop\ndata: {}\n\n",
                json!({ "type": "content_block_stop", "index": state.current_block_index })
            ));
            state.in_thinking_block = false;
            state.current_block_index += 1;
        }
        if state.in_text_block {
            events.push(format!(
                "event: content_block_stop\ndata: {}\n\n",
                json!({ "type": "content_block_stop", "index": state.current_block_index })
            ));
            state.in_text_block = false;
            state.current_block_index += 1;
        }
        if state.in_tool_block {
            events.push(format!(
                "event: content_block_stop\ndata: {}\n\n",
                json!({ "type": "content_block_stop", "index": state.current_block_index })
            ));
            state.in_tool_block = false;
            state.current_block_index += 1;
        }

        events.push(format!(
            "event: message_delta\ndata: {}\n\n",
            json!({
                "type": "message_delta",
                "delta": { "stop_reason": "end_turn", "stop_sequence": null },
                "usage": { "output_tokens": state.total_output_tokens }
            })
        ));
        events.push("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string());
        return events;
    }

    let chunk: OpenAIChatChunk = match serde_json::from_str(payload) {
        Ok(c) => c,
        Err(_) => return events,
    };

    // 1. Emit message_start if not started
    if !state.message_started {
        state.message_started = true;
        let start_event = json!({
            "type": "message_start",
            "message": {
                "id": format!("msg_{}", msg_id),
                "type": "message",
                "role": "assistant",
                "model": model,
                "content": [],
                "stop_reason": null,
                "stop_sequence": null,
                "usage": { "input_tokens": 0, "output_tokens": 0 }
            }
        });
        events.push(format!("event: message_start\ndata: {}\n\n", start_event));
    }

    if let Some(choice) = chunk.choices.first() {
        // Handle Reasoning / Thinking Tokens (e.g. DeepSeek R1 reasoning_content or <think>)
        if let Some(ref reasoning) = choice.delta.reasoning_content {
            if !reasoning.is_empty() {
                if !state.in_thinking_block {
                    if state.in_text_block {
                        events.push(format!(
                            "event: content_block_stop\ndata: {}\n\n",
                            json!({ "type": "content_block_stop", "index": state.current_block_index })
                        ));
                        state.in_text_block = false;
                        state.current_block_index += 1;
                    }
                    state.in_thinking_block = true;
                    events.push(format!(
                        "event: content_block_start\ndata: {}\n\n",
                        json!({
                            "type": "content_block_start",
                            "index": state.current_block_index,
                            "content_block": { "type": "thinking", "thinking": "" }
                        })
                    ));
                }

                events.push(format!(
                    "event: content_block_delta\ndata: {}\n\n",
                    json!({
                        "type": "content_block_delta",
                        "index": state.current_block_index,
                        "delta": { "type": "thinking_delta", "thinking": reasoning }
                    })
                ));
                state.total_output_tokens += 1;
            }
        }

        // Handle normal Text Content
        if let Some(ref text) = choice.delta.content {
            if !text.is_empty() {
                // If we were in thinking block, close it
                if state.in_thinking_block {
                    events.push(format!(
                        "event: content_block_stop\ndata: {}\n\n",
                        json!({ "type": "content_block_stop", "index": state.current_block_index })
                    ));
                    state.in_thinking_block = false;
                    state.current_block_index += 1;
                }

                if !state.in_text_block {
                    state.in_text_block = true;
                    events.push(format!(
                        "event: content_block_start\ndata: {}\n\n",
                        json!({
                            "type": "content_block_start",
                            "index": state.current_block_index,
                            "content_block": { "type": "text", "text": "" }
                        })
                    ));
                }

                events.push(format!(
                    "event: content_block_delta\ndata: {}\n\n",
                    json!({
                        "type": "content_block_delta",
                        "index": state.current_block_index,
                        "delta": { "type": "text_delta", "text": text }
                    })
                ));
                state.total_output_tokens += 1;
            }
        }

        // Handle Tool Calls
        if let Some(ref tool_chunks) = choice.delta.tool_calls {
            for tc in tool_chunks {
                if let Some(ref id) = tc.id {
                    // Start of a tool call
                    if state.in_text_block {
                        events.push(format!(
                            "event: content_block_stop\ndata: {}\n\n",
                            json!({ "type": "content_block_stop", "index": state.current_block_index })
                        ));
                        state.in_text_block = false;
                        state.current_block_index += 1;
                    }
                    if state.in_tool_block {
                        events.push(format!(
                            "event: content_block_stop\ndata: {}\n\n",
                            json!({ "type": "content_block_stop", "index": state.current_block_index })
                        ));
                        state.current_block_index += 1;
                    }

                    let name = tc.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default();
                    state.in_tool_block = true;
                    state.current_tool_id = Some(id.clone());
                    state.current_tool_name = Some(name.clone());

                    events.push(format!(
                        "event: content_block_start\ndata: {}\n\n",
                        json!({
                            "type": "content_block_start",
                            "index": state.current_block_index,
                            "content_block": {
                                "type": "tool_use",
                                "id": id,
                                "name": name
                            }
                        })
                    ));
                }

                if let Some(ref func) = tc.function {
                    if let Some(ref args) = func.arguments {
                        if !args.is_empty() {
                            events.push(format!(
                                "event: content_block_delta\ndata: {}\n\n",
                                json!({
                                    "type": "content_block_delta",
                                    "index": state.current_block_index,
                                    "delta": {
                                        "type": "input_json_delta",
                                        "partial_json": args
                                    }
                                })
                            ));
                        }
                    }
                }
            }
        }

        // Check finish_reason
        if let Some(ref finish_reason) = choice.finish_reason {
            if state.in_text_block {
                events.push(format!(
                    "event: content_block_stop\ndata: {}\n\n",
                    json!({ "type": "content_block_stop", "index": state.current_block_index })
                ));
                state.in_text_block = false;
                state.current_block_index += 1;
            }
            if state.in_thinking_block {
                events.push(format!(
                    "event: content_block_stop\ndata: {}\n\n",
                    json!({ "type": "content_block_stop", "index": state.current_block_index })
                ));
                state.in_thinking_block = false;
                state.current_block_index += 1;
            }
            if state.in_tool_block {
                events.push(format!(
                    "event: content_block_stop\ndata: {}\n\n",
                    json!({ "type": "content_block_stop", "index": state.current_block_index })
                ));
                state.in_tool_block = false;
                state.current_block_index += 1;
            }

            let stop_reason = match finish_reason.as_str() {
                "stop" => "end_turn",
                "tool_calls" => "tool_use",
                "length" => "max_tokens",
                _ => "end_turn",
            };

            let out_tokens = chunk.usage.as_ref().map(|u| u.completion_tokens).unwrap_or(state.total_output_tokens);
            events.push(format!(
                "event: message_delta\ndata: {}\n\n",
                json!({
                    "type": "message_delta",
                    "delta": { "stop_reason": stop_reason, "stop_sequence": null },
                    "usage": { "output_tokens": out_tokens }
                })
            ));
            events.push("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string());
        }
    }

    events
}

/// Convert an Anthropic SSE event to OpenAI SSE chunk format (`data: {...}\n\n`)
pub fn parse_anthropic_event_to_openai_chunks(
    line: &str,
    msg_id: &str,
    model: &str,
) -> Vec<String> {
    let mut chunks = Vec::new();
    let trimmed = line.trim();

    if !trimmed.starts_with("data:") {
        return chunks;
    }

    let payload = trimmed.trim_start_matches("data:").trim();
    let val: Value = match serde_json::from_str(payload) {
        Ok(v) => v,
        Err(_) => return chunks,
    };

    let event_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match event_type {
        "content_block_delta" => {
            let delta = val.get("delta");
            let delta_type = delta.and_then(|d| d.get("type")).and_then(|t| t.as_str()).unwrap_or("");
            if delta_type == "text_delta" {
                if let Some(text) = delta.and_then(|d| d.get("text")).and_then(|t| t.as_str()) {
                    let chunk = json!({
                        "id": msg_id,
                        "object": "chat.completion.chunk",
                        "created": chrono::Utc::now().timestamp(),
                        "model": model,
                        "choices": [{
                            "index": 0,
                            "delta": { "content": text },
                            "finish_reason": null
                        }]
                    });
                    chunks.push(format!("data: {}\n\n", chunk));
                }
            } else if delta_type == "input_json_delta" {
                if let Some(partial) = delta.and_then(|d| d.get("partial_json")).and_then(|p| p.as_str()) {
                    let chunk = json!({
                        "id": msg_id,
                        "object": "chat.completion.chunk",
                        "created": chrono::Utc::now().timestamp(),
                        "model": model,
                        "choices": [{
                            "index": 0,
                            "delta": {
                                "tool_calls": [{
                                    "index": 0,
                                    "function": { "arguments": partial }
                                }]
                            },
                            "finish_reason": null
                        }]
                    });
                    chunks.push(format!("data: {}\n\n", chunk));
                }
            }
        }
        "message_delta" => {
            let stop_reason = val.get("delta").and_then(|d| d.get("stop_reason")).and_then(|s| s.as_str()).unwrap_or("stop");
            let finish_reason = match stop_reason {
                "tool_use" => "tool_calls",
                "max_tokens" => "length",
                _ => "stop",
            };
            let chunk = json!({
                "id": msg_id,
                "object": "chat.completion.chunk",
                "created": chrono::Utc::now().timestamp(),
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": finish_reason
                }]
            });
            chunks.push(format!("data: {}\n\n", chunk));
        }
        "message_stop" => {
            chunks.push("data: [DONE]\n\n".to_string());
        }
        _ => {}
    }

    chunks
}
