use crate::components::icons::{lucide_icon, lucide_spinner, render_brand_avatar};
use crate::components::text_input::NativeTextInput;
use crate::i18n::Locale;
use crate::theme::ThemePalette;
use crate::views::providers::list::is_subscription_provider;
use crate::PivotApp;
use gpui_kit::assets::IconName;
use gpui_kit::base::{Align, Placement, Positioner};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pivot_core::db::SubscriptionAccount;
use pivot_core::oauth::{AccountQuota, QuotaBucket};
use std::collections::HashMap;

pub struct ProviderContextMenuState {
    pub provider_id: String,
    pub x: f32,
    pub y: f32,
    pub search_input: Entity<InputState>,
    pub scroll_handle: ScrollHandle,
    pub accounts: Vec<SubscriptionAccount>,
    pub quotas: HashMap<String, AccountQuota>,
    pub selected_account_id: Option<String>,
    pub loading_quota: bool,
    pub hovered_model: Option<String>,
}

fn clean_bucket_label(label: &str) -> String {
    if label.contains("Claude") && (label.contains("5小时") || label.contains("5h")) {
        return "Claude (5h)".to_string();
    }
    if label.contains("Claude") && (label.contains("周") || label.contains("week")) {
        return "Claude (周)".to_string();
    }
    if label.contains("Gemini") && (label.contains("5小时") || label.contains("5h")) {
        return "Gemini (5h)".to_string();
    }
    if label.contains("Gemini") && (label.contains("周") || label.contains("week")) {
        return "Gemini (周)".to_string();
    }
    label
        .replace("周期配额", "")
        .replace("配额", "")
        .trim()
        .to_string()
}

pub fn format_short_reset_time(reset_str: Option<&str>, locale: Locale) -> Option<String> {
    let raw = reset_str?.trim();
    if raw.is_empty() {
        return None;
    }
    let dt = chrono::DateTime::parse_from_rfc3339(raw).ok()?;
    let now = chrono::Utc::now().timestamp();
    let diff_secs = dt.timestamp() - now;
    if diff_secs <= 0 {
        return Some(locale.pick("即将", "Soon").to_string());
    }
    let mins = diff_secs / 60;
    if mins < 60 {
        return Some(format!("{}m", mins));
    }
    let hours = mins / 60;
    if hours < 24 {
        return Some(format!("{}h", hours));
    }
    Some(format!("{}d", hours / 24))
}

