use crate::gateway::types::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub struct ResponsesSseState {
    pub response_id: String,
    pub model: String,
    pub item_id: String,
    pub created_at: i64,
    pub response_created_sent: bool,
    pub text_started: bool,
    pub full_text: String,
    pub reasoning_started: bool,
    pub in_tool_call: bool,
    pub tool_calls_buf: BTreeMap<usize, ToolCallBuf>,
    pub finish_reason: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub stream_finished: bool,
    pub is_compaction: bool,
}

#[derive(Default, Clone)]
pub struct ToolCallBuf {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl ResponsesSseState {
    pub fn new(response_id: &str, model: &str) -> Self {
        Self::new_with_compaction(response_id, model, false)
    }

    pub fn new_with_compaction(response_id: &str, model: &str, is_compaction: bool) -> Self {
        let item_suffix = uuid::Uuid::new_v4().to_string().replace('-', "");
        Self {
            response_id: response_id.to_string(),
            model: model.to_string(),
            item_id: format!("msg_{}", &item_suffix[..16]),
            created_at: chrono::Utc::now().timestamp(),
            response_created_sent: false,
            text_started: false,
            full_text: String::new(),
            reasoning_started: false,
            in_tool_call: false,
            tool_calls_buf: BTreeMap::new(),
            finish_reason: "stop".to_string(),
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            stream_finished: false,
            is_compaction,
        }
    }

    fn sse(event_type: &str, data: &Value) -> String {
        format!(
            "event: {}\ndata: {}\n\n",
            event_type,
            serde_json::to_string(data).unwrap_or_default()
        )
    }

    pub fn process_line(&mut self, line: &str) -> Vec<String> {
        let mut events = Vec::new();
        if self.stream_finished {
            return events;
        }

        let trimmed = line.trim();
        if !trimmed.starts_with("data:") {
            return events;
        }

        let payload = trimmed.trim_start_matches("data:").trim();
        if payload == "[DONE]" {
            events.extend(self.finish());
            return events;
        }

        let chunk: Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(_) => return events,
        };

        if !self.response_created_sent {
            self.response_created_sent = true;
            events.push(Self::sse(
                "response.created",
                &json!({
                    "type": "response.created",
                    "response": {
                        "id": self.response_id,
                        "object": "response",
                        "created_at": self.created_at,
                        "model": self.model,
                        "status": "in_progress",
                        "output": []
                    }
                }),
            ));
        }

        if let Some(usage) = chunk.get("usage") {
            if let Some(pt) = usage.get("prompt_tokens").and_then(Value::as_u64) {
                self.prompt_tokens = pt as u32;
            }
            if let Some(ct) = usage.get("completion_tokens").and_then(Value::as_u64) {
                self.completion_tokens = ct as u32;
            }
            if let Some(tt) = usage.get("total_tokens").and_then(Value::as_u64) {
                self.total_tokens = tt as u32;
            }
        }

        let choice = chunk
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first());

        if let Some(choice) = choice {
            if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
                self.finish_reason = fr.to_string();
            }

            if let Some(delta) = choice.get("delta") {
                // 1. Thinking / reasoning content delta (e.g. DeepSeek-R1)
                if let Some(reasoning) = delta.get("reasoning_content").and_then(Value::as_str) {
                    if !reasoning.is_empty() {
                        if !self.reasoning_started {
                            self.reasoning_started = true;
                            events.push(Self::sse(
                                "response.reasoning_summary_part.added",
                                &json!({
                                    "type": "response.reasoning_summary_part.added",
                                    "item_id": self.item_id,
                                    "output_index": 0,
                                    "summary_index": 0,
                                    "part": { "type": "summary_text", "text": "" }
                                }),
                            ));
                        }
                        events.push(Self::sse(
                            "response.reasoning_summary_text.delta",
                            &json!({
                                "type": "response.reasoning_summary_text.delta",
                                "item_id": self.item_id,
                                "output_index": 0,
                                "summary_index": 0,
                                "delta": reasoning
                            }),
                        ));
                    }
                }

                // 2. Tool calls delta
                if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                    self.in_tool_call = true;
                    for tc in tool_calls {
                        let idx = tc.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                        let buf = self.tool_calls_buf.entry(idx).or_default();
                        if let Some(id) = tc.get("id").and_then(Value::as_str) {
                            buf.id.push_str(id);
                        }
                        if let Some(fn_obj) = tc.get("function") {
                            if let Some(name) = fn_obj.get("name").and_then(Value::as_str) {
                                buf.name.push_str(name);
                            }
                            if let Some(args) = fn_obj.get("arguments").and_then(Value::as_str) {
                                buf.arguments.push_str(args);
                            }
                        }
                    }
                }

