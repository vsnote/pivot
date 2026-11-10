use crate::gateway::types::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static THOUGHT_SIGNATURE_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn get_cache() -> &'static Mutex<HashMap<String, String>> {
    THOUGHT_SIGNATURE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn store_thought_signature(tool_id: &str, tool_name: &str, signature: &str) {
    if signature.is_empty() {
        return;
    }
    if let Ok(mut cache) = get_cache().lock() {
        if cache.len() > 1000 {
            cache.clear();
        }
        if !tool_id.is_empty() {
            cache.insert(tool_id.to_string(), signature.to_string());
        }
        if !tool_name.is_empty() {
            cache.insert(tool_name.to_string(), signature.to_string());
        }
    }
}

pub fn get_thought_signature(tool_id: &str, tool_name: &str) -> Option<String> {
    if let Ok(cache) = get_cache().lock() {
        if !tool_id.is_empty() {
            if let Some(sig) = cache.get(tool_id) {
                return Some(sig.clone());
            }
        }
        if !tool_name.is_empty() {
            if let Some(sig) = cache.get(tool_name) {
                return Some(sig.clone());
            }
        }
    }
    None
}

/// Google Gemini / Cloud Code PA Protobuf Schema 严格支持的字段白名单。
/// 任何不在白名单中的现代 JSON-Schema 关键字（如 propertyNames, exclusiveMinimum, $schema, additionalProperties 等）
/// 都会被 Google 的 Protobuf 反序列化器直接抛出 400 错误。
const ALLOWED_SCHEMA_KEYS: &[&str] = &[
    "type",
    "format",
    "description",
    "nullable",
    "enum",
    "properties",
    "required",
    "items",
    "minItems",
    "maxItems",
    "min_items",
    "max_items",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "min_length",
    "max_length",
    "minProperties",
    "maxProperties",
    "min_properties",
    "max_properties",
    "pattern",
    "example",
    "anyOf",
    "any_of",
    "title",
    "default",
];

/// 递归清理并规范化为 Google Gemini API / Protobuf Schema 严格兼容格式。
pub fn sanitize_gemini_schema(value: &mut Value) {
    match value {
        Value::Object(map) => {
            // 1. 递归处理子结构（properties, items, anyOf, any_of, oneOf, allOf）
            if let Some(Value::Object(props)) = map.get_mut("properties") {
                props.retain(|_, v| !v.is_null());
                for (_k, prop_val) in props.iter_mut() {
                    sanitize_gemini_schema(prop_val);
                }
            }

            if let Some(items_val) = map.get_mut("items") {
                sanitize_gemini_schema(items_val);
            }

            for key in ["anyOf", "any_of", "oneOf", "allOf"] {
                if let Some(Value::Array(arr)) = map.get_mut(key) {
                    for item in arr.iter_mut() {
                        sanitize_gemini_schema(item);
                    }
                }
            }

            // 2. 语义转换：Google Protobuf Schema 不支持 oneOf/allOf，转换为 anyOf
            if let Some(one_of) = map.remove("oneOf") {
                if !map.contains_key("anyOf") {
                    map.insert("anyOf".to_string(), one_of);
                }
            }
            if let Some(all_of) = map.remove("allOf") {
                if !map.contains_key("anyOf") {
                    map.insert("anyOf".to_string(), all_of);
                }
            }

            // 3. 语义转换：exclusiveMinimum -> minimum（避免 Google 不识别 exclusiveMinimum 报错）
            if let Some(ex_min) = map.remove("exclusiveMinimum") {
                if !map.contains_key("minimum") {
                    map.insert("minimum".to_string(), ex_min);
                }
            }

            // 4. 语义转换：exclusiveMaximum -> maximum
            if let Some(ex_max) = map.remove("exclusiveMaximum") {
                if !map.contains_key("maximum") {
                    map.insert("maximum".to_string(), ex_max);
                }
            }

            // 5. 语义转换：const -> enum: [const_val]
            if let Some(c_val) = map.remove("const") {
                if !map.contains_key("enum") {
                    map.insert("enum".to_string(), Value::Array(vec![c_val]));
                }
            }

            // 6. 规范化 enum：Google Schema enum 是 repeated string，确保项为字符串
            if let Some(Value::Array(enum_arr)) = map.get_mut("enum") {
                for val in enum_arr.iter_mut() {
                    if !val.is_string() {
                        let str_repr = match &*val {
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            other => other.to_string(),
                        };
                        *val = Value::String(str_repr);
                    }
                }
            }

            // 7. 处理 type 数组，如 ["string", "null"] -> "string" + nullable: true
            if let Some(t_val) = map.get("type").cloned() {
                if let Value::Array(arr) = t_val {
                    let has_null = arr.iter().any(|v| v.as_str() == Some("null"));
                    if has_null {
                        map.insert("nullable".to_string(), Value::Bool(true));
                    }
                    if let Some(first_non_null) = arr.iter().find(|v| v.as_str() != Some("null")) {
                        map.insert("type".to_string(), first_non_null.clone());
                    } else {
                        map.insert("type".to_string(), Value::String("string".to_string()));
                    }
                }
            }

            // 8. 规范化 float/double 为 number
            if let Some(type_str) = map.get("type").and_then(|v| v.as_str()) {
                if type_str == "float" || type_str == "double" {
                    map.insert("type".to_string(), Value::String("number".to_string()));
                }
            }

            // 9. 如果缺失 type，根据结构特征推断默认 type
            if !map.contains_key("type") {
                if map.contains_key("properties") {
                    map.insert("type".to_string(), Value::String("object".to_string()));
                } else if map.contains_key("items") {
                    map.insert("type".to_string(), Value::String("array".to_string()));
                }
            }

            // 10. 规范化 required：必须为纯字符串数组
            if let Some(req_val) = map.get_mut("required") {
                if let Value::Array(arr) = req_val {
                    arr.retain(|item| item.is_string());
                } else {
                    map.remove("required");
                }
            }

            // 11. 移除所有值为 null 的字段，避免 Protobuf 反序列化时报错
            map.retain(|_, v| !v.is_null());

            // 12. 白名单严格过滤：彻底移除所有不支持的字段（如 propertyNames, $schema, additionalProperties, $defs 等）
            map.retain(|k, _| ALLOWED_SCHEMA_KEYS.contains(&k.as_str()));
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                sanitize_gemini_schema(item);
            }
        }
        _ => {}
    }
}