pub fn render_provider_context_menu(
    app: &PivotApp,
    menu: &ProviderContextMenuState,
    palette: ThemePalette,
    window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let locale = app.prefs.locale;
    let Some(provider) = app
        .providers
        .iter()
        .find(|p| p.id == menu.provider_id)
        .cloned()
    else {
        return div().into_any_element();
    };

    let is_sub = is_subscription_provider(&provider);
    let viewport = window.viewport_size();
    let win_w = f32::from(viewport.width).max(320.0);
    let win_h = f32::from(viewport.height).max(320.0);
    let menu_w = 280.0f32.min(win_w - 20.0);
    let anchor_bounds = Bounds::new(
        point(px(menu.x.max(10.0)), px(menu.y.max(10.0))),
        size(px(1.0), px(1.0)),
    );

    let search_q = menu.search_input.read(cx).value().trim().to_lowercase();
    let filtered_models: Vec<String> = provider
        .models
        .iter()
        .filter(|m| {
            if search_q.is_empty() {
                return true;
            }
            let match_id = m.to_lowercase().contains(&search_q);
            let match_disp = app
                .model_display_names
                .get(*m)
                .map(|d| d.to_lowercase().contains(&search_q))
                .unwrap_or(false);
            match_id || match_disp
        })
        .cloned()
        .collect();

    let active_account = menu
        .accounts
        .iter()
        .find(|a| menu.selected_account_id.as_deref() == Some(a.id.as_str()))
        .or_else(|| menu.accounts.first())
        .cloned();

    let active_quota = active_account
        .as_ref()
        .and_then(|acc| menu.quotas.get(&acc.id))
        .cloned();

    let is_testing = app.testing_latencies.contains_key(&provider.id);
    let latency_ms = app.latencies.get(&provider.id).copied();
    let url_copy_key = format!("ctx_url_{}", provider.id);
    let is_url_copied = app.copied_id.as_deref() == Some(url_copy_key.as_str());

    let prov_id_for_test = provider.id.clone();
    let prov_id_for_copy = provider.id.clone();
    let prov_url_for_copy = provider.base_url.clone();
    let prov_for_edit = provider.clone();
    let prov_for_del = provider.clone();
    let prov_id_for_refresh = provider.id.clone();
    let prov_url_for_refresh = provider.base_url.clone();

    let menu_panel = div()
        .id("provider_ctx_menu_panel")
        .w(px(menu_w))
        .max_h(px((win_h - 20.0).clamp(240.0, 420.0)))
        .rounded(px(8.0))
        .bg(palette.popover)
        .border_1()
        .border_color(palette.border)
        .shadow(palette.popover_shadows())
        .overflow_hidden()
        .occlude()
        .p(px(4.0))
        .flex()
        .flex_col()
        .on_scroll(|_, _, app| {
            app.stop_propagation();
        })
        .on_mouse_down(MouseButton::Left, |_, _, app| {
            app.stop_propagation();
        })
        .on_mouse_down(MouseButton::Right, |_, _, app| {
            app.stop_propagation();
        })
        // 1. 顶部服务商标题栏
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(9.0))
                .py(px(5.5))
                .border_b_1()
                .border_color(palette.border_subtle)
                .child(render_brand_avatar(
                    &provider.name,
                    Some(&provider.protocol),
                    provider.preset_id.as_deref(),
                    18.0,
                    true,
                    palette,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(palette.foreground)
                                .truncate()
                                .child(provider.name.clone()),
                        )
                        .when(is_sub, |el| {
                            el.child(
                                div()
                                    .flex_shrink_0()
                                    .px(px(5.0))
                                    .py(px(1.0))
                                    .rounded(px(4.0))
                                    .bg(palette.sub_badge_bg)
                                    .border_1()
                                    .border_color(palette.sub_badge_border)
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.sub_badge_text)
                                    .child(locale.pick("订阅", "Subscription")),
                            )
                        }),
                ),
        )
        // 中部自适应滚动区（配额 + 可用模型，确保头部与底部操作项永远完整可见）
        .child(
            div()
                .id("ctx_menu_body_scroll")
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&menu.scroll_handle)
                .flex()
                .flex_col()
                // 2. 订阅配额微面板（仅订阅渠道展示）
                .when(is_sub, |el| {
                    let now_sec = chrono::Utc::now().timestamp();
                    let loading_q = menu.loading_quota;
                    let accounts_snapshot = menu.accounts.clone();

                    el.child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap(px(4.5))
                            .px(px(4.0))
                            .py(px(5.0))
                            .border_b_1()
                            .border_color(palette.border_subtle)
                            // 顶栏：订阅配额 + 多账号药丸 + 刷新按钮
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px(px(6.0))
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .min_w_0()
                                            .flex_1()
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .text_size(px(11.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(palette.muted_foreground)
                                                    .child(locale.pick("订阅配额", "Quotas")),
                                            )
                                            .when(accounts_snapshot.len() > 1, |row| {
                                                let mut pills = div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .overflow_hidden();
                                                for (idx, acc) in accounts_snapshot.iter().enumerate() {
                                                    let is_sel = active_account
                                                        .as_ref()
                                                        .map(|a| a.id == acc.id)
                                                        .unwrap_or(false);
                                                    let is_cd = acc.cooldown_until > now_sec;
                                                    let short_name = acc
                                                        .email
                                                        .as_deref()
                                                        .and_then(|e| e.split('@').next())
                                                        .map(|s| s.to_string())
                                                        .unwrap_or_else(|| {
                                                            format!(
                                                                "{}{}",
                                                                locale.pick("账号", "#"),
                                                                idx + 1
                                                            )
                                                        });
                                                    let acc_id = acc.id.clone();
                                                    pills = pills.child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(3.0))
                                                            .px(px(5.0))
                                                            .py(px(1.5))
                                                            .rounded(px(4.0))
                                                            .cursor_pointer()
                                                            .bg(if is_sel {
                                                                palette.secondary
                                                            } else {
                                                                rgba(0x00000000)
                                                            })
                                                            .text_size(px(10.0))
                                                            .text_color(if is_sel {
                                                                palette.foreground
                                                            } else {
                                                                palette.muted_foreground
                                                            })
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                cx.listener(move |this, _, _, cx| {
                                                                    if let Some(m) = &mut this.context_menu {
                                                                        m.selected_account_id =
                                                                            Some(acc_id.clone());
                                                                        cx.notify();
                                                                    }
                                                                }),
                                                            )
                                                            .child(short_name)
                                                            .when(is_cd, |p| {
                                                                p.child(
                                                                    div()
                                                                        .size(px(5.0))
                                                                        .rounded_full()
                                                                        .bg(palette.amber_text),
                                                                )
                                                            }),
                                                    );
                                                }
                                                row.child(pills)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .p(px(3.0))
                                            .rounded(px(4.0))
                                            .cursor_pointer()
                                            .hover(move |s| s.bg(palette.muted))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    if !loading_q {
                                                        this.refresh_context_menu_quotas(
                                                            prov_id_for_refresh.clone(),
                                                            prov_url_for_refresh.clone(),
                                                            cx,
                                                        );
                                                    }
                                                }),
                                            )
                                            .child(if loading_q {
                                                lucide_spinner(
                                                    "ctx_quota_spin",
                                                    12.0,
                                                    palette.foreground,
                                                )
                                                .into_any_element()
                                            } else {
                                                lucide_icon(
                                                    IconName::RefreshCw,
                                                    12.0,
                                                    palette.muted_foreground,
                                                )
                                                .into_any_element()
                                            }),
                                    ),
                            )
                            // 当前选中账号邮箱与状态
                            .when_some(active_account.as_ref(), |col, acc| {
                                let email_label = acc.email.clone().unwrap_or_else(|| {
                                    let short_id: String = acc.id.chars().take(8).collect();
                                    format!(
                                        "{} {}",
                                        locale.pick("账号", "Account"),
                                        short_id
                                    )
                                });
                                let is_cd = acc.cooldown_until > now_sec;
                                let cd_mins = ((acc.cooldown_until - now_sec + 59) / 60).max(1);

                                col.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .px(px(6.0))
                                        .text_size(px(10.0))
                                        .child(
                                            div()
                                                .max_w(px(170.0))
                                                .truncate()
                                                .text_color(palette.muted_foreground)
                                                .child(email_label),
                                        )
                                        .child(if is_cd {
                                            div()
                                                .text_color(palette.amber_text)
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(format!(
                                                    "429 {} ({}m)",
                                                    locale.pick("冷却中", "Cooldown"),
                                                    cd_mins
                                                ))
                                        } else if !acc.enabled {
                                            div()
                                                .text_color(palette.subtext)
                                                .child(locale.pick("已停用", "Disabled"))
                                        } else {
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(4.0))
                                                .text_color(palette.emerald_text)
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(
                                                    div()
                                                        .size(px(6.0))
                                                        .rounded_full()
                                                        .bg(rgb(0x10b981)),
                                                )
                                                .child(locale.pick("正常", "Normal"))
                                        }),
                                )
                            })
                            // 双列紧凑配额指标网格
                            .child(if loading_q && accounts_snapshot.is_empty() {
                                div()
                                    .py(px(10.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(6.0))
                                    .text_size(px(11.0))
                                    .text_color(palette.muted_foreground)
                                    .child(lucide_spinner(
                                        "ctx_init_quota_spin",
                                        12.0,
                                        palette.muted_foreground,
                                    ))
                                    .child(locale.pick("查询配额中…", "Querying quotas…"))
                            } else if accounts_snapshot.is_empty() {
                                div()
                                    .py(px(4.0))
                                    .flex()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .text_color(palette.subtext)
                                    .child(locale.pick("尚未绑定订阅账号", "No accounts bound"))
                            } else if let Some(q) = active_quota.filter(|q| !q.buckets.is_empty()) {
                                render_quota_buckets_grid(&q.buckets, locale, palette)
                            } else if loading_q {
                                div()
                                    .py(px(4.0))
                                    .flex()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .text_color(palette.muted_foreground)
                                    .child(locale.pick("更新配额中…", "Updating quotas…"))
                            } else {
                                div()
                            }),
                    )
                })
                // 3. 可用模型区
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .py(px(3.0))
                        .child(
                            div()
                                .px(px(9.0))
                                .py(px(3.0))
                                .text_size(px(10.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.muted_foreground)
                                .child(format!(
                                    "{} ({})",
                                    locale.pick("可用模型", "Available Models"),
                                    provider.models.len()
                                )),
                        )
                        .when(provider.models.len() > 5, |col| {
                            col.child(
                                div().px(px(8.0)).pb(px(3.5)).child(
                                    NativeTextInput::search(&menu.search_input, palette)
                                        .xs()
                                        .render(window, cx),
                                ),
                            )
                        })
                        .child(if filtered_models.is_empty() {
                            div()
                                .py(px(6.0))
                                .flex()
                                .justify_center()
                                .text_size(px(11.0))
                                .text_color(palette.subtext)
                                .child(if provider.models.is_empty() {
                                    locale.pick("未配置任何模型", "No models configured")
                                } else {
                                    locale.pick("无匹配模型", "No matching models")
                                })
                                .into_any_element()
                        } else {
                            let mut list = div()
                                .flex()
                                .flex_col()
                                .gap(px(1.0))
                                .px(px(4.0));

                            for (idx, model_id) in filtered_models.into_iter().enumerate() {
                                let disp = app
                                    .model_display_names
                                    .get(&model_id)
                                    .cloned()
                                    .unwrap_or_else(|| model_id.clone());
                                let copy_key = format!("ctx_model_{}", model_id);
                                let is_copied = app.copied_id.as_deref() == Some(copy_key.as_str());
                                let is_row_hovered =
                                    menu.hovered_model.as_deref() == Some(model_id.as_str());
                                let m_hover = model_id.clone();
                                let m_copy = model_id.clone();

                                list = list.child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "ctx_model_row_{}_{}",
                                            idx, model_id
                                        )))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(6.0))
                                        .px(px(8.0))
                                        .py(px(3.5))
                                        .rounded(px(5.0))
                                        .cursor_pointer()
                                        .hover(move |s| s.bg(palette.accent))
                                        .on_hover(cx.listener(
                                            move |this, hovered: &bool, _, cx| {
                                                if let Some(cm) = &mut this.context_menu {
                                                    if *hovered {
                                                        if cm.hovered_model.as_deref()
                                                            != Some(m_hover.as_str())
                                                        {
                                                            cm.hovered_model =
                                                                Some(m_hover.clone());
                                                            cx.notify();
                                                        }
                                                    } else if cm.hovered_model.as_deref()
                                                        == Some(m_hover.as_str())
                                                    {
                                                        cm.hovered_model = None;
                                                        cx.notify();
                                                    }
                                                }
                                            },
                                        ))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.copy_to_clipboard(
                                                    format!("ctx_model_{}", m_copy),
                                                    m_copy.clone(),
                                                    cx,
                                                );
                                            }),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(5.0))
                                                .min_w_0()
                                                .flex_1()
                                                .child(
                                                    div()
                                                        .min_w_0()
                                                        .text_size(px(11.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(palette.foreground)
                                                        .truncate()
                                                        .child(disp),
                                                )
                                                .when(is_copied || is_row_hovered, |r| {
                                                    r.child(
                                                        div().flex_shrink_0().child(if is_copied {
                                                            lucide_icon(
                                                                IconName::Check,
                                                                11.5,
                                                                palette.emerald_text,
                                                            )
                                                        } else {
                                                            lucide_icon(
                                                                IconName::Copy,
                                                                11.5,
                                                                palette.muted_foreground,
                                                            )
                                                        }),
                                                    )
                                                }),
                                        ),
                                );
                            }
                            list.into_any_element()
                        }),
                ),
        )
        // 分割线
        .child(
            div()
                .flex_shrink_0()
                .my(px(2.5))
                .border_t_1()
                .border_color(palette.border_subtle),
        )
        // 4. 底部快捷操作区（固定不收缩，绝不被遮挡）
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(1.5))
                // 测试端点延迟
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px(px(9.0))
                        .py(px(5.0))
                        .rounded(px(5.0))
                        .when(!provider.enabled || is_testing, |el| el.opacity(0.4))
                        .cursor(if provider.enabled && !is_testing {
                            CursorStyle::PointingHand
                        } else {
                            CursorStyle::Arrow
                        })
                        .hover(move |s| {
                            if provider.enabled && !is_testing {
                                s.bg(palette.accent)
                            } else {
                                s
                            }
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                if provider.enabled && !is_testing {
                                    this.test_provider_latency(prov_id_for_test.clone(), cx);
                                    this.context_menu = None;
                                    cx.notify();
                                }
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(7.5))
                                .text_size(px(11.5))
                                .text_color(palette.foreground)
                                .child(lucide_icon(
                                    IconName::Zap,
                                    13.0,
                                    palette.muted_foreground,
                                ))
                                .child(if is_testing {
                                    locale.pick("正在测试延迟…", "Testing latency…")
                                } else {
                                    locale.pick("测试延迟", "Test Latency")
                                }),
                        )
                        .when_some(latency_ms.filter(|&ms| ms > 0), |el, ms| {
                            el.child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(palette.subtext)
                                    .child(format!("{}ms", ms)),
                            )
                        }),
                )
                // 复制 Base URL（非订阅渠道）
                .when(!is_sub && !provider.base_url.is_empty(), |col| {
                    col.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.5))
                            .px(px(9.0))
                            .py(px(5.0))
                            .rounded(px(5.0))
                            .cursor_pointer()
                            .text_size(px(11.5))
                            .text_color(palette.foreground)
                            .hover(move |s| s.bg(palette.accent))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.copy_to_clipboard(
                                        format!("ctx_url_{}", prov_id_for_copy),
                                        prov_url_for_copy.clone(),
                                        cx,
                                    );
                                    this.context_menu = None;
                                    cx.notify();
                                }),
                            )
                            .child(if is_url_copied {
                                lucide_icon(IconName::Check, 13.0, palette.emerald_text)
                            } else {
                                lucide_icon(IconName::Copy, 13.0, palette.muted_foreground)
                            })
                            .child(if is_url_copied {
                                locale.pick("已复制", "Copied")
                            } else {
                                locale.pick("复制 Base URL", "Copy Base URL")
                            }),
                    )
                })
                // 编辑服务商配置
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.5))
                        .px(px(9.0))
                        .py(px(5.0))
                        .rounded(px(5.0))
                        .cursor_pointer()
                        .text_size(px(11.5))
                        .text_color(palette.foreground)
                        .hover(move |s| s.bg(palette.accent))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.context_menu = None;
                                this.open_editor(Some(prov_for_edit.clone()), window, cx);
                            }),
                        )
                        .child(lucide_icon(
                            IconName::SlidersHorizontal,
                            13.0,
                            palette.muted_foreground,
                        ))
                        .child(locale.pick("编辑服务商…", "Edit Provider…")),
                )
                // 删除服务商
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.5))
                        .px(px(9.0))
                        .py(px(5.0))
                        .rounded(px(5.0))
                        .cursor_pointer()
                        .text_size(px(11.5))
                        .text_color(palette.destructive_text)
                        .hover(move |s| s.bg(palette.destructive_soft_bg))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.context_menu = None;
                                this.delete_provider_target = Some(prov_for_del.clone());
                                cx.notify();
                            }),
                        )
                        .child(lucide_icon(
                            IconName::Trash,
                            13.0,
                            palette.destructive_text,
                        ))
                        .child(locale.pick("删除服务商", "Delete Provider")),
                ),
        );

    div()
        .id("provider_ctx_menu_backdrop")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .on_scroll(|_, _, app| {
            app.stop_propagation();
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.context_menu = None;
                cx.notify();
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.context_menu = None;
                cx.notify();
            }),
        )
        .child(
            Positioner::side(anchor_bounds)
                .placement(Placement::Bottom)
                .align(Align::Start)
                .offset(px(2.0))
                .margin(px(10.0))
                .occlude()
                .child(menu_panel),
        )
        .into_any_element()
}

