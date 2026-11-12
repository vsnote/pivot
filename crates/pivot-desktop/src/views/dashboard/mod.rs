use crate::components::button::{render_segmented_tabs, SegmentedTabsSize};
use crate::components::icons::{lucide_icon, render_brand_avatar, render_client_avatar};
use crate::i18n::{client_type_label, Locale};
use crate::pricing::{
    effective_tokens, estimate_cost, format_currency, format_latency, format_tokens_compact,
};
use crate::theme::ThemePalette;
use crate::PivotApp;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pivot_core::db::{RequestLog, StatsBucketPlan};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardSubView {
    Overview,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardTimeRange {
    Hour,
    Today,
    Week,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderMetricMode {
    Calls,
    Cost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardLogFilter {
    All,
    Errors,
}

pub fn compute_time_window(
    range: DashboardTimeRange,
) -> (Option<i64>, Option<StatsBucketPlan>, i64) {
    let now_utc = Utc::now();
    let now_sec = now_utc.timestamp();
    let local_now = now_utc.with_timezone(&Local);
    let today_start_epoch = local_now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|naive| Local.from_local_datetime(&naive).single())
        .map(|dt| dt.with_timezone(&Utc).timestamp())
        .unwrap_or_else(|| now_sec - now_sec.rem_euclid(86_400));

    match range {
        DashboardTimeRange::Hour => {
            let aligned_end = ((now_sec / 300) + 1) * 300;
            let start = aligned_end - 12 * 300;
            (
                Some(now_sec - 3600),
                Some(StatsBucketPlan {
                    start_epoch: start,
                    step_secs: 300,
                    count: 12,
                }),
                today_start_epoch,
            )
        }
        DashboardTimeRange::Today => (
            Some(today_start_epoch),
            Some(StatsBucketPlan {
                start_epoch: today_start_epoch,
                step_secs: 3600,
                count: 24,
            }),
            today_start_epoch,
        ),
        DashboardTimeRange::Week => {
            let week_start = today_start_epoch - 6 * 86_400;
            (
                Some(week_start),
                Some(StatsBucketPlan {
                    start_epoch: week_start,
                    step_secs: 86_400,
                    count: 7,
                }),
                today_start_epoch,
            )
        }
        DashboardTimeRange::All => (None, None, today_start_epoch),
    }
}

pub fn parse_log_utc(ts: &str) -> Option<DateTime<Utc>> {
    let trimmed = ts.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(Utc.from_utc_datetime(&naive));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        return Some(Utc.from_utc_datetime(&naive));
    }
    None
}

fn format_log_short_time(ts: &str, today_start_epoch: i64) -> String {
    let Some(dt_utc) = parse_log_utc(ts) else {
        return ts.chars().take(16).collect();
    };
    let local_dt = dt_utc.with_timezone(&Local);
    if dt_utc.timestamp() >= today_start_epoch {
        local_dt.format("%H:%M:%S").to_string()
    } else {
        local_dt.format("%m-%d %H:%M").to_string()
    }
}

fn format_bucket_hover_label(
    start_epoch: i64,
    end_epoch: i64,
    range: DashboardTimeRange,
) -> String {
    let step = end_epoch - start_epoch;
    let start_local = Utc
        .timestamp_opt(start_epoch, 0)
        .single()
        .map(|dt| dt.with_timezone(&Local));
    let end_local = Utc
        .timestamp_opt((end_epoch - 1).max(start_epoch), 0)
        .single()
        .map(|dt| dt.with_timezone(&Local));

    match (range, start_local, end_local) {
        (DashboardTimeRange::Hour, Some(s), _) => s.format("%H:%M").to_string(),
        (DashboardTimeRange::Today, Some(s), _) => s.format("%H:00").to_string(),
        (_, Some(s), Some(e)) if step > 86_400 => {
            format!("{}~{}", s.format("%m/%d"), e.format("%m/%d"))
        }
        (_, Some(s), _) => s.format("%m-%d").to_string(),
        _ => String::new(),
    }
}

fn format_trend_axes(
    buckets: &[pivot_core::db::TrendBucket],
    range: DashboardTimeRange,
    locale: Locale,
) -> (String, String, String) {
    if buckets.is_empty() {
        return (String::new(), String::new(), String::new());
    }
    match range {
        DashboardTimeRange::Hour => (
            "-60m".to_string(),
            "-30m".to_string(),
            locale.pick("现在", "Now").to_string(),
        ),
        DashboardTimeRange::Today => (
            "00:00".to_string(),
            "12:00".to_string(),
            "23:00".to_string(),
        ),
        DashboardTimeRange::Week | DashboardTimeRange::All => {
            let fmt_day = |epoch: i64| -> String {
                Utc.timestamp_opt(epoch, 0)
                    .single()
                    .map(|dt| dt.with_timezone(&Local).format("%m/%d").to_string())
                    .unwrap_or_default()
            };
            let first = fmt_day(buckets[0].start_epoch);
            let mid = fmt_day(buckets[buckets.len() / 2].start_epoch);
            let right = locale.pick("今天", "Today").to_string();
            (first, mid, right)
        }
    }
}

fn chart_palette(idx: usize) -> Rgba {
    const COLORS: [u32; 6] = [
        0x6366f1, // indigo
        0x10b981, // emerald
        0xf59e0b, // amber
        0x0ea5e9, // sky
        0x8b5cf6, // violet
        0x14b8a6, // teal
    ];
    rgb(COLORS[idx % COLORS.len()])
}

pub fn render_dashboard_view(
    app: &PivotApp,
    palette: ThemePalette,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let locale = app.prefs.locale;
    let sub_view = app.dashboard_sub_view;
    let time_range = app.dashboard_range;
    let stats = &app.dashboard_stats;
    let has_any_logs = stats.has_any_logs || !app.logs.is_empty();

    let app_for_sub = cx.entity();
    let app_for_range = cx.entity();

    // 1. 紧凑单行顶栏：左侧「概览 | 流水」，右侧「1小时 | 今天 | 7天 | 全部」+ 清空
    let top_bar = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(6.0))
        .child(render_segmented_tabs(
            vec![
                (
                    DashboardSubView::Overview,
                    locale.pick("概览", "Overview").to_string(),
                ),
                (
                    DashboardSubView::Logs,
                    locale.pick("流水", "Logs").to_string(),
                ),
            ],
            sub_view,
            SegmentedTabsSize::Sm,
            palette,
            move |next, _, app_cx| {
                app_for_sub.update(app_cx, |this, cx| {
                    this.dashboard_sub_view = next;
                    this.main_scroll_handle.set_offset(point(px(0.0), px(0.0)));
                    cx.notify();
                });
            },
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.0))
                .child(render_segmented_tabs(
                    vec![
                        (
                            DashboardTimeRange::Hour,
                            locale.pick("1小时", "1h").to_string(),
                        ),
                        (
                            DashboardTimeRange::Today,
                            locale.pick("今天", "Today").to_string(),
                        ),
                        (
                            DashboardTimeRange::Week,
                            locale.pick("7天", "7d").to_string(),
                        ),
                        (
                            DashboardTimeRange::All,
                            locale.pick("全部", "All").to_string(),
                        ),
                    ],
                    time_range,
                    SegmentedTabsSize::Sm,
                    palette,
                    move |next, _, app_cx| {
                        app_for_range.update(app_cx, |this, cx| {
                            if this.dashboard_range != next {
                                this.dashboard_range = next;
                                this.hovered_chart_bucket = None;
                                this.expanded_log_id = None;
                                this.refresh_dashboard_data(cx);
                                cx.notify();
                            }
                        });
                    },
                ))
                .child(
                    div()
                        .id("clear_logs_btn")
                        .size(px(26.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(palette.border)
                        .bg(palette.card)
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .when(has_any_logs, |el| {
                            el.cursor_pointer()
                                .hover(move |s| s.bg(palette.destructive_soft_bg))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.prompt_clear_logs(cx);
                                    }),
                                )
                        })
                        .when(!has_any_logs, |el| el.opacity(0.4).cursor_default())
                        .child(lucide_icon(
                            IconName::Trash,
                            12.5,
                            palette.muted_foreground,
                        )),
                ),
        );

    if stats.total_reqs == 0 {
        return div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(top_bar)
            .child(render_empty_state(has_any_logs, locale, palette));
    }

    let body: AnyElement = match sub_view {
        DashboardSubView::Overview => {
            render_overview_body(app, locale, palette, cx).into_any_element()
        }
        DashboardSubView::Logs => render_logs_body(app, locale, palette, cx).into_any_element(),
    };

    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(top_bar)
        .child(body)
}