                // 3. Output text delta
                if let Some(token) = delta.get("content").and_then(Value::as_str) {
                    if !token.is_empty() {
                        if !self.text_started {
                            self.text_started = true;
                            events.push(Self::sse(
                                "response.output_item.added",
                                &json!({
                                    "type": "response.output_item.added",
                                    "output_index": 0,
                                    "item": {
                                        "id": self.item_id,
                                        "object": "response.output_item",
                                        "type": "message",
                                        "role": "assistant",
                                        "status": "in_progress",
                                        "content": []
                                    }
                                }),
                            ));
                            events.push(Self::sse(
                                "response.content_part.added",
                                &json!({
                                    "type": "response.content_part.added",
                                    "item_id": self.item_id,
                                    "output_index": 0,
                                    "content_index": 0,
                                    "part": { "type": "output_text", "text": "", "annotations": [] }
                                }),
                            ));
                        }
                        self.full_text.push_str(token);
                        events.push(Self::sse(
                            "response.output_text.delta",
                            &json!({
                                "type": "response.output_text.delta",
                                "item_id": self.item_id,
                                "output_index": 0,
                                "content_index": 0,
                                "delta": token
                            }),
                        ));
                    }
                }
            }
        }

        events
    }

    pub fn finish(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        if self.stream_finished {
            return events;
        }
        self.stream_finished = true;

        if !self.response_created_sent {
            self.response_created_sent = true;
            events.push(Self::sse(
                "response.created",
                &json!({
                    "type": "response.created",
                    "response": {
                        "id": self.response_id,
                        "object": "response",
                        "created_at": self.created_at,
                        "model": self.model,
                        "status": "in_progress",
                        "output": []
                    }
                }),
            ));
        }

        let output: Vec<Value>;

        if self.is_compaction {
            let comp_item = json!({
                "id": format!("cmp_{}", &self.item_id.trim_start_matches("msg_")),
                "object": "response.output_item",
                "type": "compaction",
                "status": "completed",
                "encrypted_content": self.full_text
            });
            events.push(Self::sse(
                "response.output_item.added",
                &json!({
                    "type": "response.output_item.added",
                    "output_index": 0,
                    "item": comp_item
                }),
            ));
            events.push(Self::sse(
                "response.output_item.done",
                &json!({
                    "type": "response.output_item.done",
                    "output_index": 0,
                    "item": comp_item
                }),
            ));
            output = vec![comp_item];
        } else if self.in_tool_call && !self.tool_calls_buf.is_empty() {
            let mut tool_items = Vec::new();
            for (idx, buf) in &self.tool_calls_buf {
                let call_id = if buf.id.is_empty() {
                    format!("call_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..16].to_string())
                } else {
                    buf.id.clone()
                };
                let fc_item = json!({
                    "id": format!("msg_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..16].to_string()),
                    "object": "response.output_item",
                    "type": "function_call",
                    "call_id": call_id,
                    "name": buf.name,
                    "arguments": buf.arguments,
                    "status": "completed"
                });
                events.push(Self::sse(
                    "response.output_item.added",
                    &json!({
                        "type": "response.output_item.added",
                        "output_index": idx,
                        "item": fc_item
                    }),
                ));
                events.push(Self::sse(
                    "response.output_item.done",
                    &json!({
                        "type": "response.output_item.done",
                        "output_index": idx,
                        "item": fc_item
                    }),
                ));
                tool_items.push(fc_item);
            }
            output = tool_items;
        } else {
            if self.text_started {
                events.push(Self::sse(
                    "response.output_text.done",
                    &json!({
                        "type": "response.output_text.done",
                        "item_id": self.item_id,
                        "output_index": 0,
                        "content_index": 0,
                        "text": self.full_text
                    }),
                ));
                events.push(Self::sse(
                    "response.output_item.done",
                    &json!({
                        "type": "response.output_item.done",
                        "output_index": 0,
                        "item": {
                            "id": self.item_id,
                            "object": "response.output_item",
                            "type": "message",
                            "role": "assistant",
                            "status": "completed",
                            "content": [{
                                "type": "output_text",
                                "text": self.full_text,
                                "annotations": []
                            }]
                        }
                    }),
                ));
            }
            output = vec![json!({
                "id": self.item_id,
                "object": "response.output_item",
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [{
                    "type": "output_text",
                    "text": self.full_text,
                    "annotations": []
                }]
            })];
        }

        events.push(Self::sse(
            "response.completed",
            &json!({
                "type": "response.completed",
                "response": {
                    "id": self.response_id,
                    "object": "response",
                    "created_at": self.created_at,
                    "model": self.model,
                    "status": "completed",
                    "output": output,
                    "usage": {
                        "input_tokens": self.prompt_tokens,
                        "output_tokens": self.completion_tokens,
                        "total_tokens": self.total_tokens
                    },
                    "stop_reason": self.finish_reason
                }
            }),
        ));

        events
    }
}

fn extract_text_from_value(content: &Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(arr) = content.as_array() {
        let mut parts = Vec::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                parts.push(s.to_string());
            } else if let Some(text) = item.get("text").and_then(Value::as_str) {
                parts.push(text.to_string());
            } else if let Some(c) = item.get("content").and_then(Value::as_str) {
                parts.push(c.to_string());
            }
        }
        return parts.join("\n");
    }
    content.to_string()
}

