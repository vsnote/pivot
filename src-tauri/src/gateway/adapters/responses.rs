use crate::gateway::types::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

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
    pub custom_tools: HashSet<String>,
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
        let mut custom_tools = HashSet::new();
        custom_tools.insert("exec".to_string());
        Self::new_full(response_id, model, is_compaction, custom_tools)
    }

    pub fn new_full(
        response_id: &str,
        model: &str,
        is_compaction: bool,
        custom_tools: HashSet<String>,
    ) -> Self {
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
            custom_tools,
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

        let mut output = Vec::new();

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
            output.push(comp_item);
        } else {
            let mut next_output_index = 0;
            if self.text_started && !self.full_text.is_empty() {
                events.push(Self::sse(
                    "response.output_text.done",
                    &json!({
                        "type": "response.output_text.done",
                        "item_id": self.item_id,
                        "output_index": next_output_index,
                        "content_index": 0,
                        "text": self.full_text
                    }),
                ));
                let msg_item = json!({
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
                });
                events.push(Self::sse(
                    "response.output_item.done",
                    &json!({
                        "type": "response.output_item.done",
                        "output_index": next_output_index,
                        "item": msg_item
                    }),
                ));
                output.push(msg_item);
                next_output_index += 1;
            }

            if self.in_tool_call && !self.tool_calls_buf.is_empty() {
                for (_idx, buf) in &self.tool_calls_buf {
                    let call_id = if buf.id.is_empty() {
                        format!("call_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..16].to_string())
                    } else {
                        buf.id.clone()
                    };

                    let is_custom = self.custom_tools.contains(&buf.name) || buf.name == "exec";
                    let tc_item = if is_custom {
                        let input_val = extract_custom_tool_input(&buf.arguments, &buf.name);
                        json!({
                            "id": format!("ctc_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..16].to_string()),
                            "object": "response.output_item",
                            "type": "custom_tool_call",
                            "call_id": call_id,
                            "name": buf.name,
                            "input": input_val,
                            "status": "completed"
                        })
                    } else {
                        json!({
                            "id": format!("fc_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..16].to_string()),
                            "object": "response.output_item",
                            "type": "function_call",
                            "call_id": call_id,
                            "name": buf.name,
                            "arguments": buf.arguments,
                            "status": "completed"
                        })
                    };

                    events.push(Self::sse(
                        "response.output_item.added",
                        &json!({
                            "type": "response.output_item.added",
                            "output_index": next_output_index,
                            "item": tc_item
                        }),
                    ));
                    events.push(Self::sse(
                        "response.output_item.done",
                        &json!({
                            "type": "response.output_item.done",
                            "output_index": next_output_index,
                            "item": tc_item
                        }),
                    ));
                    output.push(tc_item);
                    next_output_index += 1;
                }
            } else if output.is_empty() {
                let empty_item = json!({
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
                });
                output.push(empty_item);
            }
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

/// Extract raw custom tool input (e.g. JavaScript code for `exec`) from arguments
pub fn extract_custom_tool_input(arguments: &str, tool_name: &str) -> String {
    if tool_name == "exec" {
        if let Ok(val) = serde_json::from_str::<Value>(arguments) {
            if let Some(code) = val.get("code").and_then(Value::as_str) {
                return code.to_string();
            }
            if let Some(code) = val.get("input").and_then(Value::as_str) {
                return code.to_string();
            }
        }
    }
    arguments.to_string()
}

/// Extract names of tools configured as custom tools (e.g. `exec`) from incoming Responses request
pub fn extract_custom_tool_names(body: &Value) -> HashSet<String> {
    let mut set = HashSet::new();
    set.insert("exec".to_string());
    if let Some(arr) = body.get("input").or_else(|| body.get("messages")).and_then(Value::as_array) {
        for item in arr {
            if item.get("type").and_then(Value::as_str) == Some("additional_tools") {
                if let Some(tools) = item.get("tools").and_then(Value::as_array) {
                    collect_custom_tools_recursive(tools, &mut set);
                }
            }
        }
    }
    if let Some(tools) = body.get("tools").and_then(Value::as_array) {
        collect_custom_tools_recursive(tools, &mut set);
    }
    set
}

fn collect_custom_tools_recursive(tools: &[Value], set: &mut HashSet<String>) {
    for t in tools {
        let t_type = t.get("type").and_then(Value::as_str).unwrap_or("");
        if t_type == "namespace" {
            if let Some(sub) = t.get("tools").and_then(Value::as_array) {
                collect_custom_tools_recursive(sub, set);
            }
        } else if t_type == "custom" {
            if let Some(name) = t.get("name").and_then(Value::as_str) {
                if !name.is_empty() {
                    set.insert(name.to_string());
                }
            }
        }
    }
}

fn parse_tools_list(tools_arr: &[Value], chat_tools: &mut Vec<OpenAITool>) {
    for t in tools_arr {
        let t_type = t.get("type").and_then(Value::as_str).unwrap_or("");
        if t_type == "namespace" {
            if let Some(sub_tools) = t.get("tools").and_then(Value::as_array) {
                parse_tools_list(sub_tools, chat_tools);
            }
        } else if let Some(func_val) = t.get("function") {
            if let Ok(func) = serde_json::from_value::<OpenAIFunction>(func_val.clone()) {
                if !chat_tools.iter().any(|existing| existing.function.name == func.name) {
                    chat_tools.push(OpenAITool {
                        tool_type: "function".to_string(),
                        function: func,
                    });
                }
            }
        } else {
            let name = t.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let description = t.get("description").and_then(Value::as_str).map(|s| s.to_string());
            let parameters = if let Some(p) = t.get("parameters") {
                p.clone()
            } else if t_type == "custom" || name == "exec" {
                json!({
                    "type": "object",
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "JavaScript code to execute in isolate"
                        }
                    },
                    "required": ["code"]
                })
            } else {
                json!({})
            };

            if !chat_tools.iter().any(|existing| existing.function.name == name) {
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
    let mut chat_tools: Vec<OpenAITool> = Vec::new();

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

                if itype == "additional_tools" {
                    if let Some(tools_arr) = item.get("tools").and_then(Value::as_array) {
                        parse_tools_list(tools_arr, &mut chat_tools);
                    }
                    continue;
                }

                if itype == "reasoning" {
                    continue;
                }

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

                if itype == "function_call_output" || itype == "custom_tool_call_output" {
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
                            } else if let Some(arr) = v.as_array() {
                                let mut text_buf = String::new();
                                for part in arr {
                                    if let Some(s) = part.as_str() {
                                        text_buf.push_str(s);
                                    } else if let Some(t) = part.get("text").and_then(Value::as_str) {
                                        text_buf.push_str(t);
                                    } else {
                                        text_buf.push_str(&part.to_string());
                                    }
                                }
                                text_buf
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
                } else if itype == "function_call" || itype == "custom_tool_call" {
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
                    let raw_args = if itype == "custom_tool_call" {
                        item.get("input")
                    } else {
                        item.get("arguments")
                    };
                    let arguments = raw_args
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                if itype == "custom_tool_call" && (name == "exec" || !s.trim().starts_with('{')) {
                                    json!({"code": s}).to_string()
                                } else {
                                    s.to_string()
                                }
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

    // 3. tools (merge with tools from body.tools)
    if let Some(tools_arr) = body.get("tools").and_then(Value::as_array) {
        parse_tools_list(tools_arr, &mut chat_tools);
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
    let mut custom_tools = HashSet::new();
    custom_tools.insert("exec".to_string());
    chat_response_to_responses_full(chat_resp, response_id, model, is_compaction, &custom_tools)
}

pub fn chat_response_to_responses_full(
    chat_resp: &Value,
    response_id: &str,
    model: &str,
    is_compaction: bool,
    custom_tools: &HashSet<String>,
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
    } else {
        let text = message.get("content").and_then(Value::as_str).unwrap_or("");
        let has_tool_calls = message
            .get("tool_calls")
            .and_then(Value::as_array)
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        if !text.is_empty() || !has_tool_calls {
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

        if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
            for tc in tool_calls {
                let fn_obj = tc.get("function").cloned().unwrap_or(Value::Null);
                let name = fn_obj.get("name").and_then(Value::as_str).unwrap_or("");
                let args = fn_obj.get("arguments").and_then(Value::as_str).unwrap_or("{}");
                let call_id = tc.get("id").and_then(Value::as_str).unwrap_or("call_default");

                let is_custom = custom_tools.contains(name) || name == "exec";
                if is_custom {
                    let input_val = extract_custom_tool_input(args, name);
                    output.push(json!({
                        "id": format!("ctc_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..16].to_string()),
                        "object": "response.output_item",
                        "type": "custom_tool_call",
                        "call_id": call_id,
                        "name": name,
                        "input": input_val,
                        "status": "completed"
                    }));
                } else {
                    output.push(json!({
                        "id": format!("fc_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..16].to_string()),
                        "object": "response.output_item",
                        "type": "function_call",
                        "call_id": call_id,
                        "name": name,
                        "arguments": args,
                        "status": "completed"
                    }));
                }
            }
        }
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

/// Convert an `OpenAIChatRequest` into a Responses API JSON body for Codex / ChatGPT subscription backend.
pub fn convert_chat_req_to_responses(req: &OpenAIChatRequest, target_model: &str) -> Value {
    let mut input_items: Vec<Value> = Vec::new();
    let mut instructions: Option<String> = None;

    for msg in &req.messages {
        let role = msg.role.as_str();
        if role == "system" {
            let text = if let Some(ref c) = msg.content {
                if let Some(s) = c.as_str() {
                    s.to_string()
                } else {
                    c.to_string()
                }
            } else {
                String::new()
            };
            if instructions.is_none() {
                instructions = Some(text);
            } else if let Some(ref mut inst) = instructions {
                inst.push_str("\n\n");
                inst.push_str(&text);
            }
            continue;
        }

        if role == "user" {
            let content_val = msg.content.clone().unwrap_or(Value::Null);
            input_items.push(json!({
                "type": "message",
                "role": "user",
                "content": if let Some(s) = content_val.as_str() {
                    json!([{ "type": "input_text", "text": s }])
                } else if content_val.is_array() {
                    content_val
                } else {
                    json!([{ "type": "input_text", "text": content_val.to_string() }])
                }
            }));
        } else if role == "assistant" {
            if let Some(ref c) = msg.content {
                let text = c.as_str().unwrap_or("");
                if !text.is_empty() {
                    input_items.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": text }]
                    }));
                }
            }
            if let Some(ref tool_calls) = msg.tool_calls {
                for tc in tool_calls {
                    input_items.push(json!({
                        "type": "function_call",
                        "call_id": tc.id,
                        "name": tc.function.name,
                        "arguments": tc.function.arguments
                    }));
                }
            }
        } else if role == "tool" {
            let output_str = if let Some(ref c) = msg.content {
                if let Some(s) = c.as_str() {
                    s.to_string()
                } else {
                    c.to_string()
                }
            } else {
                String::new()
            };
            let call_id = msg.tool_call_id.as_deref().unwrap_or("call_default");
            input_items.push(json!({
                "type": "function_call_output",
                "call_id": call_id,
                "output": output_str
            }));
        }
    }

    let mut body = json!({
        "model": target_model,
        "store": false,
        "stream": true,
        "input": input_items
    });

    if let Some(inst) = instructions {
        body["instructions"] = json!(inst);
    }

    if let Some(ref tools) = req.tools {
        let resp_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "name": t.function.name,
                    "description": t.function.description.as_deref().unwrap_or(""),
                    "parameters": t.function.parameters
                })
            })
            .collect();
        if !resp_tools.is_empty() {
            body["tools"] = json!(resp_tools);
        }
    }

    body
}

/// Parses streaming SSE chunks from Codex / ChatGPT Responses API and converts them into OpenAI Chat Completion chunks.
#[derive(Debug, Clone)]
pub struct ResponsesToChatSseParser {
    pub id: String,
    pub model: String,
    pub created: i64,
    pub current_event: String,
    pub has_tool_calls: bool,
    pub completed: bool,
}

impl ResponsesToChatSseParser {
    pub fn new(req_id: &str, model: &str) -> Self {
        Self {
            id: format!("chatcmpl_{}", req_id.replace('-', "")),
            model: model.to_string(),
            created: chrono::Utc::now().timestamp(),
            current_event: String::new(),
            has_tool_calls: false,
            completed: false,
        }
    }

    pub fn process_line(&mut self, line: &str) -> Vec<String> {
        let trimmed = line.trim();
        if trimmed.starts_with("event:") {
            self.current_event = trimmed["event:".len()..].trim().to_string();
            return Vec::new();
        }

        if !trimmed.starts_with("data:") {
            return Vec::new();
        }

        let payload = trimmed["data:".len()..].trim();
        if payload == "[DONE]" {
            self.completed = true;
            return vec!["data: [DONE]\n\n".to_string()];
        }

        let Ok(data) = serde_json::from_str::<Value>(payload) else {
            return Vec::new();
        };

        let ev_type = data.get("type").and_then(Value::as_str).unwrap_or(&self.current_event);
        let mut results = Vec::new();

        if ev_type == "response.output_text.delta" {
            if let Some(delta) = data.get("delta").and_then(Value::as_str) {
                let chunk = json!({
                    "id": self.id,
                    "object": "chat.completion.chunk",
                    "created": self.created,
                    "model": self.model,
                    "choices": [{
                        "index": 0,
                        "delta": { "content": delta },
                        "finish_reason": null
                    }]
                });
                results.push(format!("data: {}\n\n", serde_json::to_string(&chunk).unwrap_or_default()));
            }
        } else if ev_type == "response.output_item.added" {
            if let Some(item) = data.get("item") {
                if item.get("type").and_then(Value::as_str) == Some("function_call") {
                    self.has_tool_calls = true;
                    let out_idx = data.get("output_index").and_then(Value::as_u64).unwrap_or(0);
                    let call_id = item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or("call_default");
                    let name = item.get("name").and_then(Value::as_str).unwrap_or("");
                    let chunk = json!({
                        "id": self.id,
                        "object": "chat.completion.chunk",
                        "created": self.created,
                        "model": self.model,
                        "choices": [{
                            "index": 0,
                            "delta": {
                                "tool_calls": [{
                                    "index": out_idx,
                                    "id": call_id,
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
                    results.push(format!("data: {}\n\n", serde_json::to_string(&chunk).unwrap_or_default()));
                }
            }
        } else if ev_type == "response.function_call_arguments.delta" {
            let out_idx = data.get("output_index").and_then(Value::as_u64).unwrap_or(0);
            if let Some(delta) = data.get("delta").and_then(Value::as_str) {
                let chunk = json!({
                    "id": self.id,
                    "object": "chat.completion.chunk",
                    "created": self.created,
                    "model": self.model,
                    "choices": [{
                        "index": 0,
                        "delta": {
                            "tool_calls": [{
                                "index": out_idx,
                                "function": {
                                    "arguments": delta
                                }
                            }]
                        },
                        "finish_reason": null
                    }]
                });
                results.push(format!("data: {}\n\n", serde_json::to_string(&chunk).unwrap_or_default()));
            }
        } else if ev_type == "response.completed" {
            self.completed = true;
            let finish_reason = if self.has_tool_calls { "tool_calls" } else { "stop" };
            let usage = data.get("response").and_then(|r| r.get("usage"));
            let mut chunk = json!({
                "id": self.id,
                "object": "chat.completion.chunk",
                "created": self.created,
                "model": self.model,
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": finish_reason
                }]
            });
            if let Some(u) = usage {
                chunk["usage"] = json!({
                    "prompt_tokens": u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0),
                    "completion_tokens": u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0),
                    "total_tokens": u.get("total_tokens").and_then(Value::as_u64).unwrap_or(0)
                });
            }
            results.push(format!("data: {}\n\n", serde_json::to_string(&chunk).unwrap_or_default()));
            results.push("data: [DONE]\n\n".to_string());
        }

        results
    }

    pub fn finish(&mut self) -> Vec<String> {
        if !self.completed {
            self.completed = true;
            vec!["data: [DONE]\n\n".to_string()]
        } else {
            Vec::new()
        }
    }
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

    #[test]
    fn test_additional_tools_and_custom_tool_calls() {
        let req_json = json!({
            "model": "work/gpt-6-astra",
            "instructions": "You are Codex",
            "input": [
                {
                    "type": "additional_tools",
                    "id": "at_123",
                    "role": "developer",
                    "tools": [
                        {
                            "type": "namespace",
                            "name": "functions",
                            "tools": [
                                {
                                    "type": "custom",
                                    "name": "exec",
                                    "description": "Run JavaScript code to orchestrate tools"
                                },
                                {
                                    "type": "function",
                                    "name": "wait",
                                    "description": "Wait for execution",
                                    "parameters": {
                                        "type": "object",
                                        "properties": {
                                            "cell_id": { "type": "string" }
                                        },
                                        "required": ["cell_id"]
                                    }
                                }
                            ]
                        }
                    ]
                },
                {
                    "type": "reasoning",
                    "id": "rs_123",
                    "summary": [{ "type": "summary_text", "text": "Plan steps" }]
                },
                {
                    "role": "user",
                    "content": "Deploy project"
                },
                {
                    "type": "custom_tool_call",
                    "id": "ctc_123",
                    "call_id": "call_abc",
                    "name": "exec",
                    "input": "const res = await tools.exec_command({cmd: 'git status'});"
                },
                {
                    "type": "custom_tool_call_output",
                    "id": "ctco_123",
                    "call_id": "call_abc",
                    "output": [
                        { "type": "input_text", "text": "Clean working tree\n" }
                    ]
                }
            ],
            "stream": true
        });

        let custom_tools = extract_custom_tool_names(&req_json);
        assert!(custom_tools.contains("exec"));

        let chat_req = convert_responses_req_to_chat(&req_json);
        assert_eq!(chat_req.model, "work/gpt-6-astra");

        // System message + user message + assistant tool call + tool output = 4 messages
        assert_eq!(chat_req.messages.len(), 4);
        assert_eq!(chat_req.messages[0].role, "system");
        assert_eq!(chat_req.messages[1].role, "user");
        assert_eq!(chat_req.messages[2].role, "assistant");
        let tc = chat_req.messages[2].tool_calls.as_ref().unwrap();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].id, "call_abc");
        assert_eq!(tc[0].function.name, "exec");
        assert!(tc[0].function.arguments.contains("const res = await tools.exec_command"));

        assert_eq!(chat_req.messages[3].role, "tool");
        assert_eq!(chat_req.messages[3].tool_call_id.as_deref(), Some("call_abc"));
        assert_eq!(chat_req.messages[3].content.as_ref().unwrap(), &json!("Clean working tree\n"));

        // Tools extracted from additional_tools
        let tools = chat_req.tools.unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].function.name, "exec");
        assert!(tools[0].function.parameters.get("properties").and_then(|p| p.get("code")).is_some());
        assert_eq!(tools[1].function.name, "wait");
    }

    #[test]
    fn test_custom_tool_response_streaming_and_non_streaming() {
        let mut custom_tools = HashSet::new();
        custom_tools.insert("exec".to_string());

        // 1. Streaming
        let mut state = ResponsesSseState::new_full("resp_test_custom", "work/gpt-6-astra", false, custom_tools.clone());

        let chunk1 = "data: {\"choices\": [{\"delta\": {\"tool_calls\": [{\"index\": 0, \"id\": \"call_custom_1\", \"function\": {\"name\": \"exec\", \"arguments\": \"{\\\"code\\\":\\\"console.log(123);\\\"}\"}}]}}]}";
        let _ = state.process_line(chunk1);
        let evs = state.process_line("data: [DONE]");

        let added_ev = evs.iter().find(|e| e.contains("response.output_item.added")).unwrap();
        assert!(added_ev.contains("\"type\":\"custom_tool_call\""));
        assert!(added_ev.contains("\"name\":\"exec\""));
        assert!(added_ev.contains("\"input\":\"console.log(123);\""));

        let completed_ev = evs.iter().find(|e| e.contains("response.completed")).unwrap();
        assert!(completed_ev.contains("\"type\":\"custom_tool_call\""));

        // 2. Non-streaming
        let chat_resp = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [
                        {
                            "id": "call_custom_2",
                            "type": "function",
                            "function": {
                                "name": "exec",
                                "arguments": "{\"code\":\"console.log(456);\"}"
                            }
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let responses_val = chat_response_to_responses_full(&chat_resp, "resp_test_custom2", "work/gpt-6-astra", false, &custom_tools);
        let outputs = responses_val["output"].as_array().unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0]["type"], "custom_tool_call");
        assert_eq!(outputs[0]["name"], "exec");
        assert_eq!(outputs[0]["input"], "console.log(456);");
    }

    #[test]
    fn test_convert_chat_req_to_responses_and_chat_parser() {
        let chat_req = OpenAIChatRequest {
            model: "gpt-5.6-sol".to_string(),
            messages: vec![
                OpenAIMessage {
                    role: "system".to_string(),
                    content: Some(json!("You are helpful")),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                    name: None,
                },
                OpenAIMessage {
                    role: "user".to_string(),
                    content: Some(json!("Hello")),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                    name: None,
                },
            ],
            temperature: None,
            max_tokens: None,
            max_completion_tokens: None,
            reasoning_effort: None,
            tools: None,
            tool_choice: None,
            stream: Some(true),
            stream_options: None,
        };

        let responses_val = convert_chat_req_to_responses(&chat_req, "gpt-5.6-sol");
        assert_eq!(responses_val["model"], "gpt-5.6-sol");
        assert_eq!(responses_val["instructions"], "You are helpful");
        let input_arr = responses_val["input"].as_array().unwrap();
        assert_eq!(input_arr.len(), 1);
        assert_eq!(input_arr[0]["role"], "user");

        // Test parser
        let mut parser = ResponsesToChatSseParser::new("test_req_1", "gpt-5.6-sol");
        let line1 = "event: response.output_text.delta";
        let res1 = parser.process_line(line1);
        assert!(res1.is_empty());

        let line2 = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hi there\"}";
        let res2 = parser.process_line(line2);
        assert_eq!(res2.len(), 1);
        assert!(res2[0].contains("Hi there"));
        assert!(res2[0].contains("chat.completion.chunk"));

        let line3 = "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":2,\"total_tokens\":7}}}";
        let res3 = parser.process_line(line3);
        assert_eq!(res3.len(), 2);
        assert!(res3[0].contains("\"prompt_tokens\":5"));
        assert_eq!(res3[1], "data: [DONE]\n\n");
    }
}
