use crate::PivotApp;
use gpui_kit::*;
use pivot_core::db::RequestLog;
use std::collections::HashMap;

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

    // 统计延迟与错误
    let mut err_count = 0u64;
    let mut total_latency = 0i64;
    let mut provider_counts: HashMap<String, u64> = HashMap::new();

    for l in &logs {
        if l.status_code >= 400 || l.error_message.is_some() {
            err_count += 1;
        }
        total_latency += l.latency_ms;
        let p_name = l
            .routed_provider_name
            .clone()
            .unwrap_or_else(|| "默认".to_string());
        *provider_counts.entry(p_name).or_insert(0) += 1;
    }

    let avg_latency = if total_reqs > 0 {
        total_latency / total_reqs as i64
    } else {
        0
    };

    let success_rate = if total_reqs > 0 {
        ((total_reqs - err_count) as f64 / total_reqs as f64) * 100.0
    } else {
        100.0
    };

    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(1080.0))
        .mx_auto()
        .gap(px(14.0))
        // 顶部精简状态行与清空按钮
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .w_full()
                .px(px(4.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(6.0)).rounded_full().bg(rgb(0x10b981)))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(0xededee))
                                        .child(format!(":{} · 网关运行中", app.port)),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(4.0))
                                .rounded(px(5.0))
                                .cursor_pointer()
                                .bg(rgb(0x27272a))
                                .hover(|s| s.bg(rgba(0xef444420)).text_color(rgb(0xf87171)))
                                .text_size(px(11.0))
                                .text_color(rgb(0xa1a1aa))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.clear_logs(cx);
                                }))
                                .child("清空日志"),
                        ),
                ),
        )
        // 1. 原版 4 核心指标单行 4 等分集成条
        .child(
            div()
                .flex()
                .w_full()
                .rounded(px(8.0))
                .border_1()
                .border_color(rgb(0x2e2e32))
                .bg(rgb(0x1c1c1f))
                .p(px(12.0))
                .child(
                    // 单元 1: 总请求
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .pr(px(12.0))
                        .border_r_1()
                        .border_color(rgb(0x2e2e32))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xa1a1aa))
                                .child("总请求"),
                        )
                        .child(
                            div()
                                .text_size(px(18.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xededee))
                                .child(total_reqs.to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(if err_count > 0 {
                                    rgb(0xf87171)
                                } else {
                                    rgb(0x34d399)
                                })
                                .child(format!("{:.0}% 成功", success_rate)),
                        ),
                )
                .child(
                    // 单元 2: 总 Token
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .px(px(12.0))
                        .border_r_1()
                        .border_color(rgb(0x2e2e32))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xa1a1aa))
                                .child("总 Token"),
                        )
                        .child(
                            div()
                                .text_size(px(18.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xededee))
                                .child(format_token_count(total_tokens)),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(0x71717a))
                                .child(format!("↑{} · ↓{}", format_compact_token(prompt_tokens), format_compact_token(completion_tokens))),
                        ),
                )
                .child(
                    // 单元 3: 预估花费
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .px(px(12.0))
                        .border_r_1()
                        .border_color(rgb(0x2e2e32))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xa1a1aa))
                                .child("预估费用"),
                        )
                        .child(
                            div()
                                .text_size(px(18.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xededee))
                                .child("$0.00"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(0x71717a))
                                .child("均 $0.00"),
                        ),
                )
                .child(
                    // 单元 4: 平均延迟
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .pl(px(12.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xa1a1aa))
                                .child("平均延迟"),
                        )
                        .child(
                            div()
                                .text_size(px(18.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xededee))
                                .child(format!("{}ms", avg_latency)),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(0xfbbf24))
                                .child("⚡ 100% 流式"),
                        ),
                ),
        )
        // 2. 服务商分发占比与排行
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .rounded(px(8.0))
                .border_1()
                .border_color(rgb(0x2e2e32))
                .bg(rgb(0x1c1c1f))
                .p(px(14.0))
                .gap(px(10.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xededee))
                        .child("服务商分发占比"),
                )
                .child(
                    // 彩色全宽比例条
                    div()
                        .flex()
                        .w_full()
                        .h(px(6.0))
                        .rounded(px(3.0))
                        .bg(rgb(0x27272a))
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(if total_reqs > 0 { px(1000.0) } else { px(0.0) })
                                .bg(rgb(0x3b82f6)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .children(provider_counts.into_iter().map(|(p, count)| {
                            let share = if total_reqs > 0 {
                                (count as f64 / total_reqs as f64) * 100.0
                            } else {
                                0.0
                            };
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(6.0)).rounded_full().bg(rgb(0x3b82f6)))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(rgb(0xededee))
                                        .child(format!("{} · {}次 ({:.0}%)", p, count, share)),
                                )
                        })),
                ),
        )
        // 3. 实时请求日志流 (原版极简表格)
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
                    // 表头
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .px(px(14.0))
                        .py(px(8.0))
                        .bg(rgb(0x222226))
                        .border_b_1()
                        .border_color(rgb(0x2e2e32))
                        .text_size(px(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0xa1a1aa))
                        .child("实时请求日志明细"),
                )
                .child(if logs.is_empty() {
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .py(px(30.0))
                        .text_size(px(12.0))
                        .text_color(rgb(0x71717a))
                        .child("暂无请求记录")
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .children(logs.iter().take(50).map(render_log_table_row))
                }),
        )
}

fn render_log_table_row(log: &RequestLog) -> impl IntoElement {
    let is_ok = log.status_code >= 200 && log.status_code < 300;
    let model = if !log.routed_model.is_empty() {
        log.routed_model.as_str()
    } else if !log.incoming_model.is_empty() {
        log.incoming_model.as_str()
    } else {
        "unknown"
    };
    let provider = log.routed_provider_name.as_deref().unwrap_or("unknown");
    let total_tokens = log.prompt_tokens.max(0) + log.completion_tokens.max(0);

    // 截取时间
    let time_str = if log.timestamp.len() >= 19 {
        &log.timestamp[11..19]
    } else {
        &log.timestamp
    };

    div()
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .h(px(38.0))
        .px(px(14.0))
        .border_b_1()
        .border_color(rgb(0x242428))
        .hover(|s| s.bg(rgb(0x222226)))
        .child(
            // 左侧：状态码胶囊 + 时间 + 模型与服务商
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .px(px(6.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .bg(if is_ok {
                            rgba(0x10b98120)
                        } else {
                            rgba(0xef444420)
                        })
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_ok {
                            rgb(0x34d399)
                        } else {
                            rgb(0xf87171)
                        })
                        .child(log.status_code.to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0x71717a))
                        .child(time_str.to_string()),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0xededee))
                        .child(model.to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0x71717a))
                        .child(format!("({})", provider)),
                ),
        )
        .child(
            // 右侧：延迟 + Token 数
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0xa1a1aa))
                        .child(format!("{}ms", log.latency_ms)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0x60a5fa))
                        .child(format!("{} tok", total_tokens)),
                ),
        )
}

fn format_token_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_compact_token(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.0}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
