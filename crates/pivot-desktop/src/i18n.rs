use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[serde(rename = "zh-CN")]
    #[default]
    ZhCn,
    #[serde(rename = "en")]
    En,
}

impl Locale {
    #[inline]
    pub fn pick<'a>(self, zh: &'a str, en: &'a str) -> &'a str {
        match self {
            Self::ZhCn => zh,
            Self::En => en,
        }
    }

    pub fn protocol_label(self, protocol: &str) -> &'static str {
        match (self, protocol) {
            (Self::ZhCn, "openai") => "OpenAI 兼容",
            (Self::En, "openai") => "OpenAI Compatible",
            (_, "anthropic") => "Anthropic",
            (_, "gemini") => "Google Gemini",
            (_, "openrouter") => "OpenRouter",
            (_, "ollama") => "Ollama",
            (_, "chatgpt") => "ChatGPT",
            _ => "OpenAI",
        }
    }

    pub fn client_type_label(self, client_type: &str) -> String {
        match (self, client_type) {
            (_, "claude_code") => "Claude Code".to_string(),
            (_, "codex_cli") => "Codex CLI".to_string(),
            (_, "gemini_cli") => "Gemini CLI".to_string(),
            (_, "pi") => "Pi Agent".to_string(),
            (_, "opencode") => "OpenCode".to_string(),
            (_, "openai_api") => "OpenAI API".to_string(),
            (_, "anthropic_api") => "Anthropic API".to_string(),
            (Self::ZhCn, "generic" | "") => "通用客户端".to_string(),
            (Self::En, "generic" | "") => "Universal Agent".to_string(),
            (_, other) => other.to_string(),
        }
    }
}

#[inline]
pub fn protocol_label(protocol: &str, locale: Locale) -> &'static str {
    locale.protocol_label(protocol)
}

#[inline]
pub fn client_type_label(client_type: &str, locale: Locale) -> String {
    locale.client_type_label(client_type)
}
