use crate::PivotApp;
use gpui_kit::*;

pub fn render_settings_view(
    app: &PivotApp,
    _window: &mut Window,
    _cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let port = app.port;
    let db_path = pivot_core::get_db_path();

    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(960.0))
        .mx_auto()
        .gap(px(14.0))
        // 卡片 1：通用偏好
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .rounded(px(8.0))
                .border_1()
                .border_color(rgb(0x2e2e32))
                .bg(rgb(0x1c1c1f))
                .overflow_hidden()
                .child(
                    div()
                        .px(px(16.0))
                        .py(px(10.0))
                        .bg(rgb(0x222226))
                        .border_b_1()
                        .border_color(rgb(0x2e2e32))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xededee))
                        .child("通用偏好设置"),
                )
                // 外观模式
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px(px(16.0))
                        .py(px(12.0))
                        .border_b_1()
                        .border_color(rgb(0x2e2e32))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xededee))
                                .child("界面外观"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .p(px(2.0))
                                .rounded(px(6.0))
                                .bg(rgb(0x27272a))
                                .child(render_setting_chip("深色", true))
                                .child(render_setting_chip("浅色", false))
                                .child(render_setting_chip("跟随系统", false)),
                        ),
                )
                // 语言
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px(px(16.0))
                        .py(px(12.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xededee))
                                .child("界面语言"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .p(px(2.0))
                                .rounded(px(6.0))
                                .bg(rgb(0x27272a))
                                .child(render_setting_chip("简体中文", true))
                                .child(render_setting_chip("English", false)),
                        ),
                ),
        )
        // 卡片 2：常用快捷键速查
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .rounded(px(8.0))
                .border_1()
                .border_color(rgb(0x2e2e32))
                .bg(rgb(0x1c1c1f))
                .overflow_hidden()
                .child(
                    div()
                        .px(px(16.0))
                        .py(px(10.0))
                        .bg(rgb(0x222226))
                        .border_b_1()
                        .border_color(rgb(0x2e2e32))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xededee))
                        .child("常用快捷键"),
                )
                .child(render_shortcut_row("切换主要视图页面", "⌘ 1 ~ 4"))
                .child(render_shortcut_row("快速打开新增服务商", "⌘ N"))
                .child(render_shortcut_row("关闭抽屉或取消返回", "Esc")),
        )
        // 卡片 3：关于 Pivot 与网关信息
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .rounded(px(8.0))
                .border_1()
                .border_color(rgb(0x2e2e32))
                .bg(rgb(0x1c1c1f))
                .p(px(16.0))
                .gap(px(12.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(36.0))
                                        .rounded(px(8.0))
                                        .bg(rgb(0x2563eb))
                                        .text_size(px(18.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xffffff))
                                        .child("P"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(6.0))
                                                .child(
                                                    div()
                                                        .text_size(px(14.0))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(rgb(0xededee))
                                                        .child("Pivot Native (GPUI)"),
                                                )
                                                .child(
                                                    div()
                                                        .px(px(6.0))
                                                        .py(px(1.0))
                                                        .rounded(px(4.0))
                                                        .bg(rgb(0x27272a))
                                                        .text_size(px(10.0))
                                                        .text_color(rgb(0xa1a1aa))
                                                        .child("v0.2.11"),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgb(0x71717a))
                                                .child("高性能、零内存泄漏的本地 AI 网关与模型调度中枢"),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(6.0)).rounded_full().bg(rgb(0x10b981)))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(rgb(0x34d399))
                                        .child("已是最新版本"),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pt(px(10.0))
                        .border_t_1()
                        .border_color(rgb(0x2e2e32))
                        .text_size(px(11.0))
                        .text_color(rgb(0x71717a))
                        .child(format!("本地网关: http://127.0.0.1:{} · 数据库: {}", port, db_path.to_string_lossy()))
                        .child(
                            div()
                                .text_color(rgb(0x60a5fa))
                                .cursor_pointer()
                                .child("github.com/vsnote/pivot ↗"),
                        ),
                ),
        )
}

fn render_setting_chip(label: &'static str, is_active: bool) -> impl IntoElement {
    div()
        .px(px(8.0))
        .py(px(3.0))
        .rounded(px(4.0))
        .text_size(px(11.0))
        .font_weight(if is_active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .bg(if is_active {
            rgb(0x3f3f46)
        } else {
            rgba(0x00000000)
        })
        .text_color(if is_active {
            rgb(0xffffff)
        } else {
            rgb(0xa1a1aa)
        })
        .cursor_pointer()
        .child(label)
}

fn render_shortcut_row(label: &'static str, shortcut: &'static str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .px(px(16.0))
        .py(px(10.0))
        .border_b_1()
        .border_color(rgb(0x2e2e32))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(0xededee))
                .child(label),
        )
        .child(
            div()
                .px(px(8.0))
                .py(px(2.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(rgb(0x3f3f46))
                .bg(rgb(0x27272a))
                .text_size(px(11.0))
                .text_color(rgb(0xd4d4d8))
                .child(shortcut),
        )
}
