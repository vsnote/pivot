use crate::PivotApp;
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub fn render_clients_view(
    app: &PivotApp,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let interceptors = app.interceptors.clone();
    let providers = app.providers.clone();

    // 收集所有已启用服务商声明的所有模型池
    let mut available_models = Vec::new();
    for p in &providers {
        if p.enabled {
            for m in &p.models {
                if !available_models.contains(m) {
                    available_models.push(m.clone());
                }
            }
        }
    }

    if interceptors.is_empty() {
        return div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .h(px(300.0))
            .w_full()
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(0x71717a))
                    .child("正在探测本地 Agent 与命令行客户端..."),
            );
    }

    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(14.0))
        .children(interceptors.into_iter().map(|item| {
            let item_id = item.id.clone();
            let is_enabled = item.is_intercepted;
            let current_model = item.model.clone();
            let has_conflict = item.conflict.is_some();
            let conflict_info = item.conflict.clone();

            div()
                .flex()
                .flex_col()
                .w_full()
                .p(px(16.0))
                .rounded(px(10.0))
                .bg(rgb(0x202023))
                .border_1()
                .border_color(rgba(0xffffff10))
                .gap(px(12.0))
                .child(
                    // 头部：Agent 名称、拦截开关、冲突标识
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .text_size(px(15.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xffffff))
                                        .child(item.name.clone()),
                                )
                                .child(
                                    div()
                                        .px(px(6.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(if is_enabled {
                                            rgba(0x22c55e1a)
                                        } else {
                                            rgba(0xffffff0a)
                                        })
                                        .text_size(px(11.0))
                                        .text_color(if is_enabled {
                                            rgb(0x4ade80)
                                        } else {
                                            rgb(0x71717a)
                                        })
                                        .child(if is_enabled {
                                            "已接管代理"
                                        } else {
                                            "未接管"
                                        }),
                                )
                                .when(has_conflict, |el| {
                                    el.child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .bg(rgba(0xef44441a))
                                            .text_size(px(11.0))
                                            .text_color(rgb(0xf87171))
                                            .child("存在外部端口冲突"),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.0))
                                .child(
                                    Switch::new(format!("interceptor-switch-{}", item_id))
                                        .checked(is_enabled)
                                        .on_click({
                                            let id = item_id.clone();
                                            cx.listener(move |this, checked: &bool, _, cx| {
                                                this.toggle_interceptor(id.clone(), *checked, cx);
                                            })
                                        }),
                                ),
                        ),
                )
                .when_some(conflict_info, |el, info| {
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .p(px(10.0))
                            .rounded(px(6.0))
                            .bg(rgba(0xef444410))
                            .border_1()
                            .border_color(rgba(0xef444425))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xfca5a5))
                                    .child(format!("发现外部第三方配置冲突: {}", info)),
                            ),
                    )
                })
                .child(
                    // 核心铁律：各 Agent 独立选择启动模型
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(0xa1a1aa))
                                        .child("启动模型选择:"),
                                )
                                .child(
                                    div()
                                        .px(px(8.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(rgba(0x3b82f61a))
                                        .border_1()
                                        .border_color(rgba(0x3b82f630))
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(0x60a5fa))
                                        .child(current_model.clone().unwrap_or_else(|| "未指定".to_string())),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(6.0))
                                .children(available_models.iter().map(|model_name| {
                                    let is_selected = current_model.as_deref() == Some(model_name.as_str());
                                    let client_id = item_id.clone();
                                    let m = model_name.clone();

                                    div()
                                        .px(px(8.0))
                                        .py(px(4.0))
                                        .rounded(px(4.0))
                                        .cursor_pointer()
                                        .text_size(px(11.0))
                                        .font_weight(if is_selected {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .bg(if is_selected {
                                            rgb(0x2563eb)
                                        } else {
                                            rgba(0xffffff0a)
                                        })
                                        .hover(|s| {
                                            if !is_selected {
                                                s.bg(rgba(0xffffff15))
                                            } else {
                                                s
                                            }
                                        })
                                        .text_color(if is_selected {
                                            rgb(0xffffff)
                                        } else {
                                            rgb(0xd4d4d8)
                                        })
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                            this.set_interceptor_model(client_id.clone(), Some(m.clone()), cx);
                                        }))
                                        .child(model_name.clone())
                                })),
                        ),
                )
        }))
}