pub fn convert_anthropic_req_to_gemini(req: &AnthropicMessageRequest) -> GeminiRequest {
    // 预先建立 tool_use_id -> tool_name 映射，确保 tool_result 发送给 Gemini 时匹配真实函数名
    let mut tool_id_to_name: HashMap<String, String> = HashMap::new();
    for msg in &req.messages {
        if let Value::Array(blocks) = &msg.content {
            for block in blocks {
                if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                    if let (Some(id), Some(name)) = (
                        block.get("id").and_then(|v| v.as_str()),
                        block.get("name").and_then(|v| v.as_str()),
                    ) {
                        tool_id_to_name.insert(id.to_string(), name.to_string());
                    }
                }
            }
        }
    }

    let mut contents = Vec::new();

    for msg in &req.messages {
        let role = if msg.role == "assistant" {
            "model"
        } else {
            "user"
        };
        let mut parts = Vec::new();

        match &msg.content {
            Value::String(s) => {
                parts.push(GeminiPart {
                    text: Some(s.clone()),
                    function_call: None,
                    function_response: None,
                    thought_signature: None,
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
                                    thought_signature: None,
                                });
                            }
                        }
                        "tool_use" => {
                            let name = block
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let args = block.get("input").cloned().unwrap_or(json!({}));
                            let tool_id = block
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let final_id = if tool_id.is_empty() {
                                format!("call_{}", uuid::Uuid::new_v4().simple())
                            } else {
                                tool_id.to_string()
                            };

                            // 优先从 block 中读取，其次从本地缓存按 ID 或名称查找，若无则使用 Google 官方 bypass 哨兵值
                            let sig = block
                                .get("thought_signature")
                                .or_else(|| block.get("thoughtSignature"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .or_else(|| get_thought_signature(tool_id, &name))
                                .unwrap_or_else(|| "skip_thought_signature_validator".to_string());

                            parts.push(GeminiPart {
                                text: None,
                                function_call: Some(json!({
                                    "name": name,
                                    "args": args,
                                    "id": final_id
                                })),
                                function_response: None,
                                thought_signature: Some(sig),
                            });
                        }
                        "tool_result" => {
                            let content_val = block.get("content");
                            let res_text = match content_val {
                                Some(Value::String(s)) => s.clone(),
                                Some(other) => other.to_string(),
                                None => "".to_string(),
                            };
                            let tool_id = block.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("");
                            let tool_name = tool_id_to_name
                                .get(tool_id)
                                .map(|s| s.as_str())
                                .or_else(|| block.get("name").and_then(|v| v.as_str()))
                                .unwrap_or(if tool_id.is_empty() { "tool" } else { tool_id });

                            let mut fr_val = json!({
                                "name": tool_name,
                                "response": { "result": res_text }
                            });
                            if !tool_id.is_empty() {
                                fr_val["id"] = json!(tool_id);
                            }

                            parts.push(GeminiPart {
                                text: None,
                                function_call: None,
                                function_response: Some(fr_val),
                                thought_signature: None,
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
                thought_signature: None,
            }],
        }
    });

    let tools = req.tools.as_ref().and_then(|tools| {
        if tools.is_empty() {
            return None;
        }
        let declarations: Vec<Value> = tools
            .iter()
            .map(|t| {
                let mut params = t.input_schema.clone();
                sanitize_gemini_schema(&mut params);
                if let Value::Object(ref mut map) = params {
                    if !map.contains_key("type") {
                        map.insert("type".to_string(), json!("object"));
                    }
                }
                let mut decl = json!({
                    "name": t.name,
                    "parameters": params,
                });
                if let Some(ref desc) = t.description {
                    if !desc.trim().is_empty() {
                        decl["description"] = json!(desc);
                    }
                }
                decl
            })
            .collect();

        if declarations.is_empty() {
            None
        } else {
            let decls = if declarations.len() > 128 {
                declarations.into_iter().take(128).collect()
            } else {
                declarations
            };
            Some(vec![json!({ "functionDeclarations": decls })])
        }
    });

    let mut gen_config = json!({});
    if let Some(t) = req.temperature {
        gen_config["temperature"] = json!(t);
    }
    if let Some(mt) = req.max_tokens {
        gen_config["maxOutputTokens"] = json!(mt);
    }
    if let Some(ref th) = req.thinking {
        if th.thinking_type == "enabled" {
            let budget = th.budget_tokens.unwrap_or(2048);
            gen_config["thinkingConfig"] = json!({ "thinkingBudget": budget });
            if let Some(mt) = req.max_tokens {
                gen_config["maxOutputTokens"] = json!(mt.max(budget + 2048));
            }
        }
    }

    GeminiRequest {
        contents,
        system_instruction,
        tools,
        generation_config: Some(gen_config),
    }
}

