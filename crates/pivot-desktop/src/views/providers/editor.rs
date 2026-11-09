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
    let edit_id = app.editor_provider_id.clone();

    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(960.0))
        .mx_auto()
        .gap(px(14.0))
        .child(
            // 原版风格面板卡片
            div()
                .flex()
                .flex_col()
                .w_full()
                .p(px(20.0))
                .rounded(px(8.0))
                .bg(rgb(0x1c1c1f))
                .border_1()
                .border_color(rgb(0x2e2e32))
                .gap(px(18.0))
                .child(
                    // 表单头部：标题与返回/删除
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pb(px(14.0))
                        .border_b_1()
                        .border_color(rgb(0x2e2e32))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(0xededee))
                                        .child(if is_edit {
                                            "编辑服务商"
                                        } else {
                                            "新增服务商"
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .when(is_edit, |el| {
                                    el.child(
                                        div()
                                            .px(px(8.0))
                                            .py(px(4.0))
                                            .rounded(px(5.0))
                                            .cursor_pointer()
                                            .bg(rgba(0xef444415))
                                            .hover(|s| s.bg(rgba(0xef444425)))
                                            .text_size(px(11.0))
                                            .text_color(rgb(0xf87171))
                                            .on_mouse_down(MouseButton::Left, cx.listener({
                                                let id = edit_id.clone().unwrap_or_default();
                                                move |this, _, _, cx| {
                                                    this.delete_provider(id.clone(), cx);
                                                    this.close_editor(cx);
                                                }
                                            }))
                                            .child("删除服务商"),
                                    )
                                })
                                .child(
                                    div()
                                        .px(px(10.0))
                                        .py(px(4.0))
                                        .rounded(px(5.0))
                                        .cursor_pointer()
                                        .bg(rgb(0x27272a))
                                        .hover(|s| s.bg(rgb(0x323238)))
                                        .text_size(px(11.0))
                                        .text_color(rgb(0xa1a1aa))
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                            this.close_editor(cx);
                                        }))
                                        .child("返回列表"),
                                ),
                        ),
                )
                // 协议分段标签
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xa1a1aa))
                                .child("协议类型 (Protocol)"),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(6.0))
                                .children(["openai", "anthropic", "gemini", "openrouter", "ollama"].iter().map(|&proto| {
                                    let is_active = current_proto == proto;
                                    let p = proto.to_string();
                                    div()
                                        .px(px(10.0))
                                        .py(px(4.0))
                                        .rounded(px(5.0))
                                        .cursor_pointer()
                                        .text_size(px(11.0))
                                        .font_weight(if is_active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                        .bg(if is_active {
                                            rgb(0x2563eb)
                                        } else {
                                            rgb(0x27272a)
                                        })
                                        .text_color(if is_active {
                                            rgb(0xffffff)
                                        } else {
                                            rgb(0xa1a1aa)
                                        })
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                            this.editor_protocol = p.clone();
                                            cx.notify();
                                        }))
                                        .child(proto.to_uppercase())
                                })),
                        ),
                )
                // 服务商名称
                .when_some(app.editor_name.as_ref(), |el, input| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xa1a1aa))
                                    .child("服务商显示名称 (Name)"),
                            )
                            .child(Input::new(input)),
                    )
                })
                // Base URL
                .when_some(app.editor_url.as_ref(), |el, input| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xa1a1aa))
                                    .child("API 端点地址 (Base URL)"),
                            )
                            .child(Input::new(input)),
                    )
                })
                // API Key
                .when_some(app.editor_key.as_ref(), |el, input| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xa1a1aa))
                                    .child("API 密钥 (API Key)"),
                            )
                            .child(Input::new(input)),
                    )
                })
                // 声明可用模型
                .when_some(app.editor_models.as_ref(), |el, input| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xa1a1aa))
                                    .child("声明的模型列表 (逗号分隔，如 gpt-4o, claude-3-7-sonnet, deepseek-chat)"),
                            )
                            .child(Input::new(input)),
                    )
                })
                // 底部操作栏
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(10.0))
                        .pt(px(10.0))
                        .border_t_1()
                        .border_color(rgb(0x2e2e32))
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
                ),
        )
}
