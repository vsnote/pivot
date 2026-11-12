use crate::components::button::NativeButton;
use crate::components::icons::{lucide_icon, lucide_spinner, render_brand_avatar};
use crate::components::text_input::NativeTextInput;
use crate::i18n::{protocol_label, Locale};
use crate::theme::ThemePalette;
use crate::views::providers::list::is_subscription_provider;
use crate::views::providers::model_pool::render_model_pool_section;
use crate::views::providers::subscription::render_subscription_section;
use crate::{ActiveTab, PivotApp};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pivot_core::db::presets::Preset;
use pivot_core::db::{Provider, SubscriptionAccount};
use pivot_core::oauth::AccountQuota;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFilterTab {
    All,
    Selected,
    Unselected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbandonAction {
    Leave,
    BackToPresets,
    SwitchTab(ActiveTab),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorBaseline {
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub api_keys: Vec<String>,
    pub models: Vec<String>,
}

pub struct ProviderEditorState {
    pub is_new: bool,
    pub choosing_preset: bool,
    pub editing_provider: Option<Provider>,
    pub selected_preset: Option<Preset>,
    pub provider_id: String,
    pub preset_id: Option<String>,
    pub auth_type: Option<String>,
    pub protocol: String,
    pub protocol_picker_open: bool,
    pub show_key_hint: bool,
    pub show_keys: bool,

    pub preset_search_input: Entity<InputState>,
    pub name_input: Entity<InputState>,
    pub base_url_input: Entity<InputState>,
    pub key_inputs: Vec<Entity<InputState>>,
    pub model_search_input: Entity<InputState>,
    pub custom_model_input: Entity<InputState>,

    pub baseline: EditorBaseline,
    pub pending_abandon: Option<AbandonAction>,

    pub testing_endpoint: bool,
    pub endpoint_latency: Option<i64>,
    pub endpoint_error: Option<String>,

    pub model_pool: Vec<String>,
    pub selected_models: Vec<String>,
    pub upstream_models: Option<Vec<String>>,
    pub model_filter: ModelFilterTab,
    pub pool_scroll_handle: ScrollHandle,
    pub fetching_models: bool,
    pub hovered_pool_model: Option<String>,

    pub sub_accounts: Vec<SubscriptionAccount>,
    pub sub_quotas: HashMap<String, AccountQuota>,
    pub loading_quotas: HashMap<String, bool>,
    pub expanded_quota_account_id: Option<String>,
    pub connecting_oauth: bool,
    pub importing_codex: bool,
    pub consuming_credit_id: Option<String>,
    pub auto_saved: bool,

    pub error_msg: Option<String>,
    pub saving: bool,
}

pub fn clean_preset_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(idx) = trimmed.find(" (").or_else(|| trimmed.find('（')) {
        let prefix = trimmed[..idx].trim();
        if !prefix.is_empty() {
            return prefix.to_string();
        }
    }
    trimmed.to_string()
}

pub fn is_subscription_preset(preset: &Preset) -> bool {
    preset.auth_type.as_deref() == Some("subscription_pool")
        || preset.id == "antigravity"
        || preset.id == "chatgpt_subscription"
        || preset.protocol == "chatgpt"
}

fn default_base_url_for_protocol(protocol: &str) -> &'static str {
    match protocol {
        "anthropic" => "https://api.anthropic.com/v1",
        "gemini" => "https://generativelanguage.googleapis.com/v1beta",
        "openrouter" => "https://openrouter.ai/api/v1",
        "ollama" => "http://127.0.0.1:11434/v1",
        "chatgpt" => "https://chatgpt.com/backend-api/codex",
        _ => "https://api.openai.com/v1",
    }
}

fn normalize_url_cmp(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn normalize_models_cmp(models: &[String]) -> Vec<String> {
    let mut set: Vec<String> = models
        .iter()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .collect();
    set.sort();
    set.dedup();
    set
}

fn merge_unique_models(primary: &[String], extra: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for m in primary.iter().chain(extra.iter()) {
        let t = m.trim();
        if !t.is_empty() && seen.insert(t.to_string()) {
            out.push(t.to_string());
        }
    }
    out
}

pub fn validate_base_url_hint(url: &str, locale: Locale) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Some(
            locale
                .pick(
                    "接口地址必须以 http:// 或 https:// 开头",
                    "Base URL must start with http:// or https://",
                )
                .to_string(),
        );
    }
    if trimmed.contains("YOUR_ACCOUNT_ID") {
        return Some(
            locale
                .pick(
                    "请将地址中的 YOUR_ACCOUNT_ID 替换为真实账户 ID",
                    "Replace YOUR_ACCOUNT_ID with your actual account ID",
                )
                .to_string(),
        );
    }
    let no_slash = trimmed.trim_end_matches('/');
    if no_slash.ends_with("/chat/completions")
        || no_slash.ends_with("/messages")
        || no_slash.ends_with("/models")
    {
        return Some(
            locale
                .pick(
                    "请填写 API 根路径（如 .../v1），无需包含 /chat/completions 或 /messages",
                    "Enter root Base URL (e.g. .../v1) without /chat/completions or /messages",
                )
                .to_string(),
        );
    }
    None
}

