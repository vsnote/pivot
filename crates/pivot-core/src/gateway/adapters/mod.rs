pub mod anthropic_to_gemini;
pub mod anthropic_to_openai;
pub mod openai_to_anthropic;
pub mod responses;

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
    pub current_tool_raw_index: Option<usize>,
    pub last_thought_signature: Option<String>,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub pending_stop_reason: Option<String>,
    pub stream_finished: bool,
}

#[derive(Default)]
pub struct AnthropicToOpenAiSseState {
    pub tool_index_map: std::collections::HashMap<usize, usize>,
    pub next_tool_index: usize,
}

impl AnthropicToOpenAiSseState {
    pub fn alloc_tool_index(&mut self, block_index: usize) -> usize {
        if let Some(&idx) = self.tool_index_map.get(&block_index) {
            idx
        } else {
            let idx = self.next_tool_index;
            self.next_tool_index += 1;
            self.tool_index_map.insert(block_index, idx);
            idx
        }
    }
}

/// 跨 TCP 分块按字节累积行缓冲后再解码 UTF-8，彻底消除多字节中文/Emoji 在网络分片边界被拆断丢失的问题
#[derive(Default)]
pub struct SseLineBuffer {
    buf: Vec<u8>,
}

impl SseLineBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_and_read_lines(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let mut end = pos;
            if end > 0 && self.buf[end - 1] == b'\r' {
                end -= 1;
            }
            lines.push(String::from_utf8_lossy(&self.buf[..end]).into_owned());
            self.buf.drain(..=pos);
        }
        lines
    }

    pub fn finish(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            return None;
        }
        if self.buf.last() == Some(&b'\r') {
            self.buf.pop();
        }
        let rest = String::from_utf8_lossy(&self.buf).into_owned();
        self.buf.clear();
        Some(rest)
    }
}

impl SseState {
    pub fn new() -> Self {
        Self::default()
    }
}

fn close_active_anthropic_blocks(events: &mut Vec<String>, state: &mut SseState) {
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
}

