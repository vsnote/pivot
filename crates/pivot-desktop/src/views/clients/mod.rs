pub mod conflict_modal;
pub mod model_select;

use crate::components::button::NativeButton;
use crate::components::icons::{lucide_icon, lucide_spinner, render_client_avatar};
use crate::components::switch::NativeSwitch;
use crate::i18n::{client_type_label, Locale};
use crate::theme::ThemePalette;
use crate::views::clients::model_select::{
    build_grouped_models, render_client_model_trigger, resolve_default_model_for_client,
};
use crate::PivotApp;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub use conflict_modal::render_conflict_dialog;
pub use model_select::{render_client_model_popover, ClientModelSelectState};

pub fn render_clients_view(
    app: &PivotApp,
    palette: ThemePalette,
    window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let locale = app.prefs.locale;

    if !app.initial_loaded {
        return div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .py(px(48.0))
            .gap(px(8.0))
            .child(lucide_spinner(
                "clients_loading_spinner",
                22.0,
                palette.muted_foreground,
            ))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(palette.muted_foreground)
                    .child(locale.pick("载入客户端状态…", "Loading agent status…")),
            );
    }

    let grouped_models = build_grouped_models(&app.providers);
    let has_enabled_providers = app.providers.iter().any(|p| p.enabled);
    let gateway_up = app.gateway_status.is_running;
    let port = app.gateway_status.port;

    // 1. 客户端接管主卡片
    let mut list_card = div()
        .w_full()
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .overflow_hidden()
        .flex()
        .flex_col();

    if app.interceptors.is_empty() {
        list_card = list_card.child(
            div()
                .px(px(16.0))
                .py(px(28.0))
                .text_size(px(12.0))
                .text_color(palette.muted_foreground)
                .text_center()
                .child(locale.pick("未检测到支持的编程客户端", "No supported agents found")),
        );
    } else {
        for (idx, client) in app.interceptors.iter().enumerate() {
            let label = client_type_label(&client.id, locale);
            let is_busy = app.toggling_interceptor_id.as_deref() == Some(client.id.as_str());
            let any_busy = app.toggling_interceptor_id.is_some();
            let is_select_open = app
                .client_model_select
                .as_ref()
                .map(|s| s.client_id == client.id)
                .unwrap_or(false);

            let fallback_default = resolve_default_model_for_client(&client.id, &app.providers);
            let effective_model = client
                .model
                .clone()
                .filter(|s| !s.trim().is_empty())
                .or(fallback_default)
                .unwrap_or_default();

            let client_for_conflict = client.clone();
            let client_for_switch = client.clone();
            let next_checked = !client.is_intercepted;

            list_card = list_card.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .px(px(14.0))
                    .py(px(8.5))
                    .min_h(px(42.0))
                    .when(idx > 0, |el| {
                        el.border_t_1().border_color(palette.border)
                    })
                    .when(!client.installed, |el| el.opacity(0.6))
                    // 左侧图标 + 标准客户端名
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .min_w_0()
                            .child(render_client_avatar(&client.id, 24.0, true, palette))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.foreground)
                                    .truncate()
                                    .child(label),
                            ),
                    )
                    // 右侧：已安装显示模型选择器 + 开关/解决冲突；未安装仅显示纯文本
                    .child(if client.installed {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .flex_shrink_0()
                            .child(render_client_model_trigger(
                                &client.id,
                                &effective_model,
                                grouped_models.is_empty() || is_busy,
                                locale.pick("无可用模型", "No models"),
                                &grouped_models,
                                is_select_open,
                                app.trigger_bounds.clone(),
                                palette,
                                cx,
                            ))
                            .child(if is_busy {
                                div()
                                    .w(px(36.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(lucide_spinner(
                                        format!("client_spin_{}", client.id),
                                        14.0,
                                        palette.muted_foreground,
                                    ))
                                    .into_any_element()
                            } else if client.is_overwritten {
                                div()
                                    .id(SharedString::from(format!(
                                        "resolve_conflict_{}",
                                        client.id
                                    )))
                                    .px(px(8.0))
                                    .py(px(2.5))
                                    .rounded(px(4.5))
                                    .bg(palette.amber_soft_bg)
                                    .border_1()
                                    .border_color(palette.amber_soft_border)
                                    .cursor_pointer()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.amber_text)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.conflict_target =
                                                Some(client_for_conflict.clone());
                                            cx.notify();
                                        }),
                                    )
                                    .child(locale.pick("解决冲突", "Resolve"))
                                    .into_any_element()
                            } else {
                                NativeSwitch::new(
                                    format!("client_switch_{}", client.id),
                                    client.is_intercepted,
                                    palette,
                                )
                                .disabled(any_busy)
                                .on_toggle(cx.listener(move |this, _, _, cx| {
                                    this.handle_client_switch(
                                        client_for_switch.clone(),
                                        next_checked,
                                        cx,
                                    );
                                }))
                                .into_any_element()
                            })
                    } else {
                        div()
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .text_size(px(11.0))
                            .text_color(palette.muted_foreground)
                            .child(locale.pick("未安装", "Not installed"))
                    }),
            );
        }
    }

    // 2. 动态状态提示文案（4 级优先级）
    let any_taken_over = app.interceptors.iter().any(|c| c.is_intercepted);
    let any_installed = app.interceptors.iter().any(|c| c.installed);
    let (footer_text, footer_color) = if !gateway_up {
        (
            format!(
                "{} {}",
                locale.pick("本地网关端口未监听：", "Gateway port not listening:"),
                port
            ),
            palette.destructive_text,
        )
    } else if any_taken_over && !has_enabled_providers {
        (
            locale
                .pick(
                    "已接管客户端，但当前没有已启用的服务商渠道，请求将无法转发",
                    "Agents intercepted, but no providers are enabled yet",
                )
                .to_string(),
            palette.amber_text,
        )
    } else if has_enabled_providers && any_installed && !any_taken_over {
        (
            locale
                .pick(
                    "开启右侧开关即可将客户端流量无感接入本地网关，关闭即自动还原原配置",
                    "Turn on a switch to route agent traffic through Pivot; turn off to restore",
                )
                .to_string(),
            palette.muted_foreground,
        )
    } else {
        (
            locale
                .pick(
                    "开启接管时自动备份原配置，关闭开关即刻无损恢复",
                    "Configs are backed up automatically and restored cleanly when turned off",
                )
                .to_string(),
            palette.muted_foreground,
        )
    };

    let is_wide = f32::from(window.viewport_size().width) >= 520.0;

    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(list_card)
        .child(
            div()
                .px(px(4.0))
                .text_size(px(11.0))
                .line_height(rems(1.4))
                .text_color(footer_color)
                .child(footer_text),
        )
        .child(render_universal_client_card(
            app, is_wide, locale, palette, cx,
        ))
}

