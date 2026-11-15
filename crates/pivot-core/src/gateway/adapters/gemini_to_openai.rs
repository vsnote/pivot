use crate::gateway::types::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};

fn extract_gemini_system_text(val: &Value) -> String {
    if let Some(s) = val.as_str() {
        return s.to_string();
    }
    let mut out = String::new();
    if let Some(parts) = val.get("parts").and_then(Value::as_array) {
        for part in parts {
            if let Some(t) = part.get("text").and_then(Value::as_str) {
                if !out.is_empty() && !t.is_empty() {
                    out.push('\n');
                }
                out.push_str(t);
            }
        }
    } else if let Some(arr) = val.as_array() {
        for item in arr {
            if let Some(t) = item.as_str().or_else(|| item.get("text").and_then(Value::as_str)) {
                if !out.is_empty() && !t.is_empty() {
                    out.push('\n');
                }
                out.push_str(t);
            }
        }
    }
    out
}

fn parse_tool_args_object(args_str: &str) -> Value {
    let trimmed = args_str.trim();
    if trimmed.is_empty() {
        return json!({});
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => json!({ "raw": args_str }),
    }
}

fn map_openai_finish_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("length") => "MAX_TOKENS",
        Some("content_filter") => "SAFETY",
        _ => "STOP",
    }
}

