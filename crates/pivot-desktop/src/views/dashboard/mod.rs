use crate::components::button::{render_segmented_tabs, SegmentedTabsSize};
use crate::components::icons::{lucide_icon, render_brand_avatar, render_client_avatar};
use crate::i18n::client_type_label;
use crate::pricing::{
    effective_tokens, estimate_cost, format_currency, format_latency, format_tokens, format_uptime,
};
use crate::theme::ThemePalette;
use crate::PivotApp;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Timelike, Utc};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pivot_core::db::RequestLog;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardTimeRange {
    All,
    Today,
    Hour,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderMetricMode {
    Calls,
    Cost,
}

fn parse_log_utc(ts: &str) -> Option<DateTime<Utc>> {
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

fn chart_palette(idx: usize) -> Rgba {
    const COLORS: [u32; 7] = [
        0x6366f1, // indigo
        0x10b981, // emerald
        0xf59e0b, // amber
        0x0ea5e9, // sky
        0x8b5cf6, // violet
        0xf43f5e, // rose
        0x14b8a6, // teal
    ];
    rgb(COLORS[idx % COLORS.len()])
}

pub fn render_dashboard_view(
    app: &PivotApp,
    palette: ThemePalette,
    window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let locale = app.prefs.locale;
    let time_range = app.dashboard_range;
    let metric_mode = app.dashboard_metric;
    let is_wide = f32::from(window.viewport_size().width) >= 560.0;

    let now_utc = Utc::now();
    let local_now = now_utc.with_timezone(&Local);
    let today_start_utc = local_now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|naive| Local.from_local_datetime(&naive).single())
        .map(|dt| dt.with_timezone(&Utc));

    let filtered_logs: Vec<&RequestLog> = app
        .logs
        .iter()
        .filter(|log| match time_range {
            DashboardTimeRange::All => true,
            DashboardTimeRange::Hour => parse_log_utc(&log.timestamp)
                .map(|dt| (now_utc - dt).num_seconds() <= 3600)
                .unwrap_or(false),
            DashboardTimeRange::Today => match (parse_log_utc(&log.timestamp), today_start_utc) {
                (Some(dt), Some(start)) => dt >= start,
                _ => false,
            },
        })
        .collect();

    let app_for_range = cx.entity();
    let has_any_logs = !app.logs.is_empty();

    // 1. 顶部网关状态胶囊 + 时间筛选 + 清空按钮
    let top_bar = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .flex_wrap()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .h(px(26.0))
                .px(px(9.0))
                .rounded(px(6.0))
                .bg(palette.card)
                .border_1()
                .border_color(palette.border)
                .child(
                    div()
                        .size(px(6.0))
                        .rounded_full()
                        .bg(if app.gateway_status.is_running {
                            palette.emerald_text
                        } else {
                            palette.rose_text
                        }),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.foreground)
                        .child(format!("127.0.0.1:{}", app.gateway_status.port)),
                )
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(palette.muted_foreground)
                        .child(format!(
                            "· {}",
                            format_uptime(app.gateway_status.uptime_secs, locale)
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(render_segmented_tabs(
                    vec![
                        (
                            DashboardTimeRange::All,
                            locale.pick("全部", "All").to_string(),
                        ),
                        (
                            DashboardTimeRange::Today,
                            locale.pick("今天", "Today").to_string(),
                        ),
                        (
                            DashboardTimeRange::Hour,
                            locale.pick("1 小时", "1h").to_string(),
                        ),
                    ],
                    time_range,
                    SegmentedTabsSize::Sm,
                    palette,
                    move |next, _, app_cx| {
                        app_for_range.update(app_cx, |this, cx| {
                            this.dashboard_range = next;
                            cx.notify();
                        });
                    },
                ))
                .child(div().w(px(1.0)).h(px(16.0)).bg(palette.border))
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

    if filtered_logs.is_empty() {
        return div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(top_bar)
            .child(
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
                                    "通过已接管的客户端或通用端点发起模型调用后，这里将实时汇总流量与费用",
                                    "Traffic, tokens, latency, and estimated costs will appear here in real time.",
                                )
                            }),
                    ),
            );
    }

    // 2. 聚合核心指标
    let total_reqs = filtered_logs.len();
    let mut ok_2xx = 0usize;
    let mut err_4xx = 0usize;
    let mut err_5xx = 0usize;
    let mut stream_count = 0usize;
    let mut fallback_count = 0usize;
    let mut sum_prompt_tokens = 0u64;
    let mut sum_completion_tokens = 0u64;
    let mut sum_cost = 0.0f64;
    let mut sum_latency_ms = 0i64;
    let mut latency_samples = 0usize;

    struct ProvAgg {
        name: String,
        protocol: String,
        preset_id: Option<String>,
        calls: usize,
        cost: f64,
    }
    struct ModelAgg {
        model: String,
        provider_hint: Option<String>,
        calls: usize,
        tokens: u64,
        cost: f64,
    }

    let mut prov_map: HashMap<String, ProvAgg> = HashMap::new();
    let mut model_map: HashMap<String, ModelAgg> = HashMap::new();
    let mut client_map: HashMap<String, usize> = HashMap::new();
    let mut err_reasons: HashMap<&'static str, usize> = HashMap::new();
    let mut hourly_buckets = [0usize; 24];

    for log in &filtered_logs {
        if (200..300).contains(&log.status_code) {
            ok_2xx += 1;
        } else if (400..500).contains(&log.status_code) {
            err_4xx += 1;
        } else {
            err_5xx += 1;
        }

        if !(200..300).contains(&log.status_code) {
            let reason_key = match log.status_code {
                429 => "429",
                401 | 403 => "auth",
                404 => "404",
                502..=504 => "upstream_timeout",
                500 => "500",
                _ => "other",
            };
            *err_reasons.entry(reason_key).or_insert(0) += 1;
        }

        if log.is_stream {
            stream_count += 1;
        }
        if log.fallback_used {
            fallback_count += 1;
        }
        if log.latency_ms > 0 {
            sum_latency_ms += log.latency_ms;
            latency_samples += 1;
        }

        let (p_tok, c_tok, tot_tok) = effective_tokens(log);
        sum_prompt_tokens += p_tok;
        sum_completion_tokens += c_tok;

        let model_for_rate = if !log.routed_model.trim().is_empty() {
            &log.routed_model
        } else {
            &log.incoming_model
        };
        let req_cost = estimate_cost(&app.model_rates, model_for_rate, p_tok, c_tok);
        sum_cost += req_cost;

        // 服务商聚合
        let pid = if log.routed_provider_id.trim().is_empty() {
            "unrouted".to_string()
        } else {
            log.routed_provider_id.clone()
        };
        let matched_prov = app.providers.iter().find(|p| p.id == pid);
        let pname = log
            .routed_provider_name
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| matched_prov.map(|p| p.name.clone()))
            .unwrap_or_else(|| locale.pick("未匹配服务商", "Unrouted").to_string());
        let proto = matched_prov
            .map(|p| p.protocol.clone())
            .unwrap_or_else(|| "openai".to_string());
        let preset_id = matched_prov.and_then(|p| p.preset_id.clone());

        let p_entry = prov_map.entry(pid).or_insert(ProvAgg {
            name: pname.clone(),
            protocol: proto,
            preset_id,
            calls: 0,
            cost: 0.0,
        });
        p_entry.calls += 1;
        p_entry.cost += req_cost;

        // 模型聚合
        let m_name = if !log.routed_model.trim().is_empty() {
            log.routed_model.trim().to_string()
        } else if !log.incoming_model.trim().is_empty() {
            log.incoming_model.trim().to_string()
        } else {
            "unknown".to_string()
        };
        let m_entry = model_map.entry(m_name.clone()).or_insert(ModelAgg {
            model: m_name,
            provider_hint: Some(pname),
            calls: 0,
            tokens: 0,
            cost: 0.0,
        });
        m_entry.calls += 1;
        m_entry.tokens += tot_tok;
        m_entry.cost += req_cost;

        // 客户端聚合
        let c_key = if log.client_type.trim().is_empty() {
            "unknown".to_string()
        } else {
            log.client_type.trim().to_string()
        };
        *client_map.entry(c_key).or_insert(0) += 1;

        // 最近 24 小时分桶（bucket 23 为当前小时，bucket 0 为 23 小时前）
        if let Some(dt) = parse_log_utc(&log.timestamp) {
            let diff_hours = (now_utc.timestamp() - dt.timestamp()) / 3600;
            if (0..24).contains(&diff_hours) {
                let slot = (23 - diff_hours) as usize;
                hourly_buckets[slot] += 1;
            }
        }
    }

    let total_tokens = sum_prompt_tokens + sum_completion_tokens;
    let err_total = total_reqs.saturating_sub(ok_2xx);
    let success_pct = ((ok_2xx as f64 / total_reqs as f64) * 100.0).round() as i32;
    let stream_pct = ((stream_count as f64 / total_reqs as f64) * 100.0).round() as i32;
    let fallback_pct = ((fallback_count as f64 / total_reqs as f64) * 100.0).round() as i32;
    let avg_latency = if latency_samples > 0 {
        sum_latency_ms as f64 / latency_samples as f64
    } else {
        0.0
    };
    let avg_cost = sum_cost / total_reqs as f64;

    // 排序各维度列表
    let mut prov_list: Vec<ProvAgg> = prov_map.into_values().collect();
    match metric_mode {
        ProviderMetricMode::Calls => prov_list.sort_by(|a, b| b.calls.cmp(&a.calls)),
        ProviderMetricMode::Cost => prov_list.sort_by(|a, b| {
            b.cost
                .partial_cmp(&a.cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
    }

    let mut model_list: Vec<ModelAgg> = model_map.into_values().collect();
    model_list.sort_by(|a, b| b.calls.cmp(&a.calls));
    model_list.truncate(5);

    let mut client_list: Vec<(String, usize)> = client_map.into_iter().collect();
    client_list.sort_by(|a, b| b.1.cmp(&a.1));

    let mut err_list: Vec<(&'static str, usize)> = err_reasons.into_iter().collect();
    err_list.sort_by(|a, b| b.1.cmp(&a.1));
    err_list.truncate(3);

    // 3. 组装顶部单行紧凑 4 指标集成卡
    let kpi_reqs = render_kpi_cell(
        locale.pick("总请求", "Requests"),
        total_reqs.to_string(),
        if err_total > 0 {
            format!(
                "{}% · {} {}",
                success_pct,
                err_total,
                locale.pick("失败", "err")
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
        locale.pick("总 Token", "Tokens"),
        format_tokens(total_tokens).replace(' ', ""),
        format!(
            "↑{} · ↓{}",
            format_tokens(sum_prompt_tokens).replace(' ', ""),
            format_tokens(sum_completion_tokens).replace(' ', "")
        ),
        palette.muted_foreground,
        true,
        palette,
    );
    let kpi_cost = render_kpi_cell(
        locale.pick("预估费用", "Est. Cost"),
        format_currency(sum_cost),
        format!("{} {}", locale.pick("均", "Avg"), format_currency(avg_cost)),
        palette.muted_foreground,
        true,
        palette,
    );
    let kpi_latency = render_kpi_cell(
        locale.pick("平均延迟", "Latency"),
        format_latency(avg_latency),
        format!("{}% {}", stream_pct, locale.pick("流式", "stream")),
        palette.muted_foreground,
        true,
        palette,
    );

    let kpi_section = div()
        .w_full()
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .py(px(7.5))
        .px(px(2.0))
        .flex()
        .items_stretch()
        .child(kpi_reqs)
        .child(kpi_tokens)
        .child(kpi_cost)
        .child(kpi_latency);

    // 4. 服务商分发卡
    let app_for_metric = cx.entity();
    let total_metric_denom = match metric_mode {
        ProviderMetricMode::Calls => total_reqs.max(1) as f64,
        ProviderMetricMode::Cost => {
            if sum_cost > 0.0 {
                sum_cost
            } else {
                1.0
            }
        }
    };

    let mut prov_bar = div()
        .w_full()
        .h(px(6.0))
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

    let mut prov_rows = div().flex().flex_col().gap(px(6.0));
    for (idx, p) in prov_list.iter().enumerate() {
        let val = match metric_mode {
            ProviderMetricMode::Calls => p.calls as f64,
            ProviderMetricMode::Cost => p.cost,
        };
        let pct = ((val / total_metric_denom) * 100.0).round() as i32;
        let right_main = match metric_mode {
            ProviderMetricMode::Calls => format!("{} {}", p.calls, locale.pick("次", "reqs")),
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
                        .child(
                            div()
                                .size(px(6.0))
                                .rounded_full()
                                .flex_shrink_0()
                                .bg(chart_palette(idx)),
                        )
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
                                .child(right_main),
                        )
                        .child(
                            div()
                                .w(px(34.0))
                                .text_right()
                                .text_size(px(10.5))
                                .text_color(palette.muted_foreground)
                                .child(format!("{}%", pct)),
                        ),
                ),
        );
    }

    let provider_card = div()
        .w_full()
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.foreground)
                        .child(locale.pick("服务商分发", "Provider Share")),
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
                )),
        )
        .child(prov_bar)
        .child(prov_rows);

    // 5. 模型排行 Top 5 卡
    let max_model_calls = model_list.first().map(|m| m.calls).unwrap_or(1).max(1) as f32;
    let mut model_rows = div().flex().flex_col().gap(px(8.0));
    for (idx, m) in model_list.iter().enumerate() {
        let rank_color = match idx {
            0 => palette.amber_text,
            1 => palette.blue_text,
            2 => palette.emerald_text,
            _ => palette.muted_foreground,
        };
        let friendly = app
            .model_display_names
            .get(&m.model)
            .cloned()
            .unwrap_or_else(|| m.model.clone());
        let ratio = (m.calls as f32 / max_model_calls).clamp(0.04, 1.0);

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
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rank_color)
                                        .flex_shrink_0()
                                        .child(format!("#{}", idx + 1)),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.foreground)
                                        .truncate()
                                        .child(friendly),
                                )
                                .when_some(m.provider_hint.clone(), |el, hint| {
                                    el.child(
                                        div()
                                            .px(px(4.5))
                                            .py(px(0.5))
                                            .rounded(px(3.5))
                                            .bg(palette.muted)
                                            .text_size(px(9.5))
                                            .text_color(palette.muted_foreground)
                                            .flex_shrink_0()
                                            .child(hint),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .flex_shrink_0()
                                .text_size(px(10.5))
                                .text_color(palette.muted_foreground)
                                .child(format!("{} tok", format_tokens(m.tokens)))
                                .child("·")
                                .child(format_currency(m.cost))
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
                        .h(px(3.5))
                        .rounded_full()
                        .bg(palette.muted)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(chart_palette(idx))
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
        .gap(px(10.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.foreground)
                .child(locale.pick("模型调用排行 Top 5", "Top 5 Models")),
        )
        .child(model_rows);

    // 6. 客户端来源分布卡
    let mut client_bar = div()
        .w_full()
        .h(px(5.0))
        .rounded_full()
        .bg(palette.muted)
        .overflow_hidden()
        .flex()
        .gap(px(1.5));
    let mut client_rows = div().flex().flex_col().gap(px(6.0));
    for (idx, (cid, cnt)) in client_list.iter().enumerate() {
        let ratio = (*cnt as f32 / total_reqs.max(1) as f32).clamp(0.02, 1.0);
        let pct = ((*cnt as f64 / total_reqs.max(1) as f64) * 100.0).round() as i32;
        client_bar = client_bar.child(
            div()
                .h_full()
                .bg(chart_palette(idx + 2))
                .w(relative(ratio)),
        );
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
                        .child(render_client_avatar(cid, 18.0, true, palette))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .truncate()
                                .child(client_type_label(cid, locale)),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(palette.muted_foreground)
                        .child(format!("{} ({}%)", cnt, pct)),
                ),
        );
    }

    let clients_card = div()
        .flex_1()
        .min_w_0()
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.foreground)
                .child(locale.pick("客户端分布", "Agent Sources")),
        )
        .child(client_bar)
        .child(client_rows);

    // 7. 异常诊断与健康度卡
    let r_2xx = (ok_2xx as f32 / total_reqs.max(1) as f32).clamp(0.0, 1.0);
    let r_4xx = (err_4xx as f32 / total_reqs.max(1) as f32).clamp(0.0, 1.0);
    let r_5xx = (err_5xx as f32 / total_reqs.max(1) as f32).clamp(0.0, 1.0);

    let health_card = div()
        .flex_1()
        .min_w_0()
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .flex()
        .flex_col()
        .justify_between()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(palette.foreground)
                                .child(locale.pick("状态与异常诊断", "Health & Diagnostics")),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .text_size(px(10.5))
                                .child(
                                    div()
                                        .text_color(palette.emerald_text)
                                        .child(format!("2xx: {}", ok_2xx)),
                                )
                                .child(
                                    div()
                                        .text_color(palette.amber_text)
                                        .child(format!("4xx: {}", err_4xx)),
                                )
                                .child(
                                    div()
                                        .text_color(palette.rose_text)
                                        .child(format!("5xx: {}", err_5xx)),
                                ),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(5.0))
                        .rounded_full()
                        .bg(palette.muted)
                        .overflow_hidden()
                        .flex()
                        .when(r_2xx > 0.0, |b| {
                            b.child(div().h_full().bg(palette.emerald_text).w(relative(r_2xx)))
                        })
                        .when(r_4xx > 0.0, |b| {
                            b.child(div().h_full().bg(palette.amber_text).w(relative(r_4xx)))
                        })
                        .when(r_5xx > 0.0, |b| {
                            b.child(div().h_full().bg(palette.rose_text).w(relative(r_5xx)))
                        }),
                )
                .child(if err_list.is_empty() {
                    div()
                        .py(px(4.0))
                        .text_size(px(11.0))
                        .text_color(palette.emerald_text)
                        .child(locale.pick(
                            "所有请求均正常响应，无上游报错",
                            "All requests succeeded with zero upstream errors",
                        ))
                } else {
                    let mut col = div().flex().flex_col().gap(px(4.0));
                    for (code_key, cnt) in &err_list {
                        let desc = match *code_key {
                            "429" => locale.pick("429 触发速率或配额限制", "429 Rate / Quota Limited"),
                            "auth" => locale.pick("401/403 密钥或授权失效", "401/403 Auth Failed"),
                            "404" => locale.pick("404 上游模型或路径不存在", "404 Model Not Found"),
                            "upstream_timeout" => {
                                locale.pick("502/504 上游连接超时", "502/504 Upstream Timeout")
                            }
                            "500" => locale.pick("500 上游服务内部错误", "500 Upstream Error"),
                            _ => locale.pick("其他请求异常", "Other HTTP Errors"),
                        };
                        col = col.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .text_size(px(11.0))
                                .child(div().text_color(palette.muted_foreground).child(desc))
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.rose_text)
                                        .child(format!("{}x", cnt)),
                                ),
                        );
                    }
                    col
                }),
        )
        .child(
            div()
                .pt(px(8.0))
                .border_t_1()
                .border_color(palette.border_subtle)
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(10.5))
                .text_color(palette.muted_foreground)
                .child(format!(
                    "{}: {} ({}%)",
                    locale.pick("故障自动兜底", "Failover"),
                    fallback_count,
                    fallback_pct
                ))
                .child(format!(
                    "{}: {}%",
                    locale.pick("流式输出", "Streaming"),
                    stream_pct
                )),
        );

    let mid_pair = if is_wide {
        div()
            .flex()
            .items_stretch()
            .gap(px(8.0))
            .child(clients_card)
            .child(health_card)
    } else {
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(clients_card)
            .child(health_card)
    };

    // 8. 24 小时请求量趋势柱状图
    let max_bucket = hourly_buckets.iter().copied().max().unwrap_or(0).max(1);
    let cur_local_hour = local_now.hour() as i32;

    let hovered_hint = app.hovered_chart_hour.map(|slot| {
        let h = (cur_local_hour - (23 - slot as i32)).rem_euclid(24);
        let cnt = hourly_buckets.get(slot).copied().unwrap_or(0);
        format!(
            "{:02}:00 · {} {}",
            h,
            cnt,
            locale.pick("次请求", "requests")
        )
    });

    let mut bars_row = div()
        .w_full()
        .h(px(68.0))
        .flex()
        .items_end()
        .gap(px(3.0));

    for (slot, &cnt) in hourly_buckets.iter().enumerate() {
        let is_hov = app.hovered_chart_hour == Some(slot);
        let h_px = if cnt == 0 {
            3.0
        } else {
            ((cnt as f32 / max_bucket as f32) * 62.0).max(6.0)
        };
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
                        this.hovered_chart_hour = Some(slot);
                    } else if this.hovered_chart_hour == Some(slot) {
                        this.hovered_chart_hour = None;
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .w_full()
                        .h(px(h_px))
                        .rounded(px(2.0))
                        .bg(if cnt == 0 {
                            palette.muted
                        } else if is_hov {
                            palette.primary
                        } else {
                            palette.blue_text
                        }),
                ),
        );
    }

    let trend_card = div()
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
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.foreground)
                        .child(locale.pick("24 小时调用趋势", "24h Traffic Trend")),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(palette.muted_foreground)
                        .child(hovered_hint.unwrap_or_else(|| {
                            format!(
                                "{}: {}",
                                locale.pick("近 24h 合计", "24h total"),
                                hourly_buckets.iter().sum::<usize>()
                            )
                        })),
                ),
        )
        .child(bars_row)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(10.0))
                .text_color(palette.subtext)
                .child("-24h")
                .child("-12h")
                .child(locale.pick("现在", "Now")),
        );

    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(top_bar)
        .child(kpi_section)
        .child(provider_card)
        .child(top_models_card)
        .child(mid_pair)
        .child(trend_card)
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