fn render_universal_client_card(
    app: &PivotApp,
    is_wide: bool,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let expanded = app.universal_card_expanded;
    let port = app.gateway_status.port;
    let openai_url = format!("http://127.0.0.1:{}/v1", port);
    let anthropic_url = format!("http://127.0.0.1:{}", port);

    let copied_openai = app.copied_id.as_deref() == Some("univ_openai");
    let copied_anthropic = app.copied_id.as_deref() == Some("univ_anthropic");

    let openai_copy_val = openai_url.clone();
    let anthropic_copy_val = anthropic_url.clone();

    let openai_box = div()
        .flex_1()
        .min_w_0()
        .p(px(12.0))
        .rounded(px(6.0))
        .bg(palette.icon_slot_bg)
        .border_1()
        .border_color(palette.border_subtle)
        .flex()
        .flex_col()
        .justify_between()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(5.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .child(locale.pick("OpenAI 兼容协议", "OpenAI Compatible")),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(palette.muted_foreground)
                                .child("/v1/chat/completions"),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(palette.muted_foreground)
                        .truncate()
                        .child(openai_url),
                ),
        )
        .child(
            div().flex().items_center().child(
                NativeButton::new(
                    if copied_openai {
                        locale.pick("已复制", "Copied")
                    } else {
                        locale.pick("复制 Base URL", "Copy Base URL")
                    },
                    palette,
                )
                .outline()
                .xs()
                .icon_colored(
                    if copied_openai {
                        IconName::Check
                    } else {
                        IconName::Copy
                    },
                    if copied_openai {
                        palette.emerald_text
                    } else {
                        palette.muted_foreground
                    },
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.copy_to_clipboard(
                        "univ_openai".to_string(),
                        openai_copy_val.clone(),
                        cx,
                    );
                })),
            ),
        );

    let anthropic_box = div()
        .flex_1()
        .min_w_0()
        .p(px(12.0))
        .rounded(px(6.0))
        .bg(palette.icon_slot_bg)
        .border_1()
        .border_color(palette.border_subtle)
        .flex()
        .flex_col()
        .justify_between()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(5.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .child(locale.pick("Anthropic 兼容协议", "Anthropic Compatible")),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(palette.muted_foreground)
                                .child("/v1/messages"),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(palette.muted_foreground)
                        .truncate()
                        .child(anthropic_url),
                ),
        )
        .child(
            div().flex().items_center().child(
                NativeButton::new(
                    if copied_anthropic {
                        locale.pick("已复制", "Copied")
                    } else {
                        locale.pick("复制 Base URL", "Copy Base URL")
                    },
                    palette,
                )
                .outline()
                .xs()
                .icon_colored(
                    if copied_anthropic {
                        IconName::Check
                    } else {
                        IconName::Copy
                    },
                    if copied_anthropic {
                        palette.emerald_text
                    } else {
                        palette.muted_foreground
                    },
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.copy_to_clipboard(
                        "univ_anthropic".to_string(),
                        anthropic_copy_val.clone(),
                        cx,
                    );
                })),
            ),
        );

    div()
        .mt(px(4.0))
        .w_full()
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .overflow_hidden()
        .flex()
        .flex_col()
        // 可点击折叠标题栏（对齐原版 bg-muted/30）
        .child(
            div()
                .id("toggle_universal_client_card")
                .flex()
                .items_center()
                .justify_between()
                .px(px(14.0))
                .py(px(9.0))
                .cursor_pointer()
                .bg(palette.muted_subtle)
                .when(expanded, |el| {
                    el.border_b_1().border_color(palette.border)
                })
                .hover(move |s| s.bg(palette.accent))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.universal_card_expanded = !this.universal_card_expanded;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.0))
                        .child(lucide_icon(
                            IconName::CodeXml,
                            13.0,
                            palette.muted_foreground,
                        ))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .child(locale.pick("通用接入端点", "Universal Endpoints")),
                        ),
                )
                .child(lucide_icon(
                    if expanded {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    },
                    13.5,
                    palette.muted_foreground,
                )),
        )
        .when(expanded, |el| {
            el.child(
                div()
                    .p(px(14.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .line_height(rems(1.4))
                            .text_color(palette.muted_foreground)
                            .child(locale.pick(
                                "适用于 Cursor、Cline、Roo Code、Cherry Studio 等任意自定义 API 工具，API Key 可填任意非空字符（如 pivot）",
                                "Works with Cursor, Cline, Roo Code, Cherry Studio, or any custom client. Use any non-empty string (e.g. pivot) as API Key.",
                            )),
                    )
                    .child(if is_wide {
                        div()
                            .flex()
                            .items_stretch()
                            .gap(px(10.0))
                            .child(openai_box)
                            .child(anthropic_box)
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(openai_box)
                            .child(anthropic_box)
                    }),
            )
        })
}