/// Convert a Gemini streaming response line to Anthropic SSE events.
/// Gemini emits one JSON object per `data:` line and does not use a `[DONE]`
/// sentinel, so the caller must invoke `finish_gemini_stream` when the body
/// ends if the upstream did not include a finish reason.
pub fn parse_gemini_chunk_to_anthropic_events(
    line: &str,
    state: &mut SseState,
    msg_id: &str,
    model: &str,
) -> Vec<String> {
    let mut events = Vec::new();
    if state.stream_finished {
        return events;
    }

    let trimmed = line.trim();
    if !trimmed.starts_with("data:") {
        return events;
    }
    let payload = trimmed.trim_start_matches("data:").trim();
    let value: Value = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(_) => return events,
    };

    let actual_val = value.get("response").unwrap_or(&value);
    let candidates = actual_val.get("candidates").and_then(Value::as_array);
    let usage = actual_val.get("usageMetadata");
    if candidates.is_none() && usage.is_none() {
        return events;
    }

    if !state.message_started {
        state.message_started = true;
        events.push(format!(
            "event: message_start\ndata: {}\n\n",
            json!({
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
            })
        ));
    }

    if let Some(candidates) = candidates {
        if let Some(candidate) = candidates.first() {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|content| content.get("parts"))
                .and_then(Value::as_array)
            {
                for part in parts {
                    if let Some(sig) = part
                        .get("thoughtSignature")
                        .or_else(|| part.get("thought_signature"))
                        .and_then(Value::as_str)
                    {
                        state.last_thought_signature = Some(sig.to_string());
                    }

                    let is_thought = part.get("thought").and_then(Value::as_bool).unwrap_or(false);

                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        if text.is_empty() {
                            continue;
                        }
                        if state.in_tool_block {
                            close_active_anthropic_blocks(&mut events, state);
                        }

                        if is_thought {
                            if state.in_text_block {
                                close_active_anthropic_blocks(&mut events, state);
                            }
                            if !state.in_thinking_block {
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
                                    "delta": { "type": "thinking_delta", "thinking": text }
                                })
                            ));
                        } else {
                            if state.in_thinking_block {
                                close_active_anthropic_blocks(&mut events, state);
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
                        }
                        state.total_output_tokens = state.total_output_tokens.saturating_add(1);
                    }

                    if let Some(function_call) = part.get("functionCall") {
                        if state.in_text_block || state.in_thinking_block || state.in_tool_block {
                            close_active_anthropic_blocks(&mut events, state);
                        }
                        let name = function_call
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("tool")
                            .to_string();
                        let id = function_call
                            .get("id")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("{}_tool_{}", msg_id, state.current_block_index));
                        state.in_tool_block = true;
                        state.current_tool_id = Some(id.clone());
                        state.current_tool_name = Some(name.clone());

                        let sig = part
                            .get("thoughtSignature")
                            .or_else(|| part.get("thought_signature"))
                            .or_else(|| function_call.get("thoughtSignature"))
                            .or_else(|| function_call.get("thought_signature"))
                            .and_then(Value::as_str)
                            .map(|s| s.to_string())
                            .or_else(|| state.last_thought_signature.clone());

                        if let Some(ref s) = sig {
                            anthropic_to_gemini::store_thought_signature(&id, &name, s);
                        }

                        events.push(format!(
                            "event: content_block_start\ndata: {}\n\n",
                            json!({
                                "type": "content_block_start",
                                "index": state.current_block_index,
                                "content_block": { "type": "tool_use", "id": id, "name": name }
                            })
                        ));

                        if let Some(args) = function_call.get("args") {
                            let partial_json =
                                serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string());
                            events.push(format!(
                                "event: content_block_delta\ndata: {}\n\n",
                                json!({
                                    "type": "content_block_delta",
                                    "index": state.current_block_index,
                                    "delta": {
                                        "type": "input_json_delta",
                                        "partial_json": partial_json
                                    }
                                })
                            ));
                        }

                        close_active_anthropic_blocks(&mut events, state);
                    }
                }
            }

            if let Some(reason) = candidate.get("finishReason").and_then(Value::as_str) {
                let stop_reason = if state.current_tool_id.is_some() || reason == "FUNCTION_CALL" {
                    "tool_use"
                } else {
                    match reason {
                        "MAX_TOKENS" => "max_tokens",
                        "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" => "end_turn",
                        _ => "end_turn",
                    }
                };
                if let Some(usage) = usage {
                    if let Some(input_tokens) = usage.get("promptTokenCount").and_then(Value::as_u64) {
                        state.total_input_tokens = input_tokens as u32;
                    }
                    if let Some(output_tokens) = usage.get("candidatesTokenCount").and_then(Value::as_u64) {
                        state.total_output_tokens = output_tokens as u32;
                    }
                }
                events.extend(finish_gemini_stream(state, stop_reason));
                return events;
            }
        }
    }

    if let Some(usage) = usage {
        if let Some(input_tokens) = usage.get("promptTokenCount").and_then(Value::as_u64) {
            state.total_input_tokens = input_tokens as u32;
        }
        if let Some(output_tokens) = usage.get("candidatesTokenCount").and_then(Value::as_u64) {
            state.total_output_tokens = output_tokens as u32;
        }
    }
    events
}

/// Finish an Anthropic SSE response produced from Gemini streaming output.
pub fn finish_gemini_stream(state: &mut SseState, stop_reason: &str) -> Vec<String> {
    finish_openai_stream(state, stop_reason)
}