impl ProviderEditorState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        editing: Option<Provider>,
        presets: &[Preset],
        saved_pools: &HashMap<String, Vec<String>>,
        sub_accounts: Vec<SubscriptionAccount>,
        locale: Locale,
        window: &mut Window,
        cx: &mut Context<PivotApp>,
    ) -> Self {
        let preset_search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(locale.pick("搜索内置服务商预设…", "Search built-in presets…"))
        });
        cx.subscribe_in(
            &preset_search_input,
            window,
            |_this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        )
        .detach();

        let model_search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(locale.pick("筛选模型名称或 ID…", "Filter models…"))
        });
        cx.subscribe_in(
            &model_search_input,
            window,
            |_this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        )
        .detach();

        let custom_model_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(locale.pick(
                "输入自定义模型 ID（回车添加，支持逗号分隔）",
                "Enter custom model ID (Enter to add)",
            ))
        });
        cx.subscribe_in(
            &custom_model_input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    this.editor_add_custom_model(window, cx);
                }
                InputEvent::Change => {
                    cx.notify();
                }
                _ => {}
            },
        )
        .detach();

        if let Some(p) = editing {
            let matched_preset = p
                .preset_id
                .as_deref()
                .and_then(|pid| presets.iter().find(|pr| pr.id == pid).cloned());

            let name_val = p.name.clone();
            let base_url_val = p.base_url.clone();

            let name_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(locale.pick("例如 OpenAI / DeepSeek", "e.g. OpenAI / DeepSeek"))
                    .default_value(name_val.clone())
            });
            let base_url_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("https://api.openai.com/v1")
                    .default_value(base_url_val.clone())
            });

            cx.subscribe_in(&name_input, window, |_this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            cx.subscribe_in(
                &base_url_input,
                window,
                |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        if let Some(ed) = this.editor.as_mut() {
                            ed.endpoint_latency = None;
                            ed.endpoint_error = None;
                        }
                        cx.notify();
                    }
                },
            )
            .detach();

            let raw_keys: Vec<String> = if p.api_keys.is_empty() {
                vec![String::new()]
            } else {
                p.api_keys.clone()
            };
            let mut key_inputs = Vec::with_capacity(raw_keys.len());
            for k in &raw_keys {
                let val = k.clone();
                let ki = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("sk-...")
                        .default_value(val)
                        .masked(true)
                });
                cx.subscribe_in(&ki, window, |_this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
                .detach();
                key_inputs.push(ki);
            }

            let saved_pool = saved_pools.get(&p.id).cloned().unwrap_or_default();
            let preset_defaults = matched_preset
                .as_ref()
                .map(|pr| pr.default_models.clone())
                .unwrap_or_default();
            let merged_pool = merge_unique_models(
                &p.models,
                &merge_unique_models(&saved_pool, &preset_defaults),
            );
            let selected_models = p.models.clone();

            let clean_keys: Vec<String> = p
                .api_keys
                .iter()
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty())
                .collect();

            let baseline = EditorBaseline {
                name: p.name.trim().to_string(),
                protocol: p.protocol.clone(),
                base_url: normalize_url_cmp(&p.base_url),
                api_keys: clean_keys,
                models: normalize_models_cmp(&selected_models),
            };

            Self {
                is_new: false,
                choosing_preset: false,
                selected_preset: matched_preset,
                provider_id: p.id.clone(),
                preset_id: p.preset_id.clone(),
                auth_type: p.auth_type.clone(),
                protocol: p.protocol.clone(),
                editing_provider: Some(p),
                protocol_picker_open: false,
                show_key_hint: false,
                show_keys: false,
                preset_search_input,
                name_input,
                base_url_input,
                key_inputs,
                model_search_input,
                custom_model_input,
                baseline,
                pending_abandon: None,
                testing_endpoint: false,
                endpoint_latency: None,
                endpoint_error: None,
                model_pool: merged_pool,
                selected_models,
                upstream_models: None,
                model_filter: ModelFilterTab::All,
                pool_scroll_handle: ScrollHandle::new(),
                fetching_models: false,
                hovered_pool_model: None,
                sub_accounts,
                sub_quotas: HashMap::new(),
                loading_quotas: HashMap::new(),
                expanded_quota_account_id: None,
                connecting_oauth: false,
                importing_codex: false,
                consuming_credit_id: None,
                auto_saved: false,
                error_msg: None,
                saving: false,
            }
        } else {
            let name_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(locale.pick("例如 OpenAI / DeepSeek", "e.g. OpenAI / DeepSeek"))
            });
            let base_url_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("https://api.openai.com/v1"));
            let first_key = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("sk-...")
                    .masked(true)
            });

            cx.subscribe_in(&name_input, window, |_this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            cx.subscribe_in(
                &base_url_input,
                window,
                |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        if let Some(ed) = this.editor.as_mut() {
                            ed.endpoint_latency = None;
                            ed.endpoint_error = None;
                        }
                        cx.notify();
                    }
                },
            )
            .detach();
            cx.subscribe_in(&first_key, window, |_this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();

            let baseline = EditorBaseline {
                name: String::new(),
                protocol: "openai".to_string(),
                base_url: String::new(),
                api_keys: Vec::new(),
                models: Vec::new(),
            };

            Self {
                is_new: true,
                choosing_preset: true,
                editing_provider: None,
                selected_preset: None,
                provider_id: uuid::Uuid::new_v4().to_string(),
                preset_id: None,
                auth_type: None,
                protocol: "openai".to_string(),
                protocol_picker_open: false,
                show_key_hint: false,
                show_keys: false,
                preset_search_input,
                name_input,
                base_url_input,
                key_inputs: vec![first_key],
                model_search_input,
                custom_model_input,
                baseline,
                pending_abandon: None,
                testing_endpoint: false,
                endpoint_latency: None,
                endpoint_error: None,
                model_pool: Vec::new(),
                selected_models: Vec::new(),
                upstream_models: None,
                model_filter: ModelFilterTab::All,
                pool_scroll_handle: ScrollHandle::new(),
                fetching_models: false,
                hovered_pool_model: None,
                sub_accounts: Vec::new(),
                sub_quotas: HashMap::new(),
                loading_quotas: HashMap::new(),
                expanded_quota_account_id: None,
                connecting_oauth: false,
                importing_codex: false,
                consuming_credit_id: None,
                auto_saved: false,
                error_msg: None,
                saving: false,
            }
        }
    }

    pub fn apply_preset(
        &mut self,
        preset: Option<Preset>,
        saved_pools: &HashMap<String, Vec<String>>,
        window: &mut Window,
        cx: &mut Context<PivotApp>,
    ) {
        self.choosing_preset = false;
        self.error_msg = None;
        self.endpoint_latency = None;
        self.endpoint_error = None;
        self.protocol_picker_open = false;

        if let Some(pr) = preset {
            let clean_name = clean_preset_name(&pr.name);
            self.name_input.update(cx, |s, cx| {
                s.set_value(clean_name.clone(), window, cx);
            });
            self.base_url_input.update(cx, |s, cx| {
                s.set_value(pr.base_url.clone(), window, cx);
            });
            self.protocol = pr.protocol.clone();
            self.preset_id = Some(pr.id.clone());
            self.auth_type = if is_subscription_preset(&pr) {
                Some("subscription_pool".to_string())
            } else {
                pr.auth_type.clone()
            };

            let cached = saved_pools.get(&self.provider_id).cloned().unwrap_or_default();
            let pool = merge_unique_models(&pr.default_models, &cached);
            let selected = if is_subscription_preset(&pr) {
                Vec::new()
            } else {
                pr.default_models.clone()
            };

            self.model_pool = pool;
            self.selected_models = selected.clone();
            self.selected_preset = Some(pr.clone());

            self.baseline = EditorBaseline {
                name: clean_name.trim().to_string(),
                protocol: pr.protocol,
                base_url: normalize_url_cmp(&pr.base_url),
                api_keys: Vec::new(),
                models: normalize_models_cmp(&selected),
            };
        } else {
            self.selected_preset = None;
            self.preset_id = None;
            self.auth_type = None;
            self.protocol = "openai".to_string();
            let default_url = default_base_url_for_protocol("openai").to_string();
            self.name_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            self.base_url_input
                .update(cx, |s, cx| s.set_value(default_url.clone(), window, cx));
            self.model_pool.clear();
            self.selected_models.clear();

            self.baseline = EditorBaseline {
                name: String::new(),
                protocol: "openai".to_string(),
                base_url: normalize_url_cmp(&default_url),
                api_keys: Vec::new(),
                models: Vec::new(),
            };
        }
    }

    pub fn is_subscription_mode(&self) -> bool {
        self.auth_type.as_deref() == Some("subscription_pool")
            || self.preset_id.as_deref() == Some("antigravity")
            || self.preset_id.as_deref() == Some("chatgpt_subscription")
            || self.protocol == "chatgpt"
    }

    pub fn can_edit_base_url(&self) -> bool {
        let Some(ref pr) = self.selected_preset else {
            return true;
        };
        pr.category.contains("本地")
            || pr.base_url.contains("YOUR_ACCOUNT_ID")
            || pr.id == "opencode"
    }

    pub fn collect_api_keys(&self, cx: &App) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for ki in &self.key_inputs {
            let v = ki.read(cx).value().trim().to_string();
            if !v.is_empty() && seen.insert(v.clone()) {
                out.push(v);
            }
        }
        out
    }

    pub fn has_unsaved_changes(&self, cx: &App) -> bool {
        if self.choosing_preset {
            return false;
        }
        if !self.custom_model_input.read(cx).value().trim().is_empty() {
            return true;
        }
        let cur = EditorBaseline {
            name: self.name_input.read(cx).value().trim().to_string(),
            protocol: self.protocol.clone(),
            base_url: normalize_url_cmp(&self.base_url_input.read(cx).value()),
            api_keys: self.collect_api_keys(cx),
            models: normalize_models_cmp(&self.selected_models),
        };
        cur != self.baseline
    }

    pub fn sync_baseline(&mut self, cx: &App) {
        self.baseline = EditorBaseline {
            name: self.name_input.read(cx).value().trim().to_string(),
            protocol: self.protocol.clone(),
            base_url: normalize_url_cmp(&self.base_url_input.read(cx).value()),
            api_keys: self.collect_api_keys(cx),
            models: normalize_models_cmp(&self.selected_models),
        };
    }

    pub fn build_provider_payload(&self, locale: Locale, cx: &App) -> Result<Provider, String> {
        let is_sub = self.is_subscription_mode();
        let raw_name = self.name_input.read(cx).value().trim().to_string();
        let name = if raw_name.is_empty() && is_sub {
            if self.preset_id.as_deref() == Some("chatgpt_subscription")
                || self.protocol == "chatgpt"
            {
                "ChatGPT".to_string()
            } else {
                "Antigravity".to_string()
            }
        } else {
            raw_name
        };

        if name.is_empty() {
            return Err(locale
                .pick("请填写服务商名称", "Please enter a provider name")
                .to_string());
        }

        let base_url = self.base_url_input.read(cx).value().trim().to_string();
        if !is_sub {
            if base_url.is_empty() {
                return Err(locale
                    .pick("请填写接口地址 (Base URL)", "Please enter Base URL")
                    .to_string());
            }
            if let Some(hint_err) = validate_base_url_hint(&base_url, locale) {
                return Err(hint_err);
            }
        }

        let api_keys = if is_sub {
            Vec::new()
        } else {
            self.collect_api_keys(cx)
        };

        let is_local = self.protocol == "ollama"
            || self
                .selected_preset
                .as_ref()
                .map(|p| p.category.contains("本地"))
                .unwrap_or(false)
            || base_url.contains("127.0.0.1")
            || base_url.contains("localhost");

        if !is_sub && !is_local && api_keys.is_empty() {
            return Err(locale
                .pick("请至少填写一个 API 密钥", "Please enter at least one API key")
                .to_string());
        }

        if !self.custom_model_input.read(cx).value().trim().is_empty() {
            return Err(locale
                .pick(
                    "自定义模型输入框有未提交内容，请先点击「添加」或清空后再保存",
                    "Pending custom model input — click 'Add' or clear it before saving",
                )
                .to_string());
        }

        if is_sub && self.sub_accounts.is_empty() {
            return Err(locale
                .pick(
                    "请先授权连接至少一个订阅账号",
                    "Connect at least one subscription account first",
                )
                .to_string());
        }

        if self.selected_models.is_empty() {
            return Err(locale
                .pick(
                    "请至少勾选一个可用模型",
                    "Select at least one available model",
                )
                .to_string());
        }

        let prev = self.editing_provider.as_ref();
        Ok(Provider {
            id: self.provider_id.clone(),
            name,
            protocol: self.protocol.clone(),
            base_url: if base_url.is_empty() {
                default_base_url_for_protocol(&self.protocol).to_string()
            } else {
                base_url.trim_end_matches('/').to_string()
            },
            preset_id: self
                .preset_id
                .clone()
                .or_else(|| prev.and_then(|p| p.preset_id.clone())),
            auth_type: if is_sub {
                Some("subscription_pool".to_string())
            } else {
                self.auth_type
                    .clone()
                    .or_else(|| prev.and_then(|p| p.auth_type.clone()))
            },
            api_keys,
            models: self.selected_models.clone(),
            // 严格遵守 AGENTS.md 铁律：绝不设置默认模型或默认服务商
            default_model: None,
            is_default: false,
            enabled: prev.map(|p| p.enabled).unwrap_or(true),
            priority: prev.map(|p| p.priority).unwrap_or(10),
            weight: prev.map(|p| p.weight).unwrap_or(1),
            timeout_secs: prev.map(|p| p.timeout_secs).unwrap_or(300),
            latency_ms: prev.and_then(|p| p.latency_ms),
            created_at: prev.and_then(|p| p.created_at.clone()),
            updated_at: prev.and_then(|p| p.updated_at.clone()),
        })
    }
}

