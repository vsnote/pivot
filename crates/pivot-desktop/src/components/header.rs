use crate::ActiveTab;
use gpui_kit::*;

pub fn render_header(
    active_tab: ActiveTab,
    port: u16,
    _window: &mut Window,
    cx: &mut Context<crate::PivotApp>,
) -> impl IntoElement {
    let is_running = true;

    div()
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .h(px(52.0))
        .pl(px(80.0)) // 为 macOS 交通灯留出优雅空间
        .pr(px(16.0))
        .border_b_1()
        .border_color(rgba(0xffffff15))
        .bg(rgb(0x18181b))
        .child(
            // 左侧：品牌 Logo + 网关运行状态指示
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .child("Pivot"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(8.0))
                        .py(px(2.0))
                        .rounded(px(12.0))
                        .bg(if is_running {
                            rgba(0x22c55e1a)
                        } else {
                            rgba(0xef44441a)
                        })
                        .child(
                            div()
                                .size(px(6.0))
                                .rounded_full()
                                .bg(if is_running {
                                    rgb(0x22c55e)
                                } else {
                                    rgb(0xef4444)
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(if is_running {
                                    rgb(0x4ade80)
                                } else {
                                    rgb(0xf87171)
                                })
                                .child(format!(":{}", port)),
                        ),
                ),
        )
        .child(
            // 中间：Tab 导航栏
            div()
                .flex()
                .items_center()
                .p(px(3.0))
                .rounded(px(8.0))
                .bg(rgba(0xffffff0a))
                .gap(px(2.0))
                .child(render_tab_item(
                    "服务商",
                    "⌘1",
                    active_tab == ActiveTab::Providers,
                    cx.listener(|this, _, _, cx| {
                        this.active_tab = ActiveTab::Providers;
                        cx.notify();
                    }),
                ))
                .child(render_tab_item(
                    "客户端",
                    "⌘2",
                    active_tab == ActiveTab::Clients,
                    cx.listener(|this, _, _, cx| {
                        this.active_tab = ActiveTab::Clients;
                        cx.notify();
                    }),
                ))
                .child(render_tab_item(
                    "看板与日志",
                    "⌘3",
                    active_tab == ActiveTab::Logs,
                    cx.listener(|this, _, _, cx| {
                        this.active_tab = ActiveTab::Logs;
                        cx.notify();
                    }),
                ))
                .child(render_tab_item(
                    "设置",
                    "⌘4",
                    active_tab == ActiveTab::Settings,
                    cx.listener(|this, _, _, cx| {
                        this.active_tab = ActiveTab::Settings;
                        cx.notify();
                    }),
                )),
        )
        .child(
            // 右侧：+ 新增服务商按钮
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(12.0))
                        .py(px(6.0))
                        .rounded(px(6.0))
                        .bg(rgb(0x2563eb))
                        .text_color(rgb(0xffffff))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x1d4ed8)))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                            this.open_editor(None, window, cx);
                        }))
                        .child("+ 新增服务商")
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgba(0xffffff99))
                                .child("⌘N"),
                        ),
                ),
        )
}

fn render_tab_item(
    label: &'static str,
    shortcut: &'static str,
    is_active: bool,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(12.0))
        .py(px(5.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .bg(if is_active {
            rgba(0xffffff1f)
        } else {
            rgba(0x00000000)
        })
        .hover(|s| {
            if !is_active {
                s.bg(rgba(0xffffff0f))
            } else {
                s
            }
        })
        .on_mouse_down(MouseButton::Left, on_click)
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(if is_active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_active {
                    rgb(0xffffff)
                } else {
                    rgb(0xa1a1aa)
                })
                .child(label),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(if is_active {
                    rgb(0xd4d4d8)
                } else {
                    rgb(0x71717a)
                })
                .child(shortcut),
        )
}