/// Check if the incoming Responses API request is asking for context compaction
pub fn is_compaction_request(body: &Value) -> bool {
    let raw_input = body.get("input").or_else(|| body.get("messages"));
    if let Some(arr) = raw_input.and_then(Value::as_array) {
        for item in arr {
            if let Some(itype) = item.get("type").and_then(Value::as_str) {
                if itype == "compaction_trigger" {
                    return true;
                }
            }
        }
    }
    false
}

/// Convert incoming `/v1/responses` JSON body to standard `OpenAIChatRequest`
pub fn convert_responses_req_to_chat(body: &Value) -> OpenAIChatRequest {
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("gpt-4o")
        .to_string();

    let mut messages: Vec<OpenAIMessage> = Vec::new();

    // 1. instructions / system prompt
    if let Some(inst) = body.get("instructions").or_else(|| body.get("system")) {
        let text = extract_text_from_value(inst);
        if !text.is_empty() {
            messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: Some(json!(text)),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
                name: None,
            });
        }
    }

    // 2. input items / messages
    let raw_input = body.get("input").or_else(|| body.get("messages"));
    if let Some(raw) = raw_input {
        if let Some(s) = raw.as_str() {
            messages.push(OpenAIMessage {
                role: "user".to_string(),
                content: Some(json!(s)),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
                name: None,
            });
        } else if let Some(arr) = raw.as_array() {
            let mut pending_tool_calls: Vec<OpenAIToolCall> = Vec::new();

            for item in arr {
                if let Some(s) = item.as_str() {
                    if !pending_tool_calls.is_empty() {
                        messages.push(OpenAIMessage {
                            role: "assistant".to_string(),
                            content: None,
                            tool_calls: Some(std::mem::take(&mut pending_tool_calls)),
                            tool_call_id: None,
                            reasoning_content: None,
                            name: None,
                        });
                    }
                    messages.push(OpenAIMessage {
                        role: "user".to_string(),
                        content: Some(json!(s)),
                        tool_calls: None,
                        tool_call_id: None,
                        reasoning_content: None,
                        name: None,
                    });
                    continue;
                }

                if !item.is_object() {
                    continue;
                }

                let itype = item.get("type").and_then(Value::as_str).unwrap_or("");
                let role = item.get("role").and_then(Value::as_str).unwrap_or("");

                if itype == "compaction" {
                    if !pending_tool_calls.is_empty() {
                        messages.push(OpenAIMessage {
                            role: "assistant".to_string(),
                            content: None,
                            tool_calls: Some(std::mem::take(&mut pending_tool_calls)),
                            tool_call_id: None,
                            reasoning_content: None,
                            name: None,
                        });
                    }
                    if let Some(enc) = item.get("encrypted_content").and_then(Value::as_str) {
                        if !enc.is_empty() {
                            messages.push(OpenAIMessage {
                                role: "system".to_string(),
                                content: Some(json!(format!("[Compacted session history]:\n{}", enc))),
                                tool_calls: None,
                                tool_call_id: None,
                                reasoning_content: None,
                                name: None,
                            });
                        }
                    }
                    continue;
                } else if itype == "compaction_trigger" {
                    if !pending_tool_calls.is_empty() {
                        messages.push(OpenAIMessage {
                            role: "assistant".to_string(),
                            content: None,
                            tool_calls: Some(std::mem::take(&mut pending_tool_calls)),
                            tool_call_id: None,
                            reasoning_content: None,
                            name: None,
                        });
                    }
                    messages.push(OpenAIMessage {
                        role: "user".to_string(),
                        content: Some(json!("Please provide a concise, comprehensive summary of the preceding conversation and workspace state for context compaction.")),
                        tool_calls: None,
                        tool_call_id: None,
                        reasoning_content: None,
                        name: None,
                    });
                    continue;
                }

                if itype == "function_call_output" {
                    if !pending_tool_calls.is_empty() {
                        messages.push(OpenAIMessage {
                            role: "assistant".to_string(),
                            content: None,
                            tool_calls: Some(std::mem::take(&mut pending_tool_calls)),
                            tool_call_id: None,
                            reasoning_content: None,
                            name: None,
                        });
                    }
                    let call_id = item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .unwrap_or("call_default")
                        .to_string();
                    let output = item
                        .get("output")
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                s.to_string()
                            } else {
                                v.to_string()
                            }
                        })
                        .unwrap_or_default();
                    messages.push(OpenAIMessage {
                        role: "tool".to_string(),
                        content: Some(json!(output)),
                        tool_call_id: Some(call_id),
                        tool_calls: None,
                        reasoning_content: None,
                        name: None,
                    });
                } else if itype == "function_call" {
                    let call_id = item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .unwrap_or("call_default")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let arguments = item
                        .get("arguments")
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                s.to_string()
                            } else {
                                v.to_string()
                            }
                        })
                        .unwrap_or_else(|| "{}".to_string());
                    pending_tool_calls.push(OpenAIToolCall {
                        index: None,
                        id: call_id,
                        call_type: "function".to_string(),
                        function: OpenAIFunctionCall { name, arguments },
                    });
                } else {
                    if !pending_tool_calls.is_empty() {
                        messages.push(OpenAIMessage {
                            role: "assistant".to_string(),
                            content: None,
                            tool_calls: Some(std::mem::take(&mut pending_tool_calls)),
                            tool_call_id: None,
                            reasoning_content: None,
                            name: None,
                        });
                    }
                    let role_str = match role {
                        "system" | "developer" => "system",
                        "assistant" | "model" => "assistant",
                        _ => "user",
                    };
                    let content_val = item
                        .get("content")
                        .or_else(|| item.get("text"))
                        .cloned()
                        .unwrap_or(Value::Null);

                    // Convert input_text to text if structured content
                    let clean_content = if let Some(parts) = content_val.as_array() {
                        let mut new_parts = Vec::new();
                        for p in parts {
                            if let Some(obj) = p.as_object() {
                                let mut new_obj = obj.clone();
                                if new_obj.get("type").and_then(Value::as_str) == Some("input_text") {
                                    new_obj.insert("type".to_string(), json!("text"));
                                }
                                new_parts.push(Value::Object(new_obj));
                            } else {
                                new_parts.push(p.clone());
                            }
                        }
                        Value::Array(new_parts)
                    } else if let Some(s) = content_val.as_str() {
                        json!(s)
                    } else {
                        json!(extract_text_from_value(&content_val))
                    };

                    messages.push(OpenAIMessage {
                        role: role_str.to_string(),
                        content: Some(clean_content),
                        tool_calls: None,
                        tool_call_id: None,
                        reasoning_content: None,
                        name: None,
                    });
                }
            }

            if !pending_tool_calls.is_empty() {
                messages.push(OpenAIMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(pending_tool_calls),
                    tool_call_id: None,
                    reasoning_content: None,
                    name: None,
                });
            }
        }
    }

    // 3. tools
    let mut chat_tools = Vec::new();
    if let Some(tools_arr) = body.get("tools").and_then(Value::as_array) {
        for t in tools_arr {
            if let Some(func_val) = t.get("function") {
                if let Ok(func) = serde_json::from_value::<OpenAIFunction>(func_val.clone()) {
                    chat_tools.push(OpenAITool {
                        tool_type: "function".to_string(),
                        function: func,
                    });
                }
            } else if t.get("type").and_then(Value::as_str) == Some("function") || t.get("name").is_some() {
                let name = t.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                let description = t.get("description").and_then(Value::as_str).map(|s| s.to_string());
                let parameters = t.get("parameters").cloned().unwrap_or(json!({}));
                chat_tools.push(OpenAITool {
                    tool_type: "function".to_string(),
                    function: OpenAIFunction {
                        name,
                        description,
                        parameters,
                    },
                });
            }
        }
    }
    let tools = if chat_tools.is_empty() { None } else { Some(chat_tools) };

    let temperature = body.get("temperature").and_then(Value::as_f64);
    let max_tokens = body
        .get("max_output_tokens")
        .or_else(|| body.get("max_tokens"))
        .and_then(Value::as_u64)
        .map(|n| n as u32);
    let stream = body.get("stream").and_then(Value::as_bool).or(Some(true));
    let tool_choice = body.get("tool_choice").cloned();
    let reasoning_effort = body
        .get("reasoning_effort")
        .or_else(|| body.get("reasoning"))
        .or_else(|| body.get("model_reasoning_effort"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    OpenAIChatRequest {
        model,
        messages,
        temperature,
        max_tokens,
        max_completion_tokens: None,
        reasoning_effort,
        tools,
        tool_choice,
        stream,
        stream_options: None,
    }
}

/// Convert non-streaming OpenAI Chat Completion response to Responses API JSON format
pub fn chat_response_to_responses(
    chat_resp: &Value,
    response_id: &str,
    model: &str,
    is_compaction: bool,
) -> Value {
    let choice = chat_resp
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .cloned()
        .unwrap_or(Value::Null);

    let message = choice.get("message").cloned().unwrap_or(Value::Null);
    let finish = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .unwrap_or("stop");
    let usage = chat_resp.get("usage");

    let mut output = Vec::new();

    if is_compaction {
        let text = message.get("content").and_then(Value::as_str).unwrap_or("");
        output.push(json!({
            "id": format!("cmp_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..16].to_string()),
            "object": "response.output_item",
            "type": "compaction",
            "status": "completed",
            "encrypted_content": text
        }));
    } else if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for tc in tool_calls {
            let fn_obj = tc.get("function").cloned().unwrap_or(Value::Null);
            output.push(json!({
                "id": format!("msg_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..16].to_string()),
                "object": "response.output_item",
                "type": "function_call",
                "call_id": tc.get("id").and_then(Value::as_str).unwrap_or("call_default"),
                "name": fn_obj.get("name").and_then(Value::as_str).unwrap_or(""),
                "arguments": fn_obj.get("arguments").and_then(Value::as_str).unwrap_or("{}"),
                "status": "completed"
            }));
        }
    } else {
        let text = message.get("content").and_then(Value::as_str).unwrap_or("");
        output.push(json!({
            "id": format!("msg_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..16].to_string()),
            "object": "response.output_item",
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [{
                "type": "output_text",
                "text": text,
                "annotations": []
            }]
        }));
    }

    json!({
        "id": response_id,
        "object": "response",
        "created_at": chrono::Utc::now().timestamp(),
        "model": model,
        "status": "completed",
        "output": output,
        "usage": {
            "input_tokens": usage.and_then(|u| u.get("prompt_tokens")).and_then(Value::as_u64).unwrap_or(0),
            "output_tokens": usage.and_then(|u| u.get("completion_tokens")).and_then(Value::as_u64).unwrap_or(0),
            "total_tokens": usage.and_then(|u| u.get("total_tokens")).and_then(Value::as_u64).unwrap_or(0),
        },
        "stop_reason": finish
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_responses_req_to_chat() {
        let req_json = json!({
            "model": "deepseek/deepseek-v4-pro",
            "instructions": "You are Codex",
            "input": [
                { "role": "user", "content": "Hello" },
                {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "bash",
                    "arguments": "{\"command\":\"ls\"}"
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_123",
                    "output": "file1.txt\nfile2.txt"
                }
            ],
            "tools": [
                {
                    "type": "function",
                    "name": "bash",
                    "description": "Execute bash command",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "command": { "type": "string" }
                        }
                    }
                }
            ],
            "stream": true,
            "max_output_tokens": 2048
        });

        let chat_req = convert_responses_req_to_chat(&req_json);
        assert_eq!(chat_req.model, "deepseek/deepseek-v4-pro");
        assert_eq!(chat_req.messages.len(), 4);
        assert_eq!(chat_req.messages[0].role, "system");
        assert_eq!(chat_req.messages[0].content.as_ref().unwrap(), &json!("You are Codex"));
        assert_eq!(chat_req.messages[1].role, "user");
        assert_eq!(chat_req.messages[2].role, "assistant");
        assert!(chat_req.messages[2].tool_calls.is_some());
        assert_eq!(chat_req.messages[3].role, "tool");
        assert_eq!(chat_req.messages[3].tool_call_id.as_deref(), Some("call_123"));

        assert!(chat_req.tools.is_some());
        assert_eq!(chat_req.tools.as_ref().unwrap()[0].function.name, "bash");
        assert_eq!(chat_req.max_tokens, Some(2048));
        assert_eq!(chat_req.stream, Some(true));
    }

    #[test]
    fn test_responses_sse_state_streaming() {
        let mut state = ResponsesSseState::new("resp_test123", "deepseek/deepseek-v4-pro");

        // Chunk 1: reasoning
        let chunk1 = "data: {\"choices\": [{\"delta\": {\"reasoning_content\": \"Thinking step\"}}]}";
        let ev1 = state.process_line(chunk1);
        assert_eq!(ev1.len(), 3); // response.created + reasoning_summary_part.added + reasoning_summary_text.delta
        assert!(ev1[0].contains("response.created"));
        assert!(ev1[1].contains("response.reasoning_summary_part.added"));
        assert!(ev1[2].contains("Thinking step"));

        // Chunk 2: text content
        let chunk2 = "data: {\"choices\": [{\"delta\": {\"content\": \"Hello world\"}}]}";
        let ev2 = state.process_line(chunk2);
        assert_eq!(ev2.len(), 3); // output_item.added + content_part.added + output_text.delta
        assert!(ev2[0].contains("response.output_item.added"));
        assert!(ev2[1].contains("response.content_part.added"));
        assert!(ev2[2].contains("Hello world"));

        // Chunk 3: DONE
        let done = "data: [DONE]";
        let ev3 = state.process_line(done);
        assert_eq!(ev3.len(), 3); // output_text.done + output_item.done + response.completed
        assert!(ev3[0].contains("response.output_text.done"));
        assert!(ev3[1].contains("response.output_item.done"));
        assert!(ev3[2].contains("response.completed"));
    }

    #[test]
    fn test_compaction_request_and_sse() {
        let req_json = json!({
            "model": "work/gpt-6-astra",
            "input": [
                {
                    "type": "compaction_trigger"
                }
            ],
            "stream": true
        });

        assert!(is_compaction_request(&req_json));

        let chat_req = convert_responses_req_to_chat(&req_json);
        assert_eq!(chat_req.messages.len(), 1);
        assert_eq!(chat_req.messages[0].role, "user");
        assert!(chat_req.messages[0].content.as_ref().unwrap().to_string().contains("summary"));

        let mut state = ResponsesSseState::new_with_compaction("resp_cmp123", "work/gpt-6-astra", true);
        let chunk = "data: {\"choices\": [{\"delta\": {\"content\": \"Summary of prior turns.\"}}]}";
        let _ = state.process_line(chunk);
        let evs = state.process_line("data: [DONE]");
        let completed_ev = evs.iter().find(|e| e.contains("response.completed")).unwrap();
        assert!(completed_ev.contains("\"type\":\"compaction\""));
        assert!(completed_ev.contains("\"encrypted_content\":\"Summary of prior turns.\""));

        // Non-streaming
        let chat_resp = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Summary of prior turns."
                },
                "finish_reason": "stop"
            }]
        });
        let responses_val = chat_response_to_responses(&chat_resp, "resp_cmp123", "work/gpt-6-astra", true);
        let outputs = responses_val["output"].as_array().unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0]["type"], "compaction");
        assert_eq!(outputs[0]["encrypted_content"], "Summary of prior turns.");
    }

    #[test]
    fn test_subsequent_request_with_compaction_input() {
        let req_json = json!({
            "model": "work/gpt-6-astra",
            "input": [
                {
                    "type": "compaction",
                    "encrypted_content": "Prior conversation summary checkpoint"
                },
                {
                    "role": "user",
                    "content": "What was the previous topic?"
                }
            ],
            "stream": true
        });

        assert!(!is_compaction_request(&req_json));

        let chat_req = convert_responses_req_to_chat(&req_json);
        assert_eq!(chat_req.messages.len(), 2);
        assert_eq!(chat_req.messages[0].role, "system");
        assert!(chat_req.messages[0].content.as_ref().unwrap().to_string().contains("Prior conversation summary checkpoint"));
        assert_eq!(chat_req.messages[1].role, "user");
    }
}