pub fn render_provider_editor(
    app: &PivotApp,
    editor: &ProviderEditorState,
    palette: ThemePalette,
    window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    if editor.choosing_preset {
        return render_preset_chooser(app, editor, palette, window, cx).into_any_element();
    }

    let locale = app.prefs.locale;
    let is_sub = editor.is_subscription_mode();
    let is_existing = !editor.is_new || editor.editing_provider.is_some();

    div()
        .flex()
        .flex_col()
        .gap(px(11.0))
        // 表单主体：官方订阅 vs API Key 模式
        .child(if is_sub {
            render_subscription_section(app, editor, palette, window, cx)
        } else {
            render_api_key_fields(app, editor, palette, window, cx)
        })
        // 一体化模型池区块
        .child(render_model_pool_section(
            app, editor, is_sub, palette, window, cx,
        ))
        // 表单错误提示条
        .when_some(editor.error_msg.clone(), |el, msg| {
            el.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .bg(palette.rose_soft_bg)
                    .border_1()
                    .border_color(palette.rose_soft_border)
                    .child(
                        div()
                            .mt(px(1.5))
                            .child(lucide_icon(IconName::CircleAlert, 13.0, palette.rose_text)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.5))
                            .text_color(palette.rose_text)
                            .child(msg),
                    ),
            )
        })
        // 底部单行自适应操作栏
        .child(render_editor_footer(
            editor,
            is_sub,
            is_existing,
            locale,
            palette,
            cx,
        ))
        .into_any_element()
}