fn render_empty_state(has_any_logs: bool, locale: Locale, palette: ThemePalette) -> Div {
    div()
        .w_full()
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .px(px(20.0))
        .py(px(48.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(36.0))
                .rounded_full()
                .bg(palette.icon_slot_bg)
                .border_1()
                .border_color(palette.icon_slot_border)
                .flex()
                .items_center()
                .justify_center()
                .child(lucide_icon(
                    IconName::Activity,
                    17.0,
                    palette.muted_foreground,
                )),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.foreground)
                .child(locale.pick("暂无请求统计记录", "No request telemetry yet")),
        )
        .child(
            div()
                .text_size(px(11.5))
                .text_color(palette.muted_foreground)
                .text_center()
                .child(if has_any_logs {
                    locale.pick(
                        "当前时间范围内没有调用记录，可切换至「全部」查看历史统计",
                        "No requests in this time window. Switch to 'All' to view history.",
                    )
                } else {
                    locale.pick(
                        "通过已接管的客户端或通用端点发起调用后，这里将实时汇总流量与排障流水",
                        "Traffic, costs, and request diagnostics will appear here in real time.",
                    )
                }),
        )
}

fn render_overview_body(
    app: &PivotApp,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let stats = &app.dashboard_stats;
    let metric_mode = app.dashboard_metric;
    let time_range = app.dashboard_range;

    let total_reqs = stats.total_reqs;
    let ok_2xx = stats.ok_2xx;
    let err_total = total_reqs.saturating_sub(ok_2xx);
    let success_pct = ((ok_2xx as f64 / total_reqs.max(1) as f64) * 100.0).round() as i32;

    struct ProvAgg {
        name: String,
        protocol: String,
        preset_id: Option<String>,
        calls: usize,
        err_calls: usize,
        cost: f64,
    }
    struct ModelAgg {
        model: String,
        calls: usize,
        ok_calls: usize,
        tokens: u64,
        cost: f64,
    }

    let mut sum_prompt_tokens = 0u64;
    let mut sum_completion_tokens = 0u64;
    let mut sum_cost = 0.0f64;
    let mut prov_map: HashMap<String, ProvAgg> = HashMap::new();
    let mut model_map: HashMap<String, ModelAgg> = HashMap::new();

    for slice in &stats.route_model_slices {
        sum_prompt_tokens += slice.prompt_tokens;
        sum_completion_tokens += slice.completion_tokens;

        let slice_cost = estimate_cost(
            &app.model_rates,
            &slice.model,
            slice.prompt_tokens,
            slice.completion_tokens,
        );
        sum_cost += slice_cost;

        // 仅将真实服务商渠道纳入服务商分发，剔除未匹配路由 ("none")
        if slice.provider_id != "none" && !slice.provider_id.trim().is_empty() {
            let matched_prov = app.providers.iter().find(|p| p.id == slice.provider_id);
            let pname = slice
                .provider_name
                .clone()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| matched_prov.map(|p| p.name.clone()))
                .unwrap_or_else(|| slice.provider_id.clone());
            let proto = matched_prov
                .map(|p| p.protocol.clone())
                .unwrap_or_else(|| "openai".to_string());
            let preset_id = matched_prov.and_then(|p| p.preset_id.clone());

            let p_entry = prov_map
                .entry(slice.provider_id.clone())
                .or_insert(ProvAgg {
                    name: pname,
                    protocol: proto,
                    preset_id,
                    calls: 0,
                    err_calls: 0,
                    cost: 0.0,
                });
            p_entry.calls += slice.calls;
            p_entry.err_calls += slice.err_calls;
            p_entry.cost += slice_cost;
        }

        let m_entry = model_map
            .entry(slice.model.clone())
            .or_insert(ModelAgg {
                model: slice.model.clone(),
                calls: 0,
                ok_calls: 0,
                tokens: 0,
                cost: 0.0,
            });
        m_entry.calls += slice.calls;
        m_entry.ok_calls += slice.ok_calls;
        m_entry.tokens += slice.prompt_tokens + slice.completion_tokens;
        m_entry.cost += slice_cost;
    }

    let total_tokens = sum_prompt_tokens + sum_completion_tokens;
    let avg_ok_latency = if stats.ok_latency_samples > 0 {
        stats.ok_latency_sum_ms as f64 / stats.ok_latency_samples as f64
    } else {
        0.0
    };
    let avg_cost_per_ok = if ok_2xx > 0 {
        sum_cost / ok_2xx as f64
    } else {
        0.0
    };

    // 1. 主卡：4 列核心指标 + 联动双色趋势图
    let kpi_reqs = render_kpi_cell(
        locale.pick("请求", "Requests"),
        total_reqs.to_string(),
        if err_total > 0 {
            format!(
                "{}% · {} {}",
                success_pct,
                err_total,
                locale.pick("异常", "err")
            )
        } else {
            format!("{}% {}", success_pct, locale.pick("成功", "ok"))
        },
        if err_total > 0 {
            palette.amber_text
        } else {
            palette.emerald_text
        },
        false,
        palette,
    );

    let kpi_tokens = render_kpi_cell(
        locale.pick("Token", "Tokens"),
        format_tokens_compact(total_tokens),
        if total_tokens > 0 {
            format!(
                "↑{} ↓{}",
                format_tokens_compact(sum_prompt_tokens),
                format_tokens_compact(sum_completion_tokens)
            )
        } else {
            "—".to_string()
        },
        palette.muted_foreground,
        true,
        palette,
    );

    let kpi_cost = render_kpi_cell(
        locale.pick("预估费用", "Est. Cost"),
        format_currency(sum_cost),
        if sum_cost > 0.0 && ok_2xx > 0 {
            format!(
                "{} {}",
                locale.pick("均", "Avg"),
                format_currency(avg_cost_per_ok)
            )
        } else {
            "—".to_string()
        },
        palette.muted_foreground,
        true,
        palette,
    );

    let kpi_latency = render_kpi_cell(
        locale.pick("延迟", "Latency"),
        if stats.ok_latency_samples > 0 {
            format_latency(avg_ok_latency)
        } else {
            "—".to_string()
        },
        if stats.fallback_count > 0 {
            format!(
                "{} {}x",
                locale.pick("兜底", "Failover"),
                stats.fallback_count
            )
        } else {
            locale.pick("2xx 均值", "2xx avg").to_string()
        },
        palette.muted_foreground,
        true,
        palette,
    );

    let hero_card = div()
        .w_full()
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .w_full()
                .py(px(8.5))
                .px(px(2.0))
                .flex()
                .items_stretch()
                .child(kpi_reqs)
                .child(kpi_tokens)
                .child(kpi_cost)
                .child(kpi_latency),
        )
        .child(render_inline_trend(
            &stats.buckets,
            time_range,
            app.hovered_chart_bucket,
            err_total > 0,
            locale,
            palette,
            cx,
        ));

    // 2. 条件式异常诊断卡（仅在存在异常时渲染，点击任一错误直达异常流水）
    let diagnostics_card: Option<Div> = if err_total > 0 {
        Some(render_diagnostics_card(stats, locale, palette, cx))
    } else {
        None
    };

    // 3. 服务商分发卡
    let mut prov_list: Vec<ProvAgg> = prov_map.into_values().collect();
    match metric_mode {
        ProviderMetricMode::Calls => prov_list.sort_by_key(|a| std::cmp::Reverse(a.calls)),
        ProviderMetricMode::Cost => prov_list.sort_by(|a, b| {
            b.cost
                .partial_cmp(&a.cost)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.calls.cmp(&a.calls))
        }),
    }

    let app_for_metric = cx.entity();
    let routed_calls_sum: usize = prov_list.iter().map(|p| p.calls).sum();
    let total_metric_denom = match metric_mode {
        ProviderMetricMode::Calls => routed_calls_sum.max(1) as f64,
        ProviderMetricMode::Cost => {
            if sum_cost > 0.0 {
                sum_cost
            } else {
                1.0
            }
        }
    };

    let provider_card = {
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.foreground)
                    .child(locale.pick("服务商", "Providers")),
            )
            .child(render_segmented_tabs(
                vec![
                    (
                        ProviderMetricMode::Calls,
                        locale.pick("调用量", "Calls").to_string(),
                    ),
                    (
                        ProviderMetricMode::Cost,
                        locale.pick("费用", "Cost").to_string(),
                    ),
                ],
                metric_mode,
                SegmentedTabsSize::Xs,
                palette,
                move |m, _, app_cx| {
                    app_for_metric.update(app_cx, |this, cx| {
                        this.dashboard_metric = m;
                        cx.notify();
                    });
                },
            ));

        let mut card = div()
            .w_full()
            .p(px(12.0))
            .rounded(px(8.0))
            .bg(palette.card)
            .border_1()
            .border_color(palette.border)
            .flex()
            .flex_col()
            .gap(px(9.0))
            .child(header);

        if prov_list.is_empty() {
            card = card.child(
                div()
                    .py(px(4.0))
                    .text_size(px(11.5))
                    .text_color(palette.muted_foreground)
                    .child(locale.pick(
                        "当前请求均未命中已配置的服务商",
                        "No requests matched a configured provider",
                    )),
            );
        } else {
            let mut prov_bar = div()
                .w_full()
                .h(px(5.0))
                .rounded_full()
                .bg(palette.muted)
                .overflow_hidden()
                .flex()
                .gap(px(1.5));
            for (idx, p) in prov_list.iter().enumerate() {
                let val = match metric_mode {
                    ProviderMetricMode::Calls => p.calls as f64,
                    ProviderMetricMode::Cost => p.cost,
                };
                let ratio = (val / total_metric_denom).clamp(0.0, 1.0) as f32;
                if ratio > 0.0 {
                    prov_bar = prov_bar.child(
                        div()
                            .h_full()
                            .bg(chart_palette(idx))
                            .w(relative(ratio.max(0.02))),
                    );
                }
            }

            let mut prov_rows = div().flex().flex_col().gap(px(6.5));
            for (idx, p) in prov_list.iter().enumerate() {
                let val = match metric_mode {
                    ProviderMetricMode::Calls => p.calls as f64,
                    ProviderMetricMode::Cost => p.cost,
                };
                let pct = if metric_mode == ProviderMetricMode::Cost && sum_cost <= 0.0 {
                    0
                } else {
                    ((val / total_metric_denom) * 100.0).round() as i32
                };
                let right_main = match metric_mode {
                    ProviderMetricMode::Calls => format!("{} {}", p.calls, locale.pick("次", "x")),
                    ProviderMetricMode::Cost => format_currency(p.cost),
                };

                prov_rows = prov_rows.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(7.0))
                                .min_w_0()
                                .flex_1()
                                .when(prov_list.len() > 1, |el| {
                                    el.child(
                                        div()
                                            .size(px(5.5))
                                            .rounded_full()
                                            .flex_shrink_0()
                                            .bg(chart_palette(idx)),
                                    )
                                })
                                .child(render_brand_avatar(
                                    &p.name,
                                    Some(p.protocol.as_str()),
                                    p.preset_id.as_deref(),
                                    18.0,
                                    true,
                                    palette,
                                ))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.foreground)
                                        .truncate()
                                        .child(p.name.clone()),
                                )
                                .when(p.err_calls > 0, |el| {
                                    el.child(
                                        div()
                                            .px(px(4.5))
                                            .py(px(0.5))
                                            .rounded(px(3.5))
                                            .bg(palette.rose_soft_bg)
                                            .text_size(px(9.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(palette.rose_text)
                                            .flex_shrink_0()
                                            .child(format!(
                                                "{} {}",
                                                p.err_calls,
                                                locale.pick("异常", "err")
                                            )),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .flex_shrink_0()
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.foreground)
                                        .child(right_main),
                                )
                                .child(
                                    div()
                                        .w(px(32.0))
                                        .text_right()
                                        .text_size(px(10.5))
                                        .text_color(palette.muted_foreground)
                                        .child(format!("{}%", pct)),
                                ),
                        ),
                );
            }

            card = card.child(prov_bar).child(prov_rows);
        }
        card
    };

    // 4. 模型消耗排行 Top 5（优先展示有有效调用或真实 Token 的模型，避免纯报错空跑霸榜；单色进度条消除撞色）
    let mut all_models: Vec<ModelAgg> = model_map.into_values().collect();
    let has_active_models = all_models.iter().any(|m| m.ok_calls > 0 || m.tokens > 0);
    if has_active_models {
        all_models.retain(|m| m.ok_calls > 0 || m.tokens > 0);
    }
    match metric_mode {
        ProviderMetricMode::Calls => {
            all_models.sort_by(|a, b| {
                b.ok_calls
                    .cmp(&a.ok_calls)
                    .then_with(|| b.tokens.cmp(&a.tokens))
                    .then_with(|| b.calls.cmp(&a.calls))
            });
        }
        ProviderMetricMode::Cost => {
            all_models.sort_by(|a, b| {
                b.cost
                    .partial_cmp(&a.cost)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.tokens.cmp(&a.tokens))
                    .then_with(|| b.ok_calls.cmp(&a.ok_calls))
            });
        }
    }
    all_models.truncate(5);

    let max_model_val = all_models
        .first()
        .map(|m| match metric_mode {
            ProviderMetricMode::Calls => m.calls.max(1) as f64,
            ProviderMetricMode::Cost => {
                if m.cost > 0.0 {
                    m.cost
                } else {
                    m.tokens.max(1) as f64
                }
            }
        })
        .unwrap_or(1.0)
        .max(1e-9);

    let mut model_rows = div().flex().flex_col().gap(px(8.0));
    for (idx, m) in all_models.iter().enumerate() {
        let friendly = app
            .model_display_names
            .get(&m.model)
            .cloned()
            .unwrap_or_else(|| m.model.clone());
        let cur_val = match metric_mode {
            ProviderMetricMode::Calls => m.calls as f64,
            ProviderMetricMode::Cost => {
                if sum_cost > 0.0 {
                    m.cost
                } else {
                    m.tokens as f64
                }
            }
        };
        let ratio = (cur_val / max_model_val).clamp(0.03, 1.0) as f32;

        model_rows = model_rows.child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.5))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .min_w_0()
                                .flex_1()
                                .child(
                                    div()
                                        .w(px(16.0))
                                        .text_size(px(10.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.muted_foreground)
                                        .flex_shrink_0()
                                        .child(format!("{}", idx + 1)),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.foreground)
                                        .truncate()
                                        .child(friendly),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .flex_shrink_0()
                                .text_size(px(10.5))
                                .text_color(palette.muted_foreground)
                                .when(m.tokens > 0, |el| {
                                    el.child(format_tokens_compact(m.tokens)).child("·")
                                })
                                .when(m.cost > 0.0, |el| {
                                    el.child(format_currency(m.cost)).child("·")
                                })
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(palette.foreground)
                                        .child(format!("{}x", m.calls)),
                                ),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(3.0))
                        .rounded_full()
                        .bg(palette.muted)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(palette.blue_text)
                                .opacity(if idx == 0 { 0.9 } else { 0.65 })
                                .w(relative(ratio)),
                        ),
                ),
        );
    }

    let top_models_card = div()
        .w_full()
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .flex()
        .flex_col()
        .gap(px(9.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.foreground)
                .child(locale.pick("模型排行", "Top Models")),
        )
        .child(model_rows);

    // 5. 客户端分布卡
    let mut client_rows = div().flex().flex_col().gap(px(6.5));
    for (cid, cnt) in &stats.client_counts {
        let pct = ((*cnt as f64 / total_reqs.max(1) as f64) * 100.0).round() as i32;
        client_rows = client_rows.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.0))
                        .min_w_0()
                        .flex_1()
                        .child(render_client_avatar(cid, 18.0, true, palette))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .truncate()
                                .child(client_type_label(cid, locale)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .flex_shrink_0()
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .child(format!("{} {}", cnt, locale.pick("次", "x"))),
                        )
                        .child(
                            div()
                                .w(px(32.0))
                                .text_right()
                                .text_size(px(10.5))
                                .text_color(palette.muted_foreground)
                                .child(format!("{}%", pct)),
                        ),
                ),
        );
    }

    let clients_card = div()
        .w_full()
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .flex()
        .flex_col()
        .gap(px(9.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.foreground)
                .child(locale.pick("客户端", "Agents")),
        )
        .child(client_rows);

    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(hero_card)
        .when_some(diagnostics_card, |el, diag| el.child(diag))
        .child(provider_card)
        .child(top_models_card)
        .child(clients_card)
}