/// 将入站 Gemini `generateContent` 请求体转换为内部标准 `OpenAIChatRequest`
pub fn convert_gemini_req_to_openai(req_body: &Value, model: &str) -> OpenAIChatRequest {
    let mut messages = Vec::new();
    let mut pending_call_ids: VecDeque<(String, String)> = VecDeque::new();

    if let Some(sys_val) = req_body
        .get("systemInstruction")
        .or_else(|| req_body.get("system_instruction"))
    {
        let sys_text = extract_gemini_system_text(sys_val);
        if !sys_text.trim().is_empty() {
            messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: Some(Value::String(sys_text)),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
                name: None,
            });
        }
    }

    if let Some(contents) = req_body.get("contents").and_then(Value::as_array) {
        for (msg_idx, content) in contents.iter().enumerate() {
            let role = content
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user");
            let parts = match content.get("parts").and_then(Value::as_array) {
                Some(p) => p,
                None => continue,
            };

            if role == "model" || role == "assistant" {
                let mut thinking_buf = String::new();
                let mut text_buf = String::new();
                let mut tool_calls = Vec::new();

                for (part_idx, part) in parts.iter().enumerate() {
                    if let Some(t) = part.get("text").and_then(Value::as_str) {
                        if part.get("thought").and_then(Value::as_bool) == Some(true) {
                            thinking_buf.push_str(t);
                        } else {
                            text_buf.push_str(t);
                        }
                    }

                    if let Some(fc) = part
                        .get("functionCall")
                        .or_else(|| part.get("function_call"))
                    {
                        let fn_name = fc
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let call_id = fc
                            .get("id")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("call_{}_{}", msg_idx, part_idx));
                        let args_str = fc
                            .get("args")
                            .map(|a| {
                                if a.is_null() {
                                    "{}".to_string()
                                } else {
                                    serde_json::to_string(a).unwrap_or_else(|_| "{}".to_string())
                                }
                            })
                            .unwrap_or_else(|| "{}".to_string());

                        pending_call_ids.push_back((fn_name.clone(), call_id.clone()));
                        tool_calls.push(OpenAIToolCall {
                            index: Some(tool_calls.len()),
                            id: call_id,
                            call_type: "function".to_string(),
                            function: OpenAIFunctionCall {
                                name: fn_name,
                                arguments: args_str,
                            },
                        });
                    }
                }

                let reasoning_content = if thinking_buf.is_empty() {
                    None
                } else {
                    Some(thinking_buf)
                };

                if !text_buf.is_empty() || !tool_calls.is_empty() || reasoning_content.is_some() {
                    let msg_content = if !tool_calls.is_empty() && text_buf.is_empty() {
                        None
                    } else {
                        Some(Value::String(text_buf))
                    };
                    messages.push(OpenAIMessage {
                        role: "assistant".to_string(),
                        content: msg_content,
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
            } else {
                let mut user_text = String::new();
                let mut multimodal_parts: Vec<Value> = Vec::new();
                let mut has_image = false;

                for (part_idx, part) in parts.iter().enumerate() {
                    if let Some(fr) = part
                        .get("functionResponse")
                        .or_else(|| part.get("function_response"))
                    {
                        let fn_name = fr
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let explicit_id = fr
                            .get("id")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string());

                        let call_id = if let Some(id) = explicit_id {
                            if let Some(pos) =
                                pending_call_ids.iter().position(|(_, cid)| cid == &id)
                            {
                                pending_call_ids.remove(pos);
                            }
                            id
                        } else if let Some(pos) =
                            pending_call_ids.iter().position(|(name, _)| name == &fn_name)
                        {
                            pending_call_ids
                                .remove(pos)
                                .map(|(_, cid)| cid)
                                .unwrap_or_else(|| format!("call_resp_{}_{}", msg_idx, part_idx))
                        } else if let Some((_, cid)) = pending_call_ids.pop_front() {
                            cid
                        } else {
                            format!("call_resp_{}_{}", msg_idx, part_idx)
                        };

                        let resp_str = match fr.get("response") {
                            Some(Value::String(s)) => s.clone(),
                            Some(other) => serde_json::to_string(other).unwrap_or_default(),
                            None => "{}".to_string(),
                        };

                        messages.push(OpenAIMessage {
                            role: "tool".to_string(),
                            content: Some(Value::String(resp_str)),
                            tool_calls: None,
                            tool_call_id: Some(call_id),
                            reasoning_content: None,
                            name: if fn_name.is_empty() {
                                None
                            } else {
                                Some(fn_name)
                            },
                        });
                        continue;
                    }

                    if let Some(t) = part.get("text").and_then(Value::as_str) {
                        if !t.is_empty() {
                            user_text.push_str(t);
                            multimodal_parts.push(json!({ "type": "text", "text": t }));
                        }
                    }

                    if let Some(inline) = part
                        .get("inlineData")
                        .or_else(|| part.get("inline_data"))
                    {
                        let data = inline
                            .get("data")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if !data.is_empty() {
                            let mime = inline
                                .get("mimeType")
                                .or_else(|| inline.get("mime_type"))
                                .and_then(Value::as_str)
                                .filter(|s| !s.is_empty())
                                .unwrap_or("image/png");
                            has_image = true;
                            multimodal_parts.push(json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:{};base64,{}", mime, data)
                                }
                            }));
                        }
                    }
                }

                if has_image {
                    messages.push(OpenAIMessage {
                        role: "user".to_string(),
                        content: Some(Value::Array(multimodal_parts)),
                        tool_calls: None,
                        tool_call_id: None,
                        reasoning_content: None,
                        name: None,
                    });
                } else if !user_text.is_empty() {
                    messages.push(OpenAIMessage {
                        role: "user".to_string(),
                        content: Some(Value::String(user_text)),
                        tool_calls: None,
                        tool_call_id: None,
                        reasoning_content: None,
                        name: None,
                    });
                }
            }
        }
    }

    if !messages.iter().any(|m| m.role != "system") {
        messages.push(OpenAIMessage {
            role: "user".to_string(),
            content: Some(Value::String(String::new())),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            name: None,
        });
    }

    let mut tools = Vec::new();
    if let Some(tool_arr) = req_body.get("tools").and_then(Value::as_array) {
        for tool_entry in tool_arr {
            let decls_opt = tool_entry
                .get("functionDeclarations")
                .or_else(|| tool_entry.get("function_declarations"))
                .and_then(Value::as_array);
            if let Some(decls) = decls_opt {
                for decl in decls {
                    let Some(name) = decl
                        .get("name")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                    else {
                        continue;
                    };
                    let description = decl
                        .get("description")
                        .and_then(Value::as_str)
                        .map(|s| s.to_string());
                    let parameters = decl
                        .get("parameters")
                        .or_else(|| decl.get("parametersJsonSchema"))
                        .or_else(|| decl.get("parameters_json_schema"))
                        .cloned()
                        .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
                    tools.push(OpenAITool {
                        tool_type: "function".to_string(),
                        function: OpenAIFunction {
                            name: name.to_string(),
                            description,
                            parameters,
                        },
                    });
                }
            }
        }
    }

    let tool_choice = req_body
        .get("toolConfig")
        .or_else(|| req_body.get("tool_config"))
        .and_then(|tc| {
            tc.get("functionCallingConfig")
                .or_else(|| tc.get("function_calling_config"))
        })
        .and_then(|fcc| fcc.get("mode"))
        .and_then(Value::as_str)
        .and_then(|mode| match mode.to_ascii_uppercase().as_str() {
            "NONE" => Some(json!("none")),
            "ANY" => Some(json!("required")),
            "AUTO" => Some(json!("auto")),
            _ => None,
        });

    let gen_cfg = req_body
        .get("generationConfig")
        .or_else(|| req_body.get("generation_config"));

    let temperature = gen_cfg
        .and_then(|g| g.get("temperature"))
        .and_then(Value::as_f64);
    let max_tokens = gen_cfg
        .and_then(|g| g.get("maxOutputTokens").or_else(|| g.get("max_output_tokens")))
        .and_then(Value::as_u64)
        .map(|v| v as u32);
    let reasoning_effort = gen_cfg
        .and_then(|g| g.get("thinkingConfig").or_else(|| g.get("thinking_config")))
        .and_then(|tc| {
            if let Some(budget) = tc
                .get("thinkingBudget")
                .or_else(|| tc.get("thinking_budget"))
                .and_then(Value::as_i64)
            {
                if budget >= 16384 {
                    Some("high".to_string())
                } else if budget >= 4096 {
                    Some("medium".to_string())
                } else if budget > 0 {
                    Some("low".to_string())
                } else {
                    None
                }
            } else {
                tc.get("thinkingLevel")
                    .or_else(|| tc.get("thinking_level"))
                    .and_then(Value::as_str)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .filter(|s| !s.is_empty())
            }
        });

    OpenAIChatRequest {
        model: model.to_string(),
        messages,
        temperature,
        max_tokens,
        max_completion_tokens: None,
        reasoning_effort,
        tools: if tools.is_empty() { None } else { Some(tools) },
        tool_choice,
        stream: None,
        stream_options: None,
    }
}