fn render_preset_chooser(
    app: &PivotApp,
    editor: &ProviderEditorState,
    palette: ThemePalette,
    window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> Div {
    let locale = app.prefs.locale;
    let cols = if f32::from(window.viewport_size().width) >= 520.0 {
        3usize
    } else {
        2usize
    };
    let q = editor
        .preset_search_input
        .read(cx)
        .value()
        .trim()
        .to_lowercase();

    let show_custom = q.is_empty()
        || "自定义服务商".contains(&q)
        || "custom".contains(&q)
        || "openai".contains(&q);

    let filtered_presets: Vec<Preset> = app
        .presets
        .iter()
        .filter(|p| {
            if q.is_empty() {
                return true;
            }
            p.name.to_lowercase().contains(&q)
                || p.id.to_lowercase().contains(&q)
                || p.description.to_lowercase().contains(&q)
                || p.category.to_lowercase().contains(&q)
        })
        .cloned()
        .collect();

    enum PresetTile {
        Custom,
        BuiltIn(Box<Preset>),
    }

    let mut tiles = Vec::new();
    if show_custom {
        tiles.push(PresetTile::Custom);
    }
    for pr in filtered_presets {
        tiles.push(PresetTile::BuiltIn(Box::new(pr)));
    }

    let mut grid = div().p(px(8.0)).flex().flex_col().gap(px(4.0));
    if tiles.is_empty() {
        grid = grid.child(
            div()
                .py(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.5))
                .text_color(palette.muted_foreground)
                .child(locale.pick("未找到匹配的服务商预设", "No matching presets found")),
        );
    } else {
        for chunk in tiles.chunks(cols) {
            let mut row = div().flex().items_center().gap(px(6.0));
            for item in chunk {
                match item {
                    PresetTile::Custom => {
                        row = row.child(
                            div()
                                .id("preset_tile_custom")
                                .flex_1()
                                .min_w_0()
                                .h(px(38.0))
                                .px(px(8.0))
                                .rounded(px(6.0))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .hover(move |s| s.bg(palette.accent))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.editor_choose_preset(None, window, cx);
                                    }),
                                )
                                .child(
                                    div()
                                        .size(px(24.0))
                                        .rounded(px(6.0))
                                        .flex_shrink_0()
                                        .bg(palette.icon_slot_bg)
                                        .border_1()
                                        .border_color(palette.icon_slot_border)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(lucide_icon(
                                            IconName::Plus,
                                            13.0,
                                            palette.foreground,
                                        )),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.foreground)
                                        .truncate()
                                        .child(locale.pick("自定义", "Custom")),
                                ),
                        );
                    }
                    PresetTile::BuiltIn(pr) => {
                        let is_sub = is_subscription_preset(pr);
                        let existing_sub_count: Option<usize> = if is_sub {
                            app.providers
                                .iter()
                                .find(|prov| {
                                    prov.preset_id.as_deref() == Some(pr.id.as_str())
                                        || (pr.id == "chatgpt_subscription"
                                            && prov.protocol == "chatgpt")
                                })
                                .map(|prov| {
                                    app.sub_account_stats
                                        .get(&prov.id)
                                        .map(|s| s.total)
                                        .unwrap_or(0)
                                })
                        } else {
                            None
                        };

                        let clean_name = clean_preset_name(&pr.name);
                        let pr_clone = pr.clone();
                        let tile_id = format!("preset_tile_{}", pr.id);

                        row = row.child(
                            div()
                                .id(SharedString::from(tile_id))
                                .flex_1()
                                .min_w_0()
                                .h(px(38.0))
                                .px(px(8.0))
                                .rounded(px(6.0))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(6.0))
                                .hover(move |s| s.bg(palette.accent))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, window, cx| {
                                        this.editor_choose_preset(
                                            Some((*pr_clone).clone()),
                                            window,
                                            cx,
                                        );
                                    }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .min_w_0()
                                        .flex_1()
                                        .child(render_brand_avatar(
                                            &pr.name,
                                            Some(pr.protocol.as_str()),
                                            pr.icon.as_deref().or(Some(pr.id.as_str())),
                                            24.0,
                                            true,
                                            palette,
                                        ))
                                        .child(
                                            div()
                                                .min_w_0()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(palette.foreground)
                                                .truncate()
                                                .child(clean_name),
                                        ),
                                )
                                .when(is_sub, |el| {
                                    let badge_text = match existing_sub_count {
                                        Some(n) if n > 0 => {
                                            format!("{}·{}", locale.pick("已连", "On"), n)
                                        }
                                        _ => locale.pick("订阅", "Sub").to_string(),
                                    };
                                    el.child(
                                        div()
                                            .flex_shrink_0()
                                            .px(px(4.5))
                                            .py(px(0.5))
                                            .rounded(px(3.5))
                                            .bg(palette.sub_badge_bg)
                                            .border_1()
                                            .border_color(palette.sub_badge_border)
                                            .text_size(px(9.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(palette.sub_badge_text)
                                            .child(badge_text),
                                    )
                                }),
                        );
                    }
                }
            }
            for _ in chunk.len()..cols {
                row = row.child(div().flex_1().min_w_0());
            }
            grid = grid.child(row);
        }
    }

    div()
        .w_full()
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .overflow_hidden()
        .flex()
        .flex_col()
        // 顶部搜索栏 + 返回按钮
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(10.0))
                .py(px(8.0))
                .border_b_1()
                .border_color(palette.border_subtle)
                .child(
                    div().flex_1().min_w_0().child(
                        NativeTextInput::search(&editor.preset_search_input, palette)
                            .render(window, cx),
                    ),
                )
                .child(
                    NativeButton::new(locale.pick("返回", "Back"), palette)
                        .outline()
                        .sm()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_editor_immediately(cx);
                        })),
                ),
        )
        .child(grid)
}