fn render_inline_trend(
    buckets: &[pivot_core::db::TrendBucket],
    range: DashboardTimeRange,
    hovered_idx: Option<usize>,
    has_errors: bool,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let max_bucket = buckets
        .iter()
        .map(|b| b.ok_calls + b.err_calls)
        .max()
        .unwrap_or(0)
        .max(1);

    let title = match range {
        DashboardTimeRange::Hour => locale.pick("近 1 小时趋势", "Last 1h Trend"),
        DashboardTimeRange::Today => locale.pick("今日趋势", "Today's Trend"),
        DashboardTimeRange::Week => locale.pick("近 7 天趋势", "7-Day Trend"),
        DashboardTimeRange::All => locale.pick("调用趋势", "Traffic Trend"),
    };

    let hovered_text = hovered_idx.and_then(|slot| {
        let b = buckets.get(slot)?;
        let total = b.ok_calls + b.err_calls;
        let time_lbl = format_bucket_hover_label(b.start_epoch, b.end_epoch, range);
        if b.err_calls > 0 {
            Some(format!(
                "{} · {} {} ({} {})",
                time_lbl,
                total,
                locale.pick("次", "reqs"),
                b.err_calls,
                locale.pick("异常", "err")
            ))
        } else {
            Some(format!(
                "{} · {} {}",
                time_lbl,
                total,
                locale.pick("次", "reqs")
            ))
        }
    });

    let mut bars_row = div()
        .w_full()
        .h(px(50.0))
        .flex()
        .items_end()
        .gap(px(3.0));

    for (slot, b) in buckets.iter().enumerate() {
        let tot = b.ok_calls + b.err_calls;
        let is_hov = hovered_idx == Some(slot);
        let total_h = if tot == 0 {
            3.0
        } else {
            ((tot as f32 / max_bucket as f32) * 46.0).max(5.0)
        };
        let err_h = if b.err_calls == 0 || tot == 0 {
            0.0
        } else {
            ((b.err_calls as f32 / tot as f32) * total_h)
                .max(2.5)
                .min(total_h)
        };
        let ok_h = (total_h - err_h).max(0.0);

        bars_row = bars_row.child(
            div()
                .id(SharedString::from(format!("trend_bar_{}", slot)))
                .flex_1()
                .h_full()
                .flex()
                .items_end()
                .cursor_pointer()
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    if *hovered {
                        this.hovered_chart_bucket = Some(slot);
                    } else if this.hovered_chart_bucket == Some(slot) {
                        this.hovered_chart_bucket = None;
                    }
                    cx.notify();
                }))
                .child(if tot == 0 {
                    div()
                        .w_full()
                        .h(px(3.0))
                        .rounded(px(1.5))
                        .bg(palette.muted)
                } else {
                    div()
                        .w_full()
                        .h(px(total_h))
                        .rounded(px(2.0))
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .opacity(if is_hov { 1.0 } else { 0.85 })
                        .when(err_h > 0.0, |col| {
                            col.child(div().w_full().h(px(err_h)).bg(palette.rose_text))
                        })
                        .when(ok_h > 0.0, |col| {
                            col.child(
                                div()
                                    .w_full()
                                    .h(px(ok_h))
                                    .bg(if is_hov {
                                        palette.primary
                                    } else {
                                        palette.blue_text
                                    }),
                            )
                        })
                }),
        );
    }

    let (ax_left, ax_mid, ax_right) = format_trend_axes(buckets, range, locale);

    div()
        .w_full()
        .px(px(12.0))
        .pt(px(8.5))
        .pb(px(9.0))
        .border_t_1()
        .border_color(palette.border_subtle)
        .flex()
        .flex_col()
        .gap(px(6.5))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(10.5))
                .child(div().text_color(palette.muted_foreground).child(title))
                .child(if let Some(ht) = hovered_text {
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.foreground)
                        .child(ht)
                } else if has_errors {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .text_size(px(10.0))
                        .text_color(palette.muted_foreground)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(3.5))
                                .child(div().size(px(5.0)).rounded_full().bg(palette.blue_text))
                                .child(locale.pick("正常", "OK")),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(3.5))
                                .child(div().size(px(5.0)).rounded_full().bg(palette.rose_text))
                                .child(locale.pick("异常", "Err")),
                        )
                } else {
                    div()
                }),
        )
        .child(bars_row)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(9.5))
                .text_color(palette.subtext)
                .child(ax_left)
                .child(ax_mid)
                .child(ax_right),
        )
}

