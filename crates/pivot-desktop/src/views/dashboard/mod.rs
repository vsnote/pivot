use crate::PivotApp;
use gpui_kit::*;

pub fn render_dashboard_view(
    app: &PivotApp,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let logs = app.logs.clone();
    let total_reqs = app.total_requests;
    let prompt_tokens = app.total_prompt_tokens;
    let completion_tokens = app.total_completion_tokens;
    let total_tokens = prompt_tokens + completion_tokens;

    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(16.0))
        .child(
            // 顶部 4 大统计指标卡片
            div()
                .flex()
                .gap(px(12.0))
                .w_full()
                .child(render_stat_card("总请求数", &total_reqs.to_string(), "次调用"))
                .child(render_stat_card(
                    "输入 Tokens",
                    &format_token_count(prompt_tokens),
                    "Prompt",
                ))
                .child(render_stat_card(
                    "输出 Tokens",
                    &format_token_count(completion_tokens),
                    "Completion",
                ))
                .child(render_stat_card(
                    "消耗 Tokens 总计",
                    &format_token_count(total_tokens),
                    "Total",
                )),
        )
        .child(
            // 日志头部与操作栏
            div()
                .flex()
                .items_center()
                .justify_between()
                .mt(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .child("实时请求日志"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(0x71717a))
                                .child(format!("({} 条记录)", logs.len())),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded(px(6.0))
                                .cursor_pointer()
                                .bg(rgba(0xef444415))
                                .hover(|s| s.bg(rgba(0xef44442a)))
                                .text_size(px(12.0))
                                .text_color(rgb(0xf87171))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.clear_logs(cx);
                                }))
                                .child("清理日志"),
                        ),
                ),
        )
        .child(
            // 日志明细列表
            if logs.is_empty() {
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .h(px(240.0))
                    .w_full()
                    .rounded(px(10.0))
                    .bg(rgb(0x202023))
                    .border_1()
                    .border_color(rgba(0xffffff10))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(0x71717a))
                            .child("暂无请求记录，请向网关端口发送测试请求"),
                    )
            } else {
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .rounded(px(10.0))
                    .bg(rgb(0x202023))
                    .border_1()
                    .border_color(rgba(0xffffff10))
                    .overflow_hidden()
                    .child(
                        // 表头
                        div()
                            .flex()
                            .items_center()
                            .px(px(16.0))
                            .py(px(10.0))
                            .bg(rgba(0xffffff05))
                            .border_b_1()
                            .border_color(rgba(0xffffff0a))
                            .text_size(px(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0x71717a))
                            .child(div().w(px(140.0)).child("时间"))
                            .child(div().w(px(100.0)).child("客户端"))
                            .child(div().w(px(180.0)).child("请求模型"))
                            .child(div().w(px(180.0)).child("路由提供商 / 模型"))
                            .child(div().w(px(80.0)).child("状态"))
                            .child(div().w(px(80.0)).child("耗时"))
                            .child(div().flex_1().child("Tokens (In / Out)")),
                    )
                    .children(logs.iter().take(50).map(|log| {
                        let is_ok = log.status_code >= 200 && log.status_code < 300;
                        let time_str = log.timestamp.split('T').nth(1).unwrap_or(&log.timestamp);
                        let time_display = time_str.split('.').next().unwrap_or(time_str);

                        div()
                            .flex()
                            .items_center()
                            .px(px(16.0))
                            .py(px(10.0))
                            .border_b_1()
                            .border_color(rgba(0xffffff08))
                            .text_size(px(12.0))
                            .hover(|s| s.bg(rgba(0xffffff05)))
                            .child(
                                div()
                                    .w(px(140.0))
                                    .text_color(rgb(0xa1a1aa))
                                    .child(time_display.to_string()),
                            )
                            .child(
                                div()
                                    .w(px(100.0))
                                    .text_color(rgb(0xd4d4d8))
                                    .child(log.client_type.to_uppercase()),
                            )
                            .child(
                                div()
                                    .w(px(180.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xffffff))
                                    .child(log.incoming_model.clone()),
                            )
                            .child(
                                div()
                                    .w(px(180.0))
                                    .text_color(rgb(0x60a5fa))
                                    .child(format!(
                                        "{} / {}",
                                        log.routed_provider_name.as_deref().unwrap_or("none"),
                                        log.routed_model
                                    )),
                            )
                            .child(
                                div()
                                    .w(px(80.0))
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .bg(if is_ok {
                                                rgba(0x22c55e1a)
                                            } else {
                                                rgba(0xef44441a)
                                            })
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if is_ok {
                                                rgb(0x4ade80)
                                            } else {
                                                rgb(0xf87171)
                                            })
                                            .child(log.status_code.to_string()),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(80.0))
                                    .text_color(rgb(0xa1a1aa))
                                    .child(format!("{}ms", log.latency_ms)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(rgb(0xa1a1aa))
                                    .child(format!("{} / {}", log.prompt_tokens, log.completion_tokens)),
                            )
                    }))
            },
        )
}

fn render_stat_card(label: &'static str, value: &str, unit: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .p(px(16.0))
        .rounded(px(10.0))
        .bg(rgb(0x202023))
        .border_1()
        .border_color(rgba(0xffffff10))
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(0x71717a))
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(22.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child(value.to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0xa1a1aa))
                        .child(unit),
                ),
        )
}

fn format_token_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.2}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}