fn render_api_key_fields(
    app: &PivotApp,
    editor: &ProviderEditorState,
    palette: ThemePalette,
    window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> Div {
    let locale = app.prefs.locale;
    let is_custom = editor.selected_preset.is_none() && editor.preset_id.is_none();
    let can_edit_url = editor.can_edit_base_url();
    let cur_url = editor.base_url_input.read(cx).value().to_string();
    let url_hint = validate_base_url_hint(&cur_url, locale);

    let is_local_optional_key = editor.protocol == "ollama"
        || editor
            .selected_preset
            .as_ref()
            .map(|p| p.category.contains("本地"))
            .unwrap_or(false);

    let protocols: [&str; 6] = [
        "openai",
        "anthropic",
        "gemini",
        "openrouter",
        "ollama",
        "chatgpt",
    ];

    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        // 1. 服务商名称
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.5))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .child(locale.pick("名称", "Name")),
                        )
                        .when_some(editor.selected_preset.as_ref(), |el, pr| {
                            el.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(5.0))
                                    .child(render_brand_avatar(
                                        &pr.name,
                                        Some(pr.protocol.as_str()),
                                        pr.icon.as_deref().or(Some(pr.id.as_str())),
                                        16.0,
                                        true,
                                        palette,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(palette.muted_foreground)
                                            .child(clean_preset_name(&pr.name)),
                                    ),
                            )
                        }),
                )
                .child(
                    NativeTextInput::new(&editor.name_input, palette)
                        .cleanable(true)
                        .render(window, cx),
                ),
        )
        // 2. 接口地址 + 协议胶囊 + 内联测速
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.5))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(6.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.foreground)
                                        .child(locale.pick("接口地址", "Base URL")),
                                )
                                // 协议徽章或自定义协议紧凑下拉触发器
                                .child(if is_custom {
                                    div()
                                        .id("toggle_protocol_picker")
                                        .flex()
                                        .items_center()
                                        .gap(px(3.0))
                                        .px(px(6.0))
                                        .py(px(1.5))
                                        .rounded(px(4.0))
                                        .bg(if editor.protocol_picker_open {
                                            palette.accent
                                        } else {
                                            palette.secondary
                                        })
                                        .border_1()
                                        .border_color(palette.input)
                                        .cursor_pointer()
                                        .hover(move |s| s.bg(palette.accent))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                if let Some(ed) = this.editor.as_mut() {
                                                    ed.protocol_picker_open =
                                                        !ed.protocol_picker_open;
                                                    cx.notify();
                                                }
                                            }),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(palette.foreground)
                                                .child(protocol_label(&editor.protocol, locale)),
                                        )
                                        .child(lucide_icon(
                                            if editor.protocol_picker_open {
                                                IconName::ChevronUp
                                            } else {
                                                IconName::ChevronDown
                                            },
                                            10.5,
                                            palette.muted_foreground,
                                        ))
                                        .into_any_element()
                                } else {
                                    div()
                                        .px(px(6.0))
                                        .py(px(1.5))
                                        .rounded(px(4.0))
                                        .bg(palette.muted)
                                        .text_size(px(10.5))
                                        .text_color(palette.muted_foreground)
                                        .child(protocol_label(&editor.protocol, locale))
                                        .into_any_element()
                                }),
                        )
                        // 右上角内联测速操作
                        .child(render_inline_endpoint_tester(editor, locale, palette, cx)),
                )
                // 展开的紧凑协议选择列表（仅自定义模式）
                .when(is_custom && editor.protocol_picker_open, |el| {
                    let mut list = div()
                        .p(px(4.0))
                        .rounded(px(6.0))
                        .bg(palette.popover)
                        .border_1()
                        .border_color(palette.border)
                        .shadow(palette.popover_shadows())
                        .flex()
                        .flex_wrap()
                        .gap(px(2.0));

                    for proto in protocols {
                        let is_cur = editor.protocol == proto;
                        let p_str = proto.to_string();
                        list = list.child(
                            div()
                                .id(SharedString::from(format!("proto_opt_{}", proto)))
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .px(px(8.0))
                                .py(px(4.0))
                                .rounded(px(5.0))
                                .cursor_pointer()
                                .bg(if is_cur {
                                    palette.accent
                                } else {
                                    rgba(0x00000000)
                                })
                                .hover(move |s| s.bg(palette.accent))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, window, cx| {
                                        this.editor_change_protocol(p_str.clone(), window, cx);
                                    }),
                                )
                                .when(is_cur, |r| {
                                    r.child(lucide_icon(IconName::Check, 11.0, palette.foreground))
                                })
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_weight(if is_cur {
                                            FontWeight::MEDIUM
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(palette.foreground)
                                        .child(protocol_label(proto, locale)),
                                ),
                        );
                    }
                    el.child(list)
                })
                .child(
                    NativeTextInput::new(&editor.base_url_input, palette)
                        .disabled(!can_edit_url)
                        .cleanable(can_edit_url)
                        .render(window, cx),
                )
                .when_some(url_hint, |el, hint| {
                    el.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(palette.amber_text)
                            .child(hint),
                    )
                })
                .when_some(editor.endpoint_error.clone(), |el, err| {
                    el.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(palette.rose_text)
                            .child(err),
                    )
                }),
        )
        // 3. API 密钥池（内嵌眼睛显隐切换 + 多密钥轮询增删）
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.5))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(6.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.foreground)
                                        .child(if is_local_optional_key {
                                            locale.pick("API 密钥 (可选)", "API Key (Optional)")
                                        } else {
                                            locale.pick("API 密钥", "API Key")
                                        }),
                                )
                                .child(
                                    div()
                                        .id("toggle_key_hint_btn")
                                        .flex()
                                        .items_center()
                                        .gap(px(3.0))
                                        .cursor_pointer()
                                        .text_size(px(10.5))
                                        .text_color(palette.muted_foreground)
                                        .hover(move |s| s.text_color(palette.foreground))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                if let Some(ed) = this.editor.as_mut() {
                                                    ed.show_key_hint = !ed.show_key_hint;
                                                    cx.notify();
                                                }
                                            }),
                                        )
                                        .child(lucide_icon(
                                            IconName::CircleQuestionMark,
                                            11.0,
                                            palette.muted_foreground,
                                        ))
                                        .child(locale.pick("密钥说明", "Key Help")),
                                ),
                        )
                        // 添加备用轮询密钥
                        .child(
                            div()
                                .id("add_polling_key_btn")
                                .flex()
                                .items_center()
                                .gap(px(3.0))
                                .h(px(23.0))
                                .px(px(7.0))
                                .rounded(px(4.5))
                                .border_1()
                                .border_color(palette.input)
                                .bg(palette.background)
                                .cursor_pointer()
                                .hover(move |s| s.bg(palette.accent))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.editor_add_key_row(window, cx);
                                    }),
                                )
                                .child(lucide_icon(IconName::Plus, 11.0, palette.foreground))
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.foreground)
                                        .child(locale.pick("添加密钥", "Add Key")),
                                ),
                        ),
                )
                .when(editor.show_key_hint, |el| {
                    el.child(
                        div()
                            .px(px(10.0))
                            .py(px(7.0))
                            .rounded(px(6.0))
                            .bg(palette.muted_subtle)
                            .border_1()
                            .border_color(palette.border_subtle)
                            .text_size(px(11.0))
                            .line_height(rems(1.4))
                            .text_color(palette.muted_foreground)
                            .child(locale.pick(
                                "密钥仅明文存储在本地 ~/.pivot/pivot.db，绝不上传。添加多个密钥时，网关会自动按顺序轮询均衡，并在触发 429 限流时自动切换至下一个密钥。",
                                "Keys are stored locally in ~/.pivot/pivot.db only. Multiple keys are automatically load-balanced via round-robin with automatic failover on 429.",
                            )),
                    )
                })
                .child({
                    let total_keys = editor.key_inputs.len();
                    let show_keys = editor.show_keys;
                    let mut col = div().flex().flex_col().gap(px(5.0));
                    for (idx, ki) in editor.key_inputs.iter().enumerate() {
                        let eye_btn = div()
                            .id(SharedString::from(format!("toggle_mask_key_{}", idx)))
                            .size(px(20.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(move |s| s.bg(palette.accent))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.editor_toggle_keys_visibility(window, cx);
                                }),
                            )
                            .child(lucide_icon(
                                if show_keys {
                                    IconName::EyeOff
                                } else {
                                    IconName::Eye
                                },
                                12.5,
                                palette.muted_foreground,
                            ));

                        col = col.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div().flex_1().min_w_0().child(
                                        NativeTextInput::new(ki, palette)
                                            .suffix(eye_btn)
                                            .render(window, cx),
                                    ),
                                )
                                .when(total_keys > 1, |row| {
                                    row.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "remove_key_row_{}",
                                                idx
                                            )))
                                            .size(px(30.0))
                                            .rounded(px(6.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .border_1()
                                            .border_color(palette.input)
                                            .bg(palette.background)
                                            .hover(move |s| s.bg(palette.destructive_soft_bg))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.editor_remove_key_row(idx, cx);
                                                }),
                                            )
                                            .child(lucide_icon(
                                                IconName::X,
                                                12.5,
                                                palette.muted_foreground,
                                            )),
                                    )
                                }),
                        );
                    }
                    col
                }),
        )
}