fn render_diagnostics_card(
    stats: &pivot_core::db::DashboardRawStats,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let denom = stats.total_reqs.max(1) as f32;
    let r_ok = (stats.ok_2xx as f32 / denom).clamp(0.0, 1.0);
    let r_unrouted = (stats.unrouted_reqs as f32 / denom).clamp(0.0, 1.0);
    let r_4xx = (stats.err_4xx as f32 / denom).clamp(0.0, 1.0);
    let r_5xx = (stats.err_5xx as f32 / denom).clamp(0.0, 1.0);

    let e = &stats.errors;
    let mut items: Vec<(&str, usize, bool)> = Vec::new();
    if e.unrouted > 0 {
        items.push((
            locale.pick("未匹配服务商模型", "No Provider Matched Model"),
            e.unrouted,
            false,
        ));
    }
    if e.rate_limited_429 > 0 {
        items.push((
            locale.pick("429 速率或配额受限", "429 Rate / Quota Limited"),
            e.rate_limited_429,
            false,
        ));
    }
    if e.auth_401_403 > 0 {
        items.push((
            locale.pick("401/403 密钥或授权失效", "401/403 Auth Failed"),
            e.auth_401_403,
            true,
        ));
    }
    if e.upstream_timeout_502_504 > 0 {
        items.push((
            locale.pick("502-504 上游连接超时或不可用", "502-504 Upstream Timeout"),
            e.upstream_timeout_502_504,
            true,
        ));
    }
    if e.upstream_500 > 0 {
        items.push((
            locale.pick("500 上游服务内部错误", "500 Upstream Internal Error"),
            e.upstream_500,
            true,
        ));
    }
    if e.not_found_404 > 0 {
        items.push((
            locale.pick("404 上游模型或路径不存在", "404 Endpoint / Model Not Found"),
            e.not_found_404,
            false,
        ));
    }
    if e.bad_request_4xx > 0 {
        items.push((
            locale.pick("400/422 请求参数被上游拒绝", "4xx Invalid Request"),
            e.bad_request_4xx,
            false,
        ));
    }
    if e.other_err > 0 {
        items.push((
            locale.pick("其他异常响应", "Other Upstream Errors"),
            e.other_err,
            true,
        ));
    }
    items.sort_by_key(|x| std::cmp::Reverse(x.1));

    let mut rows = div().flex().flex_col().gap(px(2.0));
    for (idx, (desc, cnt, is_severe)) in items.into_iter().enumerate() {
        rows = rows.child(
            div()
                .id(SharedString::from(format!("diag_err_row_{}", idx)))
                .mx(px(-6.0))
                .px(px(6.0))
                .py(px(3.5))
                .rounded(px(5.0))
                .cursor_pointer()
                .hover(move |s| s.bg(palette.muted_subtle))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.jump_to_error_logs(cx);
                    }),
                )
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(11.0))
                .child(div().text_color(palette.muted_foreground).child(desc.to_string()))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if is_severe {
                            palette.rose_text
                        } else {
                            palette.amber_text
                        })
                        .child(format!("{}x", cnt)),
                ),
        );
    }

    div()
        .w_full()
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.foreground)
                        .child(locale.pick("异常诊断", "Diagnostics")),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.0))
                        .text_size(px(10.5))
                        .child(
                            div()
                                .text_color(palette.emerald_text)
                                .child(format!("2xx {}", stats.ok_2xx)),
                        )
                        .when(stats.unrouted_reqs > 0, |el| {
                            el.child(div().text_color(palette.amber_text).child(format!(
                                "{} {}",
                                locale.pick("未匹配", "Unrouted"),
                                stats.unrouted_reqs
                            )))
                        })
                        .when(stats.err_4xx > 0, |el| {
                            el.child(
                                div()
                                    .text_color(palette.amber_text)
                                    .child(format!("4xx {}", stats.err_4xx)),
                            )
                        })
                        .when(stats.err_5xx > 0, |el| {
                            el.child(
                                div()
                                    .text_color(palette.rose_text)
                                    .child(format!("5xx {}", stats.err_5xx)),
                            )
                        }),
                ),
        )
        .child(
            div()
                .w_full()
                .h(px(4.5))
                .rounded_full()
                .bg(palette.muted)
                .overflow_hidden()
                .flex()
                .gap(px(1.0))
                .when(r_ok > 0.0, |b| {
                    b.child(div().h_full().bg(palette.emerald_text).w(relative(r_ok)))
                })
                .when(r_unrouted > 0.0, |b| {
                    b.child(
                        div()
                            .h_full()
                            .bg(palette.amber_text)
                            .opacity(0.65)
                            .w(relative(r_unrouted)),
                    )
                })
                .when(r_4xx > 0.0, |b| {
                    b.child(div().h_full().bg(palette.amber_text).w(relative(r_4xx)))
                })
                .when(r_5xx > 0.0, |b| {
                    b.child(div().h_full().bg(palette.rose_text).w(relative(r_5xx)))
                }),
        )
        .child(rows)
}

