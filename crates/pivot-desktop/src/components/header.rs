use crate::ActiveTab;
use gpui_kit::*;

pub fn render_header(
    active_tab: ActiveTab,
    _port: u16,
    _window: &mut Window,
    cx: &mut Context<crate::PivotApp>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .w_full()
        .h(px(48.0))
        .pl(px(96.0)) // 避让 macOS 原生红绿灯 (96px 安全区)
        .pr(px(16.0))
        .border_b_1()
        .border_color(rgb(0x2e2e32))
        .bg(rgb(0x18181b))
        .child(
            // 原版风格：纯净幽灵浮动 Tab 组
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(render_ghost_tab(
                    "服务商",
                    active_tab == ActiveTab::Providers,
                    cx.listener(|this, _, _, cx| {
                        if this.active_tab == ActiveTab::Providers && this.is_provider_editor_open {
                            this.close_editor(cx);
                        } else {
                            this.active_tab = ActiveTab::Providers;
                        }
                        cx.notify();
                    }),
                ))
                .child(render_ghost_tab(
                    "客户端",
                    active_tab == ActiveTab::Clients,
                    cx.listener(|this, _, _, cx| {
                        this.active_tab = ActiveTab::Clients;
                        cx.notify();
                    }),
                ))
                .child(render_ghost_tab(
                    "仪表盘",
                    active_tab == ActiveTab::Logs,
                    cx.listener(|this, _, _, cx| {
                        this.active_tab = ActiveTab::Logs;
                        cx.notify();
                    }),
                ))
                .child(render_ghost_tab(
                    "设置",
                    active_tab == ActiveTab::Settings,
                    cx.listener(|this, _, _, cx| {
                        this.active_tab = ActiveTab::Settings;
                        cx.notify();
                    }),
                )),
        )
        // 剩余全部区域为窗口原生拖拽区，保持通透
        .child(div().flex_1().h_full())
}

fn render_ghost_tab(
    label: &'static str,
    is_active: bool,
    on_click: impl Fn(&gpui_kit::MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .h(px(28.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .text_size(px(12.0))
        .font_weight(if is_active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .text_color(if is_active {
            rgb(0xededee)
        } else {
            rgb(0xa1a1aa)
        })
        .bg(if is_active {
            rgb(0x27272a)
        } else {
            rgba(0x00000000)
        })
        .border_1()
        .border_color(if is_active {
            rgb(0x3f3f46)
        } else {
            rgba(0x00000000)
        })
        .hover(|style| {
            if !is_active {
                style
                    .bg(rgb(0x222226))
                    .text_color(rgb(0xffffff))
            } else {
                style
            }
        })
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label)
}
