use crate::theme::ThemePalette;
use crate::{ActiveTab, PivotApp};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub fn render_header(
    app: &PivotApp,
    palette: ThemePalette,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let active_tab = app.active_tab;
    let locale = app.prefs.locale;
    let has_update = app.update_version.is_some();

    div()
        .flex()
        .items_center()
        .w_full()
        .h(px(48.0))
        .flex_shrink_0()
        .border_b_1()
        .border_color(palette.border)
        .bg(palette.header_bg)
        // 左侧 96px 红绿灯安全区，同样允许拖拽窗口
        .child(
            div()
                .w(px(96.0))
                .h_full()
                .flex_shrink_0()
                .window_control_area(WindowControlArea::Drag),
        )
        // 幽灵浮动主导航 Tab 组
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(render_ghost_tab(
                    locale.pick("服务商", "Providers"),
                    active_tab == ActiveTab::Providers,
                    false,
                    palette,
                    cx.listener(|this, _, _, cx| {
                        if this.active_tab == ActiveTab::Providers && this.is_provider_editor_open {
                            this.request_close_editor(cx);
                        } else {
                            this.switch_tab(ActiveTab::Providers, cx);
                        }
                    }),
                ))
                .child(render_ghost_tab(
                    locale.pick("客户端", "Agents"),
                    active_tab == ActiveTab::Clients,
                    false,
                    palette,
                    cx.listener(|this, _, _, cx| {
                        this.switch_tab(ActiveTab::Clients, cx);
                    }),
                ))
                .child(render_ghost_tab(
                    locale.pick("统计", "Stats"),
                    active_tab == ActiveTab::Logs,
                    false,
                    palette,
                    cx.listener(|this, _, _, cx| {
                        this.switch_tab(ActiveTab::Logs, cx);
                    }),
                ))
                .child(render_ghost_tab(
                    locale.pick("设置", "Settings"),
                    active_tab == ActiveTab::Settings,
                    has_update,
                    palette,
                    cx.listener(|this, _, _, cx| {
                        this.switch_tab(ActiveTab::Settings, cx);
                    }),
                )),
        )
        // 右侧剩余全部区域声明为原生窗口拖拽区
        .child(
            div()
                .flex_1()
                .h_full()
                .window_control_area(WindowControlArea::Drag),
        )
}

fn render_ghost_tab(
    label: &'static str,
    is_active: bool,
    show_dot: bool,
    palette: ThemePalette,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .h(px(28.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if is_active {
            palette.foreground
        } else {
            palette.muted_foreground
        })
        .bg(if is_active {
            palette.background
        } else {
            rgba(0x00000000)
        })
        .border_1()
        .border_color(if is_active {
            palette.border
        } else {
            rgba(0x00000000)
        })
        .hover(move |style| {
            if !is_active {
                style.bg(palette.muted_subtle).text_color(palette.foreground)
            } else {
                style
            }
        })
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label)
        .when(show_dot, |el| {
            el.child(
                div()
                    .size(px(10.0))
                    .rounded_full()
                    .bg(rgba(0x3b82f628))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().size(px(6.0)).rounded_full().bg(rgb(0x3b82f6))),
            )
        })
}