/// 将 OpenAI 兼容的非流式 `chat.completion` JSON 转换为 Gemini `GenerateContentResponse` JSON
pub fn convert_openai_resp_to_gemini(chat_json: &Value) -> Value {
    let mut parts: Vec<Value> = Vec::new();
    let choice0 = chat_json
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first());

    if let Some(msg) = choice0.and_then(|c| c.get("message")) {
        if let Some(reasoning) = msg
            .get("reasoning_content")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            parts.push(json!({ "thought": true, "text": reasoning }));
        }

        if let Some(content) = msg
            .get("content")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            parts.push(json!({ "text": content }));
        }

        if let Some(tcs) = msg.get("tool_calls").and_then(Value::as_array) {
            for tc in tcs {
                let id = tc.get("id").and_then(Value::as_str).unwrap_or("");
                let func = tc.get("function");
                let name = func
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let args_str = func
                    .and_then(|f| f.get("arguments"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let args_obj = parse_tool_args_object(args_str);
                parts.push(json!({
                    "functionCall": {
                        "id": id,
                        "name": name,
                        "args": args_obj
                    }
                }));
            }
        }
    }

    if parts.is_empty() {
        parts.push(json!({ "text": "" }));
    }

    let finish_reason = map_openai_finish_reason(
        choice0
            .and_then(|c| c.get("finish_reason"))
            .and_then(Value::as_str),
    );

    let usage = chat_json.get("usage");
    let prompt_tokens = usage
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = usage
        .and_then(|u| u.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .and_then(|u| u.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(prompt_tokens + completion_tokens);

    json!({
        "candidates": [{
            "content": {
                "role": "model",
                "parts": parts
            },
            "finishReason": finish_reason,
            "index": 0
        }],
        "usageMetadata": {
            "promptTokenCount": prompt_tokens,
            "candidatesTokenCount": completion_tokens,
            "totalTokenCount": total_tokens
        }
    })
}

/// 将 OpenAI SSE chunk 逐行状态化转换为 Gemini `streamGenerateContent` SSE 事件
#[derive(Default)]
pub struct OpenAiToGeminiSseState {
    pub tool_calls: BTreeMap<usize, (String, String, String)>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub pending_finish_reason: Option<String>,
    pub emitted_any_part: bool,
    pub finished: bool,
}

impl OpenAiToGeminiSseState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process_line(&mut self, line: &str) -> Vec<String> {
        let trimmed = line.trim();
        if !trimmed.starts_with("data:") {
            return Vec::new();
        }
        let payload = trimmed.trim_start_matches("data:").trim();
        if payload == "[DONE]" {
            return self.finish();
        }
        let Ok(val) = serde_json::from_str::<Value>(payload) else {
            return Vec::new();
        };

        if let Some(usage) = val.get("usage") {
            if let Some(pt) = usage.get("prompt_tokens").and_then(Value::as_u64) {
                self.prompt_tokens = pt as u32;
            }
            if let Some(ct) = usage.get("completion_tokens").and_then(Value::as_u64) {
                self.completion_tokens = ct as u32;
            }
        }

        let mut events = Vec::new();
        if let Some(choice0) = val
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
        {
            if let Some(delta) = choice0.get("delta") {
                if let Some(reasoning) = delta
                    .get("reasoning_content")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                {
                    let chunk = json!({
                        "candidates": [{
                            "content": {
                                "role": "model",
                                "parts": [{ "thought": true, "text": reasoning }]
                            },
                            "index": 0
                        }]
                    });
                    events.push(format!("data: {}\n\n", chunk));
                    self.emitted_any_part = true;
                }

                if let Some(text) = delta
                    .get("content")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                {
                    let chunk = json!({
                        "candidates": [{
                            "content": {
                                "role": "model",
                                "parts": [{ "text": text }]
                            },
                            "index": 0
                        }]
                    });
                    events.push(format!("data: {}\n\n", chunk));
                    self.emitted_any_part = true;
                }

                if let Some(tcs) = delta.get("tool_calls").and_then(Value::as_array) {
                    for tc in tcs {
                        let idx = tc.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                        let entry = self
                            .tool_calls
                            .entry(idx)
                            .or_insert_with(|| (String::new(), String::new(), String::new()));
                        if let Some(id) = tc
                            .get("id")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                        {
                            entry.0 = id.to_string();
                        }
                        if let Some(func) = tc.get("function") {
                            if let Some(name) = func
                                .get("name")
                                .and_then(Value::as_str)
                                .filter(|s| !s.is_empty())
                            {
                                entry.1 = name.to_string();
                            }
                            if let Some(args) = func.get("arguments").and_then(Value::as_str) {
                                entry.2.push_str(args);
                            }
                        }
                    }
                }
            }

            if let Some(reason) = choice0.get("finish_reason").and_then(Value::as_str) {
                self.pending_finish_reason = Some(reason.to_string());
            }
        }

        events
    }

    pub fn finish(&mut self) -> Vec<String> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;

        let mut parts: Vec<Value> = Vec::new();
        for (idx, (id, name, args_str)) in std::mem::take(&mut self.tool_calls) {
            let final_id = if id.is_empty() {
                format!("call_stream_{}", idx)
            } else {
                id
            };
            let args_obj = parse_tool_args_object(&args_str);
            parts.push(json!({
                "functionCall": {
                    "id": final_id,
                    "name": name,
                    "args": args_obj
                }
            }));
        }

        if parts.is_empty() {
            parts.push(json!({ "text": "" }));
        }

        let finish_reason = map_openai_finish_reason(self.pending_finish_reason.as_deref());
        let total = self.prompt_tokens + self.completion_tokens;
        let final_chunk = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": parts
                },
                "finishReason": finish_reason,
                "index": 0
            }],
            "usageMetadata": {
                "promptTokenCount": self.prompt_tokens,
                "candidatesTokenCount": self.completion_tokens,
                "totalTokenCount": total
            }
        });
        vec![format!("data: {}\n\n", final_chunk)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_gemini_req_to_openai_full_roundtrip_features() {
        let gemini_req = json!({
            "systemInstruction": {
                "parts": [{ "text": "You are a helpful coding assistant." }]
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        { "text": "Read Cargo.toml" },
                        { "inlineData": { "mimeType": "image/png", "data": "iVBORw0KGgo=" } }
                    ]
                },
                {
                    "role": "model",
                    "parts": [
                        { "thought": true, "text": "Need to inspect Cargo.toml first." },
                        { "functionCall": { "name": "read_file", "args": { "path": "Cargo.toml" } } }
                    ]
                },
                {
                    "role": "user",
                    "parts": [
                        {
                            "functionResponse": {
                                "name": "read_file",
                                "response": { "content": "[package]\nname = \"pivot\"" }
                            }
                        }
                    ]
                }
            ],
            "tools": [{
                "functionDeclarations": [{
                    "name": "read_file",
                    "description": "Read file from disk",
                    "parametersJsonSchema": {
                        "type": "object",
                        "properties": { "path": { "type": "string" } },
                        "required": ["path"]
                    }
                }]
            }],
            "toolConfig": {
                "functionCallingConfig": { "mode": "ANY" }
            },
            "generationConfig": {
                "temperature": 0.2,
                "maxOutputTokens": 4096,
                "thinkingConfig": { "thinkingBudget": 16384 }
            }
        });

        let oai = convert_gemini_req_to_openai(&gemini_req, "deepseek-chat");
        assert_eq!(oai.model, "deepseek-chat");
        assert_eq!(oai.temperature, Some(0.2));
        assert_eq!(oai.max_tokens, Some(4096));
        assert_eq!(oai.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(oai.tool_choice, Some(json!("required")));

        assert_eq!(oai.messages.len(), 4);
        assert_eq!(oai.messages[0].role, "system");
        assert_eq!(oai.messages[1].role, "user");
        assert!(oai.messages[1].content.as_ref().unwrap().is_array());

        let assistant = &oai.messages[2];
        assert_eq!(assistant.role, "assistant");
        assert_eq!(
            assistant.reasoning_content.as_deref(),
            Some("Need to inspect Cargo.toml first.")
        );
        let tc = &assistant.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc.id, "call_1_1");
        assert_eq!(tc.function.name, "read_file");

        let tool_msg = &oai.messages[3];
        assert_eq!(tool_msg.role, "tool");
        assert_eq!(tool_msg.tool_call_id.as_deref(), Some("call_1_1"));
        assert_eq!(tool_msg.name.as_deref(), Some("read_file"));

        let tools = oai.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "read_file");
        assert_eq!(tools[0].function.parameters["required"], json!(["path"]));
    }

    #[test]
    fn test_convert_openai_resp_to_gemini_and_sse() {
        let oai_resp = json!({
            "id": "chatcmpl_1",
            "object": "chat.completion",
            "created": 1,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "reasoning_content": "Let me check",
                    "content": "Checking now.",
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": { "name": "ls", "arguments": "{\"path\":\".\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": { "prompt_tokens": 120, "completion_tokens": 35, "total_tokens": 155 }
        });

        let gem = convert_openai_resp_to_gemini(&oai_resp);
        let parts = gem["candidates"][0]["content"]["parts"].as_array().unwrap();
        assert_eq!(parts[0], json!({ "thought": true, "text": "Let me check" }));
        assert_eq!(parts[1], json!({ "text": "Checking now." }));
        assert_eq!(
            parts[2],
            json!({ "functionCall": { "id": "call_abc", "name": "ls", "args": { "path": "." } } })
        );
        assert_eq!(gem["candidates"][0]["finishReason"], "STOP");
        assert_eq!(gem["usageMetadata"]["promptTokenCount"], 120);
        assert_eq!(gem["usageMetadata"]["candidatesTokenCount"], 35);

        let mut sse = OpenAiToGeminiSseState::new();
        let e1 = sse.process_line(r#"data: {"choices":[{"index":0,"delta":{"reasoning_content":"Think "}}]}"#);
        assert_eq!(e1.len(), 1);
        assert!(e1[0].contains(r#""thought":true"#));

        let e2 = sse.process_line(r#"data: {"choices":[{"index":0,"delta":{"content":"Done"}}]}"#);
        assert_eq!(e2.len(), 1);
        assert!(e2[0].contains(r#""text":"Done""#));

        let e3 = sse.process_line(r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"tc_1","function":{"name":"run","arguments":"{\"cmd\":"}}]}}]}"#);
        assert!(e3.is_empty());
        let e4 = sse.process_line(r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"pwd\"}"}}]},"finish_reason":"tool_calls"}]}"#);
        assert!(e4.is_empty());
        let e5 = sse.process_line(r#"data: {"choices":[],"usage":{"prompt_tokens":50,"completion_tokens":12}}"#);
        assert!(e5.is_empty());

        let done = sse.process_line("data: [DONE]");
        assert_eq!(done.len(), 1);
        let final_json: Value = serde_json::from_str(done[0].trim().trim_start_matches("data:").trim()).unwrap();
        assert_eq!(
            final_json["candidates"][0]["content"]["parts"][0],
            json!({ "functionCall": { "id": "tc_1", "name": "run", "args": { "cmd": "pwd" } } })
        );
        assert_eq!(final_json["usageMetadata"]["promptTokenCount"], 50);
        assert_eq!(final_json["usageMetadata"]["candidatesTokenCount"], 12);
    }
}