fn render_quota_buckets_grid(
    buckets: &[QuotaBucket],
    locale: Locale,
    palette: ThemePalette,
) -> Div {
    let mut container = div().flex().flex_col().gap(px(3.5)).px(px(2.0));
    let single = buckets.len() == 1;

    for pair in buckets.chunks(if single { 1 } else { 2 }) {
        let mut row = div().flex().items_stretch().gap(px(3.5)).w_full();
        for b in pair {
            let pct = (b.remaining_fraction * 100.0).round() as i32;
            let is_out = b.is_exhausted;
            let is_low = b.remaining_fraction <= 0.2;
            let short_reset = format_short_reset_time(b.reset_time.as_deref(), locale);
            let title = clean_bucket_label(&b.label);
            let val_color = if is_out {
                palette.rose_text
            } else if is_low {
                palette.amber_text
            } else {
                palette.foreground
            };

            row = row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h(px(33.0))
                    .px(px(6.0))
                    .py(px(4.5))
                    .rounded(px(5.0))
                    .bg(palette.muted_subtle)
                    .border_1()
                    .border_color(palette.border_subtle)
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(palette.muted_foreground)
                            .truncate()
                            .child(title),
                    )
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .justify_between()
                            .gap(px(4.0))
                            .mt(px(1.5))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(val_color)
                                    .child(if is_out {
                                        locale.pick("耗尽", "0%").to_string()
                                    } else {
                                        format!("{}%", pct)
                                    }),
                            )
                            .when_some(short_reset, |el, sr| {
                                el.child(
                                    div()
                                        .text_size(px(9.0))
                                        .text_color(palette.subtext)
                                        .child(sr),
                                )
                            }),
                    ),
            );
        }
        container = container.child(row);
    }

    container
}