/// Finish an Anthropic SSE response produced from OpenAI streaming output.
pub fn finish_openai_stream(state: &mut SseState, stop_reason: &str) -> Vec<String> {
    let mut events = Vec::new();
    if !state.message_started || state.stream_finished {
        return events;
    }
    close_active_anthropic_blocks(&mut events, state);
    let effective_stop_reason = state
        .pending_stop_reason
        .as_deref()
        .unwrap_or(stop_reason);
    let mut usage_obj = json!({
        "output_tokens": state.total_output_tokens
    });
    if state.total_input_tokens > 0 {
        usage_obj["input_tokens"] = json!(state.total_input_tokens);
    }
    events.push(format!(
        "event: message_delta\ndata: {}\n\n",
        json!({
            "type": "message_delta",
            "delta": { "stop_reason": effective_stop_reason, "stop_sequence": null },
            "usage": usage_obj
        })
    ));
    events.push("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string());
    state.stream_finished = true;
    events
}

/// Convert an OpenAI SSE line (`data: {...}`) to Anthropic SSE events
pub fn parse_openai_chunk_to_anthropic_events(
    line: &str,
    state: &mut SseState,
    msg_id: &str,
    model: &str,
) -> Vec<String> {
    let mut events = Vec::new();
    if state.stream_finished {
        return events;
    }
    let trimmed = line.trim();

    if !trimmed.starts_with("data:") {
        return events;
    }

    let payload = trimmed.trim_start_matches("data:").trim();
    if payload == "[DONE]" {
        return finish_openai_stream(state, "end_turn");
    }

    let chunk: OpenAIChatChunk = match serde_json::from_str(payload) {
        Ok(c) => c,
        Err(_) => return events,
    };

    // 捕获标准 usage 及 include_usage 独立尾包（choices 为空时也会带 usage）
    if let Some(ref usage) = chunk.usage {
        if usage.prompt_tokens > 0 {
            state.total_input_tokens = usage.prompt_tokens;
        }
        if usage.completion_tokens > 0 {
            state.total_output_tokens = usage.completion_tokens;
        }
    }

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
                "usage": { "input_tokens": state.total_input_tokens, "output_tokens": 0 }
            }
        });
        events.push(format!("event: message_start\ndata: {}\n\n", start_event));
    }

    if let Some(choice) = chunk.choices.first() {
        // Handle Reasoning / Thinking Tokens (e.g. DeepSeek R1 reasoning_content or <think>)
        if let Some(ref reasoning) = choice.delta.reasoning_content {
            if !reasoning.is_empty() {
                if !state.in_thinking_block {
                    if state.in_text_block || state.in_tool_block {
                        close_active_anthropic_blocks(&mut events, state);
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
                if chunk.usage.is_none() {
                    state.total_output_tokens += 1;
                }
            }
        }

        // Handle normal Text Content
        if let Some(ref text) = choice.delta.content {
            if !text.is_empty() {
                // If we were in thinking or tool block, close it
                if state.in_thinking_block || state.in_tool_block {
                    close_active_anthropic_blocks(&mut events, state);
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
                if chunk.usage.is_none() {
                    state.total_output_tokens += 1;
                }
            }
        }

        // Handle Tool Calls（防空白 id / 重复 id 将单一 tool_use 拆成多个残缺块）
        if let Some(ref tool_chunks) = choice.delta.tool_calls {
            for tc in tool_chunks {
                let non_empty_id = tc
                    .id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let raw_idx_changed = state.in_tool_block && state.current_tool_raw_index != Some(tc.index);
                let id_changed = non_empty_id.is_some()
                    && state.current_tool_id.as_deref() != non_empty_id;
                let need_new_block = !state.in_tool_block || raw_idx_changed || id_changed;

                if need_new_block {
                    close_active_anthropic_blocks(&mut events, state);
                    let id_str = non_empty_id
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("call_{}_{}", msg_id, state.current_block_index));
                    let name = tc
                        .function
                        .as_ref()
                        .and_then(|f| f.name.clone())
                        .unwrap_or_default();
                    state.in_tool_block = true;
                    state.current_tool_id = Some(id_str.clone());
                    state.current_tool_name = Some(name.clone());
                    state.current_tool_raw_index = Some(tc.index);

                    events.push(format!(
                        "event: content_block_start\ndata: {}\n\n",
                        json!({
                            "type": "content_block_start",
                            "index": state.current_block_index,
                            "content_block": {
                                "type": "tool_use",
                                "id": id_str,
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

        // 仅记录 pending_stop_reason 并关闭当前内容块，延迟至 `[DONE]` 或流结束再发送 message_delta/message_stop，
        // 避免丢弃紧随 finish_reason 之后的 stream_options.include_usage 独立尾包
        if let Some(ref finish_reason) = choice.finish_reason {
            close_active_anthropic_blocks(&mut events, state);
            let stop_reason = match finish_reason.as_str() {
                "stop" => "end_turn",
                "tool_calls" => "tool_use",
                "length" => "max_tokens",
                _ => "end_turn",
            };
            state.pending_stop_reason = Some(stop_reason.to_string());
        }
    }

    events
}

/// Convert an Anthropic SSE event to OpenAI SSE chunk format (`data: {...}\n\n`) with contiguous tool_calls index state
pub fn parse_anthropic_event_to_openai_chunks_with_state(
    raw: &str,
    msg_id: &str,
    model: &str,
    conv_state: &mut AnthropicToOpenAiSseState,
) -> Vec<String> {
    let mut chunks = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("data:") {
            continue;
        }

        let payload = trimmed.trim_start_matches("data:").trim();
        let val: Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match event_type {
        "content_block_start" => {
            let index = val.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            if let Some(block) = val.get("content_block") {
                let btype = block.get("type").and_then(Value::as_str).unwrap_or("");
                if btype == "tool_use" {
                    let tc_idx = conv_state.alloc_tool_index(index);
                    let id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let chunk = json!({
                        "id": msg_id,
                        "object": "chat.completion.chunk",
                        "created": chrono::Utc::now().timestamp(),
                        "model": model,
                        "choices": [{
                            "index": 0,
                            "delta": {
                                "tool_calls": [{
                                    "index": tc_idx,
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": ""
                                    }
                                }]
                            },
                            "finish_reason": null
                        }]
                    });
                    chunks.push(format!("data: {}\n\n", chunk));
                }
            }
        }
        "content_block_delta" => {
            let index = val.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let delta = val.get("delta");
            let delta_type = delta
                .and_then(|d| d.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
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
            } else if delta_type == "thinking_delta" {
                if let Some(thinking) = delta
                    .and_then(|d| d.get("thinking"))
                    .and_then(|t| t.as_str())
                {
                    let chunk = json!({
                        "id": msg_id,
                        "object": "chat.completion.chunk",
                        "created": chrono::Utc::now().timestamp(),
                        "model": model,
                        "choices": [{
                            "index": 0,
                            "delta": { "reasoning_content": thinking },
                            "finish_reason": null
                        }]
                    });
                    chunks.push(format!("data: {}\n\n", chunk));
                }
            } else if delta_type == "input_json_delta" {
                if let Some(partial) = delta
                    .and_then(|d| d.get("partial_json"))
                    .and_then(|p| p.as_str())
                {
                    let tc_idx = conv_state.alloc_tool_index(index);
                    let chunk = json!({
                        "id": msg_id,
                        "object": "chat.completion.chunk",
                        "created": chrono::Utc::now().timestamp(),
                        "model": model,
                        "choices": [{
                            "index": 0,
                            "delta": {
                                "tool_calls": [{
                                    "index": tc_idx,
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
            let stop_reason = val
                .get("delta")
                .and_then(|d| d.get("stop_reason"))
                .and_then(|s| s.as_str())
                .unwrap_or("stop");
            let finish_reason = match stop_reason {
                "tool_use" => "tool_calls",
                "max_tokens" => "length",
                _ => "stop",
            };
            let mut chunk = json!({
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
            if let Some(usage) = val.get("usage") {
                let out_tokens = usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
                let in_tokens = usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
                chunk["usage"] = json!({
                    "prompt_tokens": in_tokens,
                    "completion_tokens": out_tokens,
                    "total_tokens": in_tokens + out_tokens
                });
            }
            chunks.push(format!("data: {}\n\n", chunk));
        }
        "message_stop" => {
            chunks.push("data: [DONE]\n\n".to_string());
        }
        _ => {}
        }
    }

    chunks
}

/// Convert an Anthropic SSE event to OpenAI SSE chunk format (`data: {...}\n\n`)
pub fn parse_anthropic_event_to_openai_chunks(
    raw: &str,
    msg_id: &str,
    model: &str,
) -> Vec<String> {
    let mut state = AnthropicToOpenAiSseState::default();
    parse_anthropic_event_to_openai_chunks_with_state(raw, msg_id, model, &mut state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_stream_is_translated_to_anthropic_events() {
        let mut state = SseState::new();
        let first = parse_gemini_chunk_to_anthropic_events(
            r#"data: {"candidates":[{"content":{"parts":[{"text":"hello"}]}}]}"#,
            &mut state,
            "req-1",
            "claude-3-7-sonnet",
        );
        let first_output = first.concat();
        assert!(first_output.contains("message_start"));
        assert!(first_output.contains("content_block_start"));
        assert!(first_output.contains("hello"));
        assert!(!state.stream_finished);

        let second = parse_gemini_chunk_to_anthropic_events(
            r#"data: {"candidates":[{"finishReason":"STOP"}],"usageMetadata":{"candidatesTokenCount":7}}"#,
            &mut state,
            "req-1",
            "claude-3-7-sonnet",
        );
        let second_output = second.concat();
        assert!(second_output.contains("content_block_stop"));
        assert!(second_output.contains("stop_reason"));
        assert!(second_output.contains("message_stop"));
        assert_eq!(state.total_output_tokens, 7);
        assert!(state.stream_finished);
        assert!(finish_gemini_stream(&mut state, "end_turn").is_empty());
    }

    #[test]
    fn gemini_stream_without_finish_reason_is_closed_at_eof() {
        let mut state = SseState::new();
        let events = parse_gemini_chunk_to_anthropic_events(
            r#"data: {"candidates":[{"content":{"parts":[{"text":"partial"}]}}]}"#,
            &mut state,
            "req-2",
            "gemini-2.0-flash",
        );
        assert!(!events.is_empty());
        let final_events = finish_gemini_stream(&mut state, "end_turn");
        let output = final_events.concat();
        assert!(output.contains("message_delta"));
        assert!(output.contains("message_stop"));
        assert!(state.stream_finished);
    }

    #[test]
    fn openai_stream_emits_only_one_message_stop() {
        let mut state = SseState::new();
        let finish = parse_openai_chunk_to_anthropic_events(
            r#"data: {"id":"chat-1","object":"chat.completion.chunk","created":1,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":"done"},"finish_reason":"stop"}]}"#,
            &mut state,
            "req-3",
            "claude-3-7-sonnet",
        );
        // 延迟至 [DONE] 再封流，以便捕获紧随其后的 include_usage 独立尾包
        assert_eq!(finish.concat().matches("event: message_stop").count(), 0);
        let usage_tail = parse_openai_chunk_to_anthropic_events(
            r#"data: {"id":"chat-1","object":"chat.completion.chunk","created":1,"model":"gpt-4o","choices":[],"usage":{"prompt_tokens":42,"completion_tokens":7}}"#,
            &mut state,
            "req-3",
            "claude-3-7-sonnet",
        );
        assert!(usage_tail.concat().matches("event: message_stop").count() == 0);
        let done = parse_openai_chunk_to_anthropic_events(
            "data: [DONE]",
            &mut state,
            "req-3",
            "claude-3-7-sonnet",
        );
        let done_str = done.concat();
        assert_eq!(done_str.matches("event: message_stop").count(), 1);
        assert!(done_str.contains(r#""output_tokens":7"#));
        assert!(done_str.contains(r#""input_tokens":42"#));
    }

    #[test]
    fn sse_line_buffer_reassembles_split_utf8_characters() {
        let mut buf = SseLineBuffer::new();
        let line = "data: {\"text\":\"你好世界🚀\"}\n";
        let bytes = line.as_bytes();
        // 故意在多字节中文正中间切开
        let split_pos = bytes.iter().position(|&b| b == 0xE4).unwrap() + 1;
        let out1 = buf.push_and_read_lines(&bytes[..split_pos]);
        assert!(out1.is_empty());
        let out2 = buf.push_and_read_lines(&bytes[split_pos..]);
        assert_eq!(out2, vec!["data: {\"text\":\"你好世界🚀\"}"]);
    }

    #[test]
    fn gemini_stream_with_thinking_and_tool_call() {
        let mut state = SseState::new();

        // 1. Thinking block (thought = true)
        let chunk1 = r#"data: {"candidates":[{"content":{"parts":[{"text":"Thinking process...","thought":true}]}}]}"#;
        let evts1 = parse_gemini_chunk_to_anthropic_events(chunk1, &mut state, "req-t", "gemini-2.5-pro");
        let out1 = evts1.concat();
        assert!(out1.contains(r#""type":"thinking""#));
        assert!(out1.contains(r#""thinking":"Thinking process...""#));
        assert!(!out1.contains(r#""type":"text""#));

        // 2. Normal text
        let chunk2 = r#"data: {"candidates":[{"content":{"parts":[{"text":"I will run the command."}]}}]}"#;
        let evts2 = parse_gemini_chunk_to_anthropic_events(chunk2, &mut state, "req-t", "gemini-2.5-pro");
        let out2 = evts2.concat();
        assert!(out2.contains(r#""type":"text""#));
        assert!(out2.contains("I will run the command."));

        // 3. Tool call with thoughtSignature
        let chunk3 = r#"data: {"candidates":[{"content":{"parts":[{"functionCall":{"name":"Bash","args":{"command":"ls"}},"thoughtSignature":"sig-123"}]}}]}"#;
        let evts3 = parse_gemini_chunk_to_anthropic_events(chunk3, &mut state, "req-t", "gemini-2.5-pro");
        let out3 = evts3.concat();
        assert!(out3.contains(r#""type":"tool_use""#));
        assert!(out3.contains(r#""name":"Bash""#));
        assert!(out3.contains(r#""partial_json":"{\"command\":\"ls\"}""#));

        // 4. Finish reason FUNCTION_CALL
        let chunk4 = r#"data: {"candidates":[{"finishReason":"FUNCTION_CALL"}],"usageMetadata":{"candidatesTokenCount":12}}"#;
        let evts4 = parse_gemini_chunk_to_anthropic_events(chunk4, &mut state, "req-t", "gemini-2.5-pro");
        let out4 = evts4.concat();
        assert!(out4.contains(r#""stop_reason":"tool_use""#));
    }

    #[test]
    fn test_gemini_stream_parallel_function_calls() {
        let mut state = SseState::new();

        // 1. Text chunk
        let c1 = r#"data: {"candidates":[{"content":{"parts":[{"text":"Analyzing project structure..."}]}}]}"#;
        let evts1 = parse_gemini_chunk_to_anthropic_events(c1, &mut state, "req-p", "claude-opus-4-6-thinking");
        let out1 = evts1.concat();
        assert!(out1.contains(r#""type":"text""#));

        // 2. Single chunk containing two parallel function calls (Bash ls, Bash find)
        let c2 = r#"data: {"candidates":[{"content":{"parts":[
            {"functionCall":{"name":"Bash","args":{"command":"ls /Users/votan/projects/pivot"}}},
            {"functionCall":{"name":"Bash","args":{"command":"find /Users/votan/projects/pivot -name package.json"}}}
        ]}}]}"#;
        let evts2 = parse_gemini_chunk_to_anthropic_events(c2, &mut state, "req-p", "claude-opus-4-6-thinking");

        // Verify tool_use starts and stops
        let starts: Vec<_> = evts2.iter().filter(|e| e.contains("content_block_start")).collect();
        let stops: Vec<_> = evts2.iter().filter(|e| e.contains("content_block_stop")).collect();
        let deltas: Vec<_> = evts2.iter().filter(|e| e.contains("input_json_delta")).collect();

        assert_eq!(starts.len(), 2, "Must start 2 tool_use content blocks");
        assert_eq!(deltas.len(), 2, "Must emit 2 input_json_deltas");
        // 3 stops in total: 1 for preceding text block (index 0), 2 for the 2 tool_use blocks (indices 1 & 2)
        assert_eq!(stops.len(), 3, "Must stop text block (index 0) and the 2 tool blocks (indices 1 & 2)");

        // First stop closes text block (index 0)
        assert!(stops[0].contains(r#""index":0"#));

        // First tool block: index 1
        assert!(starts[0].contains(r#""index":1"#));
        assert!(deltas[0].contains(r#""partial_json":"{\"command\":\"ls /Users/votan/projects/pivot\"}""#));
        assert!(stops[1].contains(r#""index":1"#));

        // Second tool block: index 2 (must not contaminate or concatenate with the first!)
        assert!(starts[1].contains(r#""index":2"#));
        assert!(deltas[1].contains(r#""partial_json":"{\"command\":\"find /Users/votan/projects/pivot -name package.json\"}""#));
        assert!(stops[2].contains(r#""index":2"#));

        // 3. Finish reason FUNCTION_CALL
        let c3 = r#"data: {"candidates":[{"finishReason":"FUNCTION_CALL"}],"usageMetadata":{"candidatesTokenCount":50}}"#;
        let evts3 = parse_gemini_chunk_to_anthropic_events(c3, &mut state, "req-p", "claude-opus-4-6-thinking");
        let out3 = evts3.concat();
        assert!(out3.contains(r#""stop_reason":"tool_use""#));
    }

    #[test]
    fn anthropic_stream_to_openai_chunks_tool_and_thinking() {
        // 1. Tool start
        let tool_start = r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tool_123","name":"Bash"}}"#;
        let c1 = parse_anthropic_event_to_openai_chunks(tool_start, "req-oai", "gpt-4o");
        assert_eq!(c1.len(), 1);
        assert!(c1[0].contains(r#""id":"tool_123""#));
        assert!(c1[0].contains(r#""name":"Bash""#));

        // 2. Tool input delta
        let tool_delta = r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"cmd\":\"ls\"}"}}"#;
        let c2 = parse_anthropic_event_to_openai_chunks(tool_delta, "req-oai", "gpt-4o");
        assert_eq!(c2.len(), 1);
        assert!(c2[0].contains(r#""arguments":"{\"cmd\":\"ls\"}""#));

        // 3. Thinking delta
        let think_delta = r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"let me think"}}"#;
        let c3 = parse_anthropic_event_to_openai_chunks(think_delta, "req-oai", "gpt-4o");
        assert_eq!(c3.len(), 1);
        assert!(c3[0].contains(r#""reasoning_content":"let me think""#));

        // 4. Message delta with stop_reason = tool_use -> finish_reason = tool_calls
        let msg_delta = r#"data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":20}}"#;
        let c4 = parse_anthropic_event_to_openai_chunks(msg_delta, "req-oai", "gpt-4o");
        assert_eq!(c4.len(), 1);
        assert!(c4[0].contains(r#""finish_reason":"tool_calls""#));
        assert!(c4[0].contains(r#""completion_tokens":20"#));

        // 5. Message stop -> [DONE]
        let stop = r#"data: {"type":"message_stop"}"#;
        let c5 = parse_anthropic_event_to_openai_chunks(stop, "req-oai", "gpt-4o");
        assert_eq!(c5.len(), 1);
        assert!(c5[0].contains("[DONE]"));
    }

    #[test]
    fn test_gemini_stream_to_anthropic_and_openai_usage() {
        let mut state = SseState::new();
        state.total_input_tokens = 50;

        let chunk1 = r#"data: {"candidates":[{"content":{"parts":[{"text":"Hi!"}]}}]}"#;
        let _ = parse_gemini_chunk_to_anthropic_events(chunk1, &mut state, "req-u", "gemini-2.5-pro");

        let chunk2 = r#"data: {"candidates":[{"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":55,"candidatesTokenCount":15}}"#;
        let anthropic_events = parse_gemini_chunk_to_anthropic_events(chunk2, &mut state, "req-u", "gemini-2.5-pro");

        let all_a = anthropic_events.concat();
        assert!(all_a.contains(r#""output_tokens":15"#));
        assert!(all_a.contains(r#""input_tokens":55"#));

        // Now translate to OpenAI chunks
        let mut oai_chunks = Vec::new();
        for a_evt in &anthropic_events {
            oai_chunks.extend(parse_anthropic_event_to_openai_chunks(a_evt, "req-u", "gemini-2.5-pro"));
        }

        let all_o = oai_chunks.concat();
        assert!(all_o.contains(r#""prompt_tokens":55"#));
        assert!(all_o.contains(r#""completion_tokens":15"#));
        assert!(all_o.contains(r#""total_tokens":70"#));
        assert!(all_o.contains("[DONE]"));
    }

    #[test]
    fn test_openai_deepseek_reasoning_to_tool_calls_stream() {
        let mut state = SseState::new();

        // 1. DeepSeek R1 reasoning stream chunk
        let chunk1 = r#"data: {"id":"chat-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"reasoning_content":"Thinking about file list..."},"finish_reason":null}]}"#;
        let evts1 = parse_openai_chunk_to_anthropic_events(chunk1, &mut state, "req-ds", "deepseek-reasoner");
        let out1 = evts1.concat();
        assert!(out1.contains(r#""type":"message_start""#));
        assert!(out1.contains(r#""type":"content_block_start""#));
        assert!(out1.contains(r#""type":"thinking""#));
        assert!(out1.contains(r#""thinking":"Thinking about file list...""#));
        assert_eq!(state.current_block_index, 0);
        assert!(state.in_thinking_block);

        // 2. DeepSeek switches straight from reasoning into tool call (no content text)
        let chunk2 = r#"data: {"id":"chat-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_bash_01","type":"function","function":{"name":"Bash","arguments":""}}]},"finish_reason":null}]}"#;
        let evts2 = parse_openai_chunk_to_anthropic_events(chunk2, &mut state, "req-ds", "deepseek-reasoner");
        let out2 = evts2.concat();
        // Crucial: Must have closed the thinking block at index 0 first!
        assert!(
            out2.contains(r#"{"index":0,"type":"content_block_stop"}"#)
                || out2.contains(r#"{"type":"content_block_stop","index":0}"#)
        );
        // And started the tool_use block at index 1!
        assert!(out2.contains(r#""type":"content_block_start""#));
        assert!(out2.contains(r#""index":1"#));
        assert!(out2.contains(r#""type":"tool_use""#));
        assert!(out2.contains(r#""id":"call_bash_01""#));
        assert!(!state.in_thinking_block);
        assert!(state.in_tool_block);
        assert_eq!(state.current_block_index, 1);

        // 3. Tool argument delta
        let chunk3 = r#"data: {"id":"chat-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"cmd\":\"ls\"}"}}]},"finish_reason":null}]}"#;
        let evts3 = parse_openai_chunk_to_anthropic_events(chunk3, &mut state, "req-ds", "deepseek-reasoner");
        let out3 = evts3.concat();
        assert!(out3.contains(r#""type":"content_block_delta""#));
        assert!(out3.contains(r#""index":1"#));
        assert!(out3.contains(r#""partial_json":"{\"cmd\":\"ls\"}""#));

        // 4. Finish reason tool_calls (deferred message_delta/message_stop until [DONE] to preserve include_usage tail)
        let chunk4 = r#"data: {"id":"chat-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#;
        let evts4 = parse_openai_chunk_to_anthropic_events(chunk4, &mut state, "req-ds", "deepseek-reasoner");
        let out4 = evts4.concat();
        // Must close the tool_use block at index 1 immediately and stash pending_stop_reason
        assert!(
            out4.contains(r#"{"index":1,"type":"content_block_stop"}"#)
                || out4.contains(r#"{"type":"content_block_stop","index":1}"#)
        );
        assert!(!state.in_tool_block);
        assert!(!state.stream_finished);
        assert_eq!(state.pending_stop_reason.as_deref(), Some("tool_use"));

        // 5. [DONE] finalizes stream once with tool_use stop_reason, then rejects duplicates
        let evts5 = parse_openai_chunk_to_anthropic_events("data: [DONE]", &mut state, "req-ds", "deepseek-reasoner");
        let out5 = evts5.concat();
        assert!(out5.contains(r#""stop_reason":"tool_use""#));
        assert!(out5.contains("event: message_stop"));
        assert!(state.stream_finished);

        let evts6 = parse_openai_chunk_to_anthropic_events("data: [DONE]", &mut state, "req-ds", "deepseek-reasoner");
        assert!(evts6.is_empty());
    }

    #[test]
    fn test_openai_stream_eof_safeguard() {
        let mut state = SseState::new();

        // 1. Thinking block started
        let chunk1 = r#"data: {"id":"chat-2","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"reasoning_content":"Long reasoning..."},"finish_reason":null}]}"#;
        let _ = parse_openai_chunk_to_anthropic_events(chunk1, &mut state, "req-eof", "deepseek-reasoner");
        assert!(state.in_thinking_block);
        assert!(!state.stream_finished);

        // 2. Upstream closes unexpectedly without [DONE] or finish_reason
        let finish_events = finish_openai_stream(&mut state, "end_turn");
        let out = finish_events.concat();
        assert!(
            out.contains(r#"{"index":0,"type":"content_block_stop"}"#)
                || out.contains(r#"{"type":"content_block_stop","index":0}"#)
        );
        assert!(out.contains(r#""stop_reason":"end_turn""#));
        assert!(out.contains(r#"event: message_stop"#));
        assert!(state.stream_finished);
    }
}