pub fn convert_gemini_resp_to_anthropic(
    resp: &GeminiResponse,
    req_model: &str,
) -> AnthropicResponse {
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
                        let name = fc
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let args = fc.get("args").cloned().unwrap_or(json!({}));
                        let call_id = format!("call_{}", uuid::Uuid::new_v4());

                        let sig = part
                            .thought_signature
                            .as_deref()
                            .or_else(|| fc.get("thoughtSignature").and_then(|v| v.as_str()))
                            .or_else(|| fc.get("thought_signature").and_then(|v| v.as_str()));
                        if let Some(s) = sig {
                            store_thought_signature(&call_id, &name, s);
                        }

                        content_blocks.push(json!({
                            "type": "tool_use",
                            "id": call_id,
                            "name": name,
                            "input": args
                        }));
                        stop_reason = Some("tool_use".to_string());
                    }
                }
            }
        }
    }

    let usage = resp
        .usage_metadata
        .as_ref()
        .map(|u| AnthropicUsage {
            input_tokens: u.prompt_token_count.unwrap_or(0),
            output_tokens: u.candidates_token_count.unwrap_or(0),
        })
        .unwrap_or(AnthropicUsage {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_gemini_schema_strips_schema_and_defs() {
        let mut schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "$id": "https://example.com/tool.schema.json",
            "type": "object",
            "properties": {
                "command": {
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "string",
                    "description": "shell command"
                },
                "optional_flag": {
                    "type": ["string", "null"],
                    "description": "nullable string"
                }
            },
            "$defs": {
                "Something": { "type": "string" }
            }
        });

        sanitize_gemini_schema(&mut schema);

        assert!(schema.get("$schema").is_none());
        assert!(schema.get("$id").is_none());
        assert!(schema.get("$defs").is_none());

        let cmd = schema["properties"]["command"].as_object().unwrap();
        assert!(cmd.get("$schema").is_none());
        assert_eq!(cmd["type"], "string");

        let flag = schema["properties"]["optional_flag"].as_object().unwrap();
        assert_eq!(flag["type"], "string");
        assert_eq!(flag["nullable"], true);
    }

    #[test]
    fn test_sanitize_gemini_schema_claude_property_names_and_exclusive_minimum() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "options": {
                    "type": "object",
                    "propertyNames": { "pattern": "^[a-zA-Z0-9_]+$" },
                    "additionalProperties": false,
                    "properties": {
                        "count": {
                            "type": "integer",
                            "exclusiveMinimum": 1,
                            "exclusiveMaximum": 100
                        }
                    }
                },
                "status": {
                    "const": "active"
                },
                "code": {
                    "type": "integer",
                    "enum": [200, 404, 500]
                },
                "union": {
                    "oneOf": [
                        { "type": "string" },
                        { "type": "number" }
                    ]
                },
                "null_field": null
            },
            "required": ["options", "status"]
        });

        sanitize_gemini_schema(&mut schema);

        // 验证 null_field 被移除
        assert!(schema["properties"].get("null_field").is_none());

        // 验证 options 内部 propertyNames 和 additionalProperties 被剔除
        let options = schema["properties"]["options"].as_object().unwrap();
        assert!(options.get("propertyNames").is_none());
        assert!(options.get("additionalProperties").is_none());

        // 验证 count 的 exclusiveMinimum -> minimum, exclusiveMaximum -> maximum
        let count = options["properties"]["count"].as_object().unwrap();
        assert!(count.get("exclusiveMinimum").is_none());
        assert_eq!(count["minimum"], 1);
        assert!(count.get("exclusiveMaximum").is_none());
        assert_eq!(count["maximum"], 100);

        // 验证 const -> enum
        let status = schema["properties"]["status"].as_object().unwrap();
        assert!(status.get("const").is_none());
        assert_eq!(status["enum"], json!(["active"]));

        // 验证 number enum 转换为 string
        let code = schema["properties"]["code"].as_object().unwrap();
        assert_eq!(code["enum"], json!(["200", "404", "500"]));

        // 验证 oneOf -> anyOf
        let union_field = schema["properties"]["union"].as_object().unwrap();
        assert!(union_field.get("oneOf").is_none());
        assert!(union_field.get("anyOf").is_some());
    }

    #[test]
    fn test_convert_anthropic_req_empty_tools_and_none_description() {
        let req_empty_tools = AnthropicMessageRequest {
            model: "claude-sonnet-4-6".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: json!("hello"),
            }],
            system: None,
            max_tokens: Some(1024),
            temperature: None,
            stream: None,
            metadata: None,
            thinking: None,
            tool_choice: None,
            tools: Some(vec![]),
        };

        let gemini_req = convert_anthropic_req_to_gemini(&req_empty_tools);
        // 空 tools 不应生成 tools: [ { functionDeclarations: [] } ]
        assert!(gemini_req.tools.is_none());

        let req_tool_no_desc = AnthropicMessageRequest {
            model: "claude-sonnet-4-6".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: json!("hello"),
            }],
            system: None,
            max_tokens: Some(1024),
            temperature: None,
            stream: None,
            metadata: None,
            thinking: None,
            tool_choice: None,
            tools: Some(vec![AnthropicTool {
                name: "no_desc_tool".to_string(),
                description: None,
                input_schema: json!({
                    "properties": {
                        "foo": { "type": "string" }
                    }
                }),
            }]),
        };

        let gemini_req2 = convert_anthropic_req_to_gemini(&req_tool_no_desc);
        let decl = &gemini_req2.tools.as_ref().unwrap()[0]["functionDeclarations"][0];
        // description 为 None 时不应生成 "description": null
        assert!(decl.get("description").is_none());
        // 自动补充 parameters 的 type 为 object
        assert_eq!(decl["parameters"]["type"], "object");
    }

    #[test]
    fn test_convert_anthropic_req_with_claude_tools_and_results() {
        let req = AnthropicMessageRequest {
            model: "claude-sonnet-4-6".to_string(),
            messages: vec![
                AnthropicMessage {
                    role: "assistant".to_string(),
                    content: json!([
                        {
                            "type": "tool_use",
                            "id": "toolu_01_bash_call",
                            "name": "Bash",
                            "input": { "command": "ls -la" }
                        }
                    ]),
                },
                AnthropicMessage {
                    role: "user".to_string(),
                    content: json!([
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_01_bash_call",
                            "content": "file1.txt\nfile2.txt"
                        }
                    ]),
                },
            ],
            system: None,
            max_tokens: Some(1024),
            temperature: Some(0.7),
            stream: Some(false),
            metadata: None,
            thinking: None,
            tool_choice: None,
            tools: Some(vec![AnthropicTool {
                name: "Bash".to_string(),
                description: Some("Run shell commands".to_string()),
                input_schema: json!({
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" }
                    },
                    "required": ["command"]
                }),
            }]),
        };

        let gemini_req = convert_anthropic_req_to_gemini(&req);

        // 验证 tools 中的 function_declarations 参数中不再有 $schema
        let tools = gemini_req.tools.expect("tools should be present");
        let func_decl = &tools[0]["functionDeclarations"][0];
        assert_eq!(func_decl["name"], "Bash");
        assert!(func_decl["parameters"].get("$schema").is_none());

        // 验证 tool_result 正确将 name 映射为函数名 Bash，而不是 toolu_01_bash_call，且保留 id
        let last_content = gemini_req.contents.last().expect("should have user content");
        let func_resp = last_content.parts[0]
            .function_response
            .as_ref()
            .expect("should have function_response");
        assert_eq!(func_resp["name"], "Bash");
        assert_eq!(func_resp["id"], "toolu_01_bash_call");

        // 验证 model 消息中的 functionCall part 带有 thought_signature 与 id
        let model_content = &gemini_req.contents[0];
        assert_eq!(model_content.role, "model");
        let model_fc = model_content.parts[0]
            .function_call
            .as_ref()
            .expect("should have function_call");
        assert_eq!(model_fc["id"], "toolu_01_bash_call");
        assert_eq!(
            model_content.parts[0].thought_signature.as_deref(),
            Some("skip_thought_signature_validator")
        );
    }

    #[test]
    fn test_thought_signature_cache_and_restoration() {
        // 存储一个真实的签名
        store_thought_signature("tool_call_xyz", "default_api:Bash", "REAL_ENCRYPTED_SIGNATURE_123");

        let req = AnthropicMessageRequest {
            model: "claude-sonnet-4-6".to_string(),
            messages: vec![AnthropicMessage {
                role: "assistant".to_string(),
                content: json!([
                    {
                        "type": "tool_use",
                        "id": "tool_call_xyz",
                        "name": "default_api:Bash",
                        "input": { "command": "pwd" }
                    }
                ]),
            }],
            system: None,
            max_tokens: Some(1024),
            temperature: None,
            stream: None,
            metadata: None,
            thinking: None,
            tool_choice: None,
            tools: None,
        };

        let gemini_req = convert_anthropic_req_to_gemini(&req);
        let part = &gemini_req.contents[0].parts[0];
        assert_eq!(
            part.thought_signature.as_deref(),
            Some("REAL_ENCRYPTED_SIGNATURE_123")
        );
    }

    #[test]
    fn test_gemini_thinking_config_mapping() {
        let req = AnthropicMessageRequest {
            model: "gemini-2.5-flash".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: json!("Hello"),
            }],
            system: None,
            max_tokens: Some(4096),
            temperature: None,
            stream: None,
            metadata: None,
            thinking: Some(crate::gateway::types::AnthropicThinkingConfig {
                thinking_type: "enabled".to_string(),
                budget_tokens: Some(4000),
            }),
            tool_choice: None,
            tools: None,
        };

        let gemini_req = convert_anthropic_req_to_gemini(&req);
        let gen_config = gemini_req.generation_config.expect("should have generation_config");
        let thinking_config = gen_config.get("thinkingConfig").expect("should have thinkingConfig");
        assert_eq!(thinking_config["thinkingBudget"], 4000);
        assert_eq!(gen_config["maxOutputTokens"], 6048);
    }
}