fn render_inline_endpoint_tester(
    editor: &ProviderEditorState,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> AnyElement {
    if editor.testing_endpoint {
        return div()
            .flex()
            .items_center()
            .gap(px(5.0))
            .child(lucide_spinner(
                "endpoint_test_spin",
                11.5,
                palette.muted_foreground,
            ))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(palette.muted_foreground)
                    .child(locale.pick("测试中…", "Testing…")),
            )
            .into_any_element();
    }

    if let Some(ms) = editor.endpoint_latency {
        let color = if ms < 600 {
            palette.emerald_text
        } else if ms < 1500 {
            palette.amber_text
        } else {
            palette.rose_text
        };
        return div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(div().size(px(5.5)).rounded_full().bg(color))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(color)
                            .child(format!("{} ms", ms)),
                    ),
            )
            .child(
                div()
                    .id("retest_endpoint_btn")
                    .cursor_pointer()
                    .text_size(px(11.0))
                    .text_color(palette.muted_foreground)
                    .hover(move |s| s.text_color(palette.foreground))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.editor_test_endpoint(cx);
                        }),
                    )
                    .child(locale.pick("重测", "Retest")),
            )
            .into_any_element();
    }

    if editor.endpoint_error.is_some() {
        return div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.rose_text)
                    .child(locale.pick("无法连接", "Unreachable")),
            )
            .child(
                div()
                    .id("retest_endpoint_err_btn")
                    .cursor_pointer()
                    .text_size(px(11.0))
                    .text_color(palette.muted_foreground)
                    .hover(move |s| s.text_color(palette.foreground))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.editor_test_endpoint(cx);
                        }),
                    )
                    .child(locale.pick("重试", "Retry")),
            )
            .into_any_element();
    }

    div()
        .id("test_endpoint_btn")
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(move |s| s.bg(palette.accent))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.editor_test_endpoint(cx);
            }),
        )
        .child(lucide_icon(IconName::Zap, 11.5, palette.muted_foreground))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(palette.muted_foreground)
                .child(locale.pick("测试连接", "Test")),
        )
        .into_any_element()
}