fn render_logs_body(
    app: &PivotApp,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let stats = &app.dashboard_stats;
    let err_total = stats.total_reqs.saturating_sub(stats.ok_2xx);
    let filter = app.dashboard_log_filter;
    let (_, _, today_start_epoch) = compute_time_window(app.dashboard_range);

    let app_for_filter = cx.entity();
    let filter_bar = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(render_segmented_tabs(
            vec![
                (
                    DashboardLogFilter::All,
                    format!("{} ({})", locale.pick("全部", "All"), stats.total_reqs),
                ),
                (
                    DashboardLogFilter::Errors,
                    format!("{} ({})", locale.pick("仅异常", "Errors"), err_total),
                ),
            ],
            filter,
            SegmentedTabsSize::Xs,
            palette,
            move |next, _, app_cx| {
                app_for_filter.update(app_cx, |this, cx| {
                    if this.dashboard_log_filter != next {
                        this.dashboard_log_filter = next;
                        this.expanded_log_id = None;
                        this.refresh_dashboard_data(cx);
                        cx.notify();
                    }
                });
            },
        ))
        .when(app.logs.len() >= 200, |el| {
            el.child(
                div()
                    .text_size(px(10.5))
                    .text_color(palette.muted_foreground)
                    .child(locale.pick("最近 200 条", "Latest 200")),
            )
        });

    if app.logs.is_empty() {
        return div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(filter_bar)
            .child(
                div()
                    .w_full()
                    .rounded(px(8.0))
                    .bg(palette.card)
                    .border_1()
                    .border_color(palette.border)
                    .py(px(36.0))
                    .px(px(16.0))
                    .text_center()
                    .text_size(px(12.0))
                    .text_color(palette.muted_foreground)
                    .child(locale.pick("当前筛选条件下无异常记录", "No matching log entries")),
            );
    }

    let mut list_card = div()
        .w_full()
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .overflow_hidden()
        .flex()
        .flex_col();

    for (idx, log) in app.logs.iter().enumerate() {
        let is_expanded = app.expanded_log_id.as_deref() == Some(log.id.as_str());
        list_card = list_card.child(render_log_item(
            app,
            log,
            idx > 0,
            is_expanded,
            today_start_epoch,
            locale,
            palette,
            cx,
        ));
    }

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(filter_bar)
        .child(list_card)
}

