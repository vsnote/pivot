use crate::PivotApp;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub fn render_provider_editor(
    app: &PivotApp,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let is_edit = app.editor_provider_id.is_some();
    let current_proto = app.editor_protocol.clone();

    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(720.0))
        .mx_auto()
        .p(px(24.0))
        .rounded(px(12.0))
        .bg(rgb(0x202023))
        .border_1()
        .border_color(rgba(0xffffff15))
        .gap(px(18.0))
        .child(
            // 表单标题与返回按钮
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(18.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child(if is_edit {
                            "编辑服务商"
                        } else {
                            "新增服务商"
                        }),
                )
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(4.0))
                        .rounded(px(6.0))
                        .cursor_pointer()
                        .bg(rgba(0xffffff0a))
                        .hover(|s| s.bg(rgba(0xffffff15)))
                        .text_size(px(12.0))
                        .text_color(rgb(0xa1a1aa))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.close_editor(cx);
                        }))
                        .child("返回列表"),
                ),
        )
        .child(
            // 协议类型快速选择
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0xa1a1aa))
                        .child("协议类型 (Protocol)"),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .children(["openai", "anthropic", "gemini", "openrouter", "ollama"].iter().map(|&proto| {
                            let is_active = current_proto == proto;
                            let p = proto.to_string();
                            div()
                                .px(px(12.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .cursor_pointer()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .bg(if is_active {
                                    rgb(0x2563eb)
                                } else {
                                    rgba(0xffffff0a)
                                })
                                .text_color(if is_active {
                                    rgb(0xffffff)
                                } else {
                                    rgb(0xd4d4d8)
                                })
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.editor_protocol = p.clone();
                                    cx.notify();
                                }))
                                .child(proto.to_uppercase())
                        })),
                ),
        )
        .when_some(app.editor_name.as_ref(), |el, input| {
            el.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xa1a1aa))
                            .child("服务商显示名称 (Name)"),
                    )
                    .child(Input::new(input)),
            )
        })
        .when_some(app.editor_url.as_ref(), |el, input| {
            el.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xa1a1aa))
                            .child("API 地址 (Base URL)"),
                    )
                    .child(Input::new(input)),
            )
        })
        .when_some(app.editor_key.as_ref(), |el, input| {
            el.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xa1a1aa))
                            .child("API 密钥 (API Key)"),
                    )
                    .child(Input::new(input)),
            )
        })
        .when_some(app.editor_models.as_ref(), |el, input| {
            el.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xa1a1aa))
                            .child("声明的模型列表 (逗号分隔，如 gpt-4o, claude-3-7-sonnet)"),
                    )
                    .child(Input::new(input)),
            )
        })
        .child(
            // 底部操作栏
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(12.0))
                .mt(px(12.0))
                .child(
                    Button::new("cancel-btn")
                        .with_variant(ButtonVariant::Ghost)
                        .label("取消")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_editor(cx);
                        })),
                )
                .child(
                    Button::new("save-btn")
                        .with_variant(ButtonVariant::Primary)
                        .label("保存服务商")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.save_current_editor(cx);
                        })),
                ),
        )
}
