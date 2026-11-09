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
        .max_w(px(720.0))
        .mx_auto()
        .gap(px(16.0))
        .child(
            // 网关配置卡片
            div()
                .flex()
                .flex_col()
                .p(px(20.0))
                .rounded(px(10.0))
                .bg(rgb(0x202023))
                .border_1()
                .border_color(rgba(0xffffff10))
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child("网关与服务配置"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .py(px(8.0))
                        .border_b_1()
                        .border_color(rgba(0xffffff08))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(0xd4d4d8))
                                        .child("本地监听端口"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(rgb(0x71717a))
                                        .child("提供 OpenAI & Anthropic 格式的代理服务"),
                                ),
                        )
                        .child(
                            div()
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded(px(6.0))
                                .bg(rgba(0xffffff0a))
                                .border_1()
                                .border_color(rgba(0xffffff10))
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0x60a5fa))
                                .child(format!("http://127.0.0.1:{}", port)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .py(px(8.0))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(0xd4d4d8))
                                        .child("数据存储路径"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(rgb(0x71717a))
                                        .child(db_path.to_string_lossy().to_string()),
                                ),
                        ),
                ),
        )
        .child(
            // 关于与架构说明卡片
            div()
                .flex()
                .flex_col()
                .p(px(20.0))
                .rounded(px(10.0))
                .bg(rgb(0x202023))
                .border_1()
                .border_color(rgba(0xffffff10))
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child("关于 Pivot"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(0xa1a1aa))
                        .child("Pivot 是新一代纯 Rust + GPUI GPU 硬件加速的高性能 AI 网关与模型调度工具。"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(16.0))
                        .pt(px(6.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(0x71717a))
                                .child("版本: v0.2.11 (GPUI Native Edition)"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(0x71717a))
                                .child("渲染引擎: GPUI (Metal / Vulkan / DirectX)"),
                        ),
                ),
        )
}