fn render_editor_footer(
    editor: &ProviderEditorState,
    is_sub: bool,
    is_existing: bool,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let sub_ready = is_sub && !editor.sub_accounts.is_empty() && !editor.selected_models.is_empty();

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .pt(px(10.0))
        .border_t_1()
        .border_color(palette.border)
        // 左侧：编辑已有服务商时展示删除按钮
        .child(if is_existing {
            NativeButton::new(locale.pick("删除", "Delete"), palette)
                .danger_ghost()
                .sm()
                .icon(IconName::Trash)
                .disabled(editor.saving)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.editor_prompt_delete_current(cx);
                }))
        } else {
            div()
        })
        // 右侧：状态文案 + 返回/完成/保存
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .when(editor.fetching_models, |el| {
                    el.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(palette.muted_foreground)
                            .child(locale.pick("正在同步上游模型…", "Syncing models…")),
                    )
                })
                .when(!editor.fetching_models && editor.auto_saved && sub_ready, |el| {
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .size(px(5.5))
                                    .rounded_full()
                                    .bg(palette.emerald_text),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(palette.emerald_text)
                                    .child(locale.pick(
                                        "已自动保存并启用",
                                        "Auto-saved & enabled",
                                    )),
                            ),
                    )
                })
                .when(
                    !editor.fetching_models
                        && (!is_sub || !editor.sub_accounts.is_empty())
                        && editor.selected_models.is_empty(),
                    |el| {
                        el.child(
                            div()
                                .text_size(px(11.0))
                                .text_color(palette.amber_text)
                                .child(locale.pick("请先选择模型", "Select models first")),
                        )
                    },
                )
                .child(if is_sub && !editor.sub_accounts.is_empty() {
                    NativeButton::new(
                        if editor.saving {
                            locale.pick("保存中…", "Saving…")
                        } else {
                            locale.pick("完成", "Done")
                        },
                        palette,
                    )
                    .primary()
                    .sm()
                    .disabled(editor.saving || editor.selected_models.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.editor_save(cx);
                    }))
                    .into_any_element()
                } else {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            NativeButton::new(locale.pick("返回", "Back"), palette)
                                .outline()
                                .sm()
                                .disabled(editor.saving)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.editor_handle_back(window, cx);
                                })),
                        )
                        .child(
                            NativeButton::new(
                                if editor.saving {
                                    locale.pick("保存中…", "Saving…")
                                } else {
                                    locale.pick("保存", "Save")
                                },
                                palette,
                            )
                            .primary()
                            .sm()
                            .disabled(editor.saving)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.editor_save(cx);
                            })),
                        )
                        .into_any_element()
                }),
        )
}

#[allow(dead_code)]
fn _keep_ref(p: &Provider) -> bool {
    is_subscription_provider(p)
}