#[allow(clippy::too_many_arguments)]
fn render_log_item(
    app: &PivotApp,
    log: &RequestLog,
    has_top_border: bool,
    is_expanded: bool,
    today_start_epoch: i64,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let log_id = log.id.clone();
    let is_ok = (200..300).contains(&log.status_code);
    let is_unrouted = !is_ok
        && (log.routed_provider_id == "none" || log.routed_provider_id.trim().is_empty());

    let (badge_text, badge_fg, badge_bg) = if is_ok {
        (
            log.status_code.to_string(),
            palette.emerald_text,
            palette.emerald_soft_bg,
        )
    } else if is_unrouted {
        (
            locale.pick("未匹配", "Unrouted").to_string(),
            palette.amber_text,
            palette.amber_soft_bg,
        )
    } else if (400..500).contains(&log.status_code) {
        (
            log.status_code.to_string(),
            palette.amber_text,
            palette.amber_soft_bg,
        )
    } else {
        (
            log.status_code.to_string(),
            palette.rose_text,
            palette.rose_soft_bg,
        )
    };

    let raw_model = if !log.routed_model.trim().is_empty() {
        log.routed_model.trim()
    } else if !log.incoming_model.trim().is_empty() {
        log.incoming_model.trim()
    } else {
        "unknown"
    };
    let display_model = app
        .model_display_names
        .get(raw_model)
        .cloned()
        .unwrap_or_else(|| raw_model.to_string());

    let prov_label = if is_unrouted {
        None
    } else {
        log.routed_provider_name
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                app.providers
                    .iter()
                    .find(|p| p.id == log.routed_provider_id)
                    .map(|p| p.name.clone())
            })
    };

    let summary_row = div()
        .id(SharedString::from(format!("log_row_{}", log.id)))
        .px(px(11.0))
        .py(px(8.0))
        .cursor_pointer()
        .hover(move |s| s.bg(palette.muted_subtle))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                if this.expanded_log_id.as_deref() == Some(log_id.as_str()) {
                    this.expanded_log_id = None;
                } else {
                    this.expanded_log_id = Some(log_id.clone());
                }
                cx.notify();
            }),
        )
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.5))
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .px(px(5.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .bg(badge_bg)
                        .text_size(px(10.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(badge_fg)
                        .flex_shrink_0()
                        .child(badge_text),
                )
                .child(render_client_avatar(&log.client_type, 16.0, true, palette))
                .child(
                    div()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.foreground)
                        .truncate()
                        .child(display_model),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.5))
                .flex_shrink_0()
                .text_size(px(10.5))
                .text_color(palette.muted_foreground)
                .when_some(prov_label.clone(), |el, pname| {
                    el.child(div().max_w(px(74.0)).truncate().child(pname))
                })
                .child(format_latency(log.latency_ms.max(0) as f64))
                .child(
                    div()
                        .text_color(palette.subtext)
                        .child(format_log_short_time(&log.timestamp, today_start_epoch)),
                )
                .child(lucide_icon(
                    if is_expanded {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    },
                    11.5,
                    palette.subtext,
                )),
        );

    let mut wrapper = div()
        .flex()
        .flex_col()
        .when(has_top_border, |el| el.border_t_1().border_color(palette.border))
        .child(summary_row);

    if is_expanded {
        let (p_tok, c_tok, tot_tok) = effective_tokens(log);
        let cost = estimate_cost(&app.model_rates, raw_model, p_tok, c_tok);
        let model_trace = if !log.routed_model.trim().is_empty()
            && !log.incoming_model.trim().is_empty()
            && log.incoming_model.trim() != log.routed_model.trim()
        {
            format!("{} → {}", log.incoming_model.trim(), log.routed_model.trim())
        } else {
            raw_model.to_string()
        };

        let mut detail = div()
            .px(px(12.0))
            .py(px(9.0))
            .bg(palette.muted_subtle)
            .border_t_1()
            .border_color(palette.border_subtle)
            .flex()
            .flex_col()
            .gap(px(6.5))
            .text_size(px(10.5));

        // 元数据行：客户端 + 实际模型/接口 + Token 与费用
        detail = detail.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .flex_wrap()
                .gap(px(6.0))
                .text_color(palette.muted_foreground)
                .child(format!(
                    "{} · {} · {}",
                    client_type_label(&log.client_type, locale),
                    model_trace,
                    log.path
                ))
                .when(tot_tok > 0, |el| {
                    el.child(
                        div()
                            .text_color(palette.foreground)
                            .child(format!(
                                "↑{} ↓{} · {}",
                                format_tokens_compact(p_tok),
                                format_tokens_compact(c_tok),
                                format_currency(cost)
                            )),
                    )
                }),
        );

        // 重试轨迹链（当存在多次尝试或包含上游失败记录时展示）
        if log.attempts.len() > 1 || log.attempts.iter().any(|a| !(200..300).contains(&a.status)) {
            let mut chain = div().flex().flex_col().gap(px(3.5));
            for (a_idx, att) in log.attempts.iter().enumerate() {
                let att_ok = (200..300).contains(&att.status);
                chain = chain.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.5))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_color(palette.foreground)
                                        .truncate()
                                        .child(format!(
                                            "#{} {} ({})",
                                            a_idx + 1,
                                            att.provider_name,
                                            att.model
                                        )),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_color(if att_ok {
                                            palette.emerald_text
                                        } else {
                                            palette.rose_text
                                        })
                                        .child(format!(
                                            "{} · {}",
                                            att.status,
                                            format_latency(att.latency_ms.max(0) as f64)
                                        )),
                                ),
                        )
                        .when_some(
                            att.error.as_ref().filter(|s| !s.trim().is_empty()),
                            |el, err_txt| {
                                el.child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(palette.muted_foreground)
                                        .child(err_txt.trim().to_string()),
                                )
                            },
                        ),
                );
            }
            detail = detail.child(chain);
        }

        // 错误信息框 + 一键复制
        if let Some(raw_err) = log.error_message.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty())
        {
            let copy_key = format!("log_err_{}", log.id);
            let is_copied = app.copied_id.as_deref() == Some(copy_key.as_str());
            let full_err = raw_err.to_string();
            let preview_err: String = if raw_err.chars().count() > 360 {
                let head: String = raw_err.chars().take(360).collect();
                format!("{}…", head)
            } else {
                full_err.clone()
            };

            detail = detail.child(
                div()
                    .p(px(8.0))
                    .rounded(px(5.0))
                    .bg(palette.rose_soft_bg)
                    .border_1()
                    .border_color(palette.rose_soft_border)
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(10.5))
                            .text_color(palette.rose_text)
                            .child(preview_err),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("copy_err_btn_{}", log.id)))
                            .size(px(20.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .cursor_pointer()
                            .hover(move |s| s.bg(palette.card_hover))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.copy_to_clipboard(copy_key.clone(), full_err.clone(), cx);
                                }),
                            )
                            .child(if is_copied {
                                lucide_icon(IconName::Check, 11.5, palette.emerald_text)
                            } else {
                                lucide_icon(IconName::Copy, 11.5, palette.rose_text)
                            }),
                    ),
            );
        }

        wrapper = wrapper.child(detail);
    }

    wrapper
}

fn render_kpi_cell(
    label: &str,
    value: String,
    sub: String,
    sub_color: Rgba,
    has_left_border: bool,
    palette: ThemePalette,
) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .px(px(9.0))
        .py(px(1.5))
        .when(has_left_border, |el| {
            el.border_l_1().border_color(palette.border_subtle)
        })
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(10.0))
                .text_color(palette.muted_foreground)
                .truncate()
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.foreground)
                .truncate()
                .child(value),
        )
        .child(
            div()
                .text_size(px(9.5))
                .text_color(sub_color)
                .truncate()
                .child(sub),
        )
}
