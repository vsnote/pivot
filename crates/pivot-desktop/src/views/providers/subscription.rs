use crate::components::button::NativeButton;
use crate::components::icons::{lucide_icon, lucide_spinner, render_brand_avatar};
use crate::components::switch::NativeSwitch;
use crate::i18n::Locale;
use crate::theme::ThemePalette;
use crate::views::providers::context_menu::format_short_reset_time;
use crate::views::providers::editor::ProviderEditorState;
use crate::PivotApp;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pivot_core::db::SubscriptionAccount;
use pivot_core::oauth::{AccountQuota, QuotaBucket, ResetCredit};

fn is_chatgpt_editor(editor: &ProviderEditorState) -> bool {
    editor.preset_id.as_deref() == Some("chatgpt_subscription") || editor.protocol == "chatgpt"
}

fn clean_quota_label(label: &str) -> String {
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

fn is_account_auth_expired(acc: &SubscriptionAccount, quota: Option<&AccountQuota>, now_ts: i64) -> bool {
    if acc.cooldown_until - now_ts > 3600 {
        return true;
    }
    if acc.refresh_token.as_deref().unwrap_or("").is_empty()
        && acc.expires_at > 0
        && acc.expires_at <= now_ts
    {
        return true;
    }
    if let Some(err) = quota.and_then(|q| q.error.as_deref()) {
        if pivot_core::oauth::is_token_revoked_or_invalid(err)
            || err.contains("失效")
            || err.contains("重新登录")
            || err.contains("重新授权")
        {
            return true;
        }
    }
    false
}

pub fn render_subscription_section(
    app: &PivotApp,
    editor: &ProviderEditorState,
    palette: ThemePalette,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> Div {
    let locale = app.prefs.locale;
    let is_chatgpt = is_chatgpt_editor(editor);
    let account_type = if is_chatgpt { "chatgpt" } else { "antigravity" };
    let now_ts = chrono::Utc::now().timestamp();

    let total_count = editor.sub_accounts.len();
    let active_count = editor
        .sub_accounts
        .iter()
        .filter(|a| {
            a.enabled
                && a.cooldown_until <= now_ts
                && !is_account_auth_expired(a, editor.sub_quotas.get(&a.id), now_ts)
        })
        .count();

    let display_name = editor.name_input.read(cx).value().to_string();
    let brand_name = if display_name.trim().is_empty() {
        if is_chatgpt {
            "ChatGPT".to_string()
        } else {
            "Antigravity".to_string()
        }
    } else {
        display_name
    };
    let icon_key = editor.preset_id.as_deref();

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        // 1. 顶部品牌与协议状态卡
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.0))
                .py(px(10.0))
                .rounded(px(8.0))
                .bg(palette.card)
                .border_1()
                .border_color(palette.border)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .min_w_0()
                        .child(render_brand_avatar(
                            &brand_name,
                            Some(editor.protocol.as_str()),
                            icon_key,
                            24.0,
                            true,
                            palette,
                        ))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .min_w_0()
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(palette.foreground)
                                        .truncate()
                                        .child(brand_name),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.0))
                                        .px(px(6.0))
                                        .py(px(1.5))
                                        .rounded(px(4.0))
                                        .bg(palette.emerald_soft_bg)
                                        .border_1()
                                        .border_color(palette.emerald_soft_border)
                                        .child(
                                            div()
                                                .size(px(5.0))
                                                .rounded_full()
                                                .bg(palette.emerald_text),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(palette.emerald_text)
                                                .child(locale.pick("订阅", "Subscription")),
                                        ),
                                ),
                        ),
                )
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .bg(palette.muted)
                        .border_1()
                        .border_color(palette.border_subtle)
                        .text_size(px(10.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.muted_foreground)
                        .child("OAuth 2.0"),
                ),
        )
        // 2. 授权账号池标题栏 + 操作按钮
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
                        .flex_wrap()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .child(locale.pick("账号", "Accounts")),
                        )
                        .child(if total_count > 0 {
                            div()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded(px(4.0))
                                .bg(if active_count > 0 {
                                    palette.emerald_soft_bg
                                } else {
                                    palette.rose_soft_bg
                                })
                                .border_1()
                                .border_color(if active_count > 0 {
                                    palette.emerald_soft_border
                                } else {
                                    palette.rose_soft_border
                                })
                                .text_size(px(10.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if active_count > 0 {
                                    palette.emerald_text
                                } else {
                                    palette.rose_text
                                })
                                .child(format!(
                                    "{} / {} {}",
                                    active_count,
                                    total_count,
                                    locale.pick("可用", "active")
                                ))
                        } else {
                            div()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded(px(4.0))
                                .bg(palette.muted)
                                .text_size(px(10.5))
                                .text_color(palette.muted_foreground)
                                .child(locale.pick("未连接", "Not linked"))
                        })
                        .when(total_count > 1, |el| {
                            el.child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(palette.muted_foreground)
                                    .child(locale.pick("· 自动轮询均衡", "· Round-robin")),
                            )
                        }),
                )
                .when(total_count > 0, |el| {
                    let acct_kind = account_type.to_string();
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .when(is_chatgpt, |row| {
                                row.child(
                                    NativeButton::new(
                                        locale.pick("导入本地凭据", "Import Local"),
                                        palette,
                                    )
                                    .outline()
                                    .xs()
                                    .disabled(editor.importing_codex || editor.connecting_oauth)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.editor_import_codex_account(cx);
                                    })),
                                )
                            })
                            .child(
                                NativeButton::new(
                                    locale.pick("添加账号", "Add Account"),
                                    palette,
                                )
                                .outline()
                                .xs()
                                .icon(IconName::Plus)
                                .disabled(editor.connecting_oauth || editor.importing_codex)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.editor_start_subscription_oauth(acct_kind.clone(), cx);
                                })),
                            ),
                    )
                }),
        )
        // OAuth 等待提示条
        .when(editor.connecting_oauth, |el| {
            el.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(9.0))
                    .rounded(px(6.0))
                    .bg(palette.blue_soft_bg)
                    .border_1()
                    .border_color(palette.blue_soft_border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(lucide_spinner(
                                "oauth_waiting_spinner",
                                13.0,
                                palette.blue_text,
                            ))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(palette.blue_text)
                                    .child(locale.pick(
                                        "已在浏览器打开登录页，请完成授权…",
                                        "Browser opened. Complete sign-in to continue…",
                                    )),
                            ),
                    ),
            )
        })
        // 3. 空账号引导卡片 或 已连接账号卡片列表
        .child(if total_count == 0 {
            render_empty_accounts_card(editor, is_chatgpt, account_type, locale, palette, cx)
        } else {
            let mut list = div()
                .w_full()
                .rounded(px(8.0))
                .bg(palette.card)
                .border_1()
                .border_color(palette.border)
                .overflow_hidden()
                .flex()
                .flex_col();

            for (idx, acc) in editor.sub_accounts.iter().enumerate() {
                list = list.child(render_account_row(
                    acc,
                    idx > 0,
                    account_type,
                    now_ts,
                    editor,
                    locale,
                    palette,
                    cx,
                ));
            }
            list
        })
}

fn render_empty_accounts_card(
    editor: &ProviderEditorState,
    is_chatgpt: bool,
    account_type: &str,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let acct_kind = account_type.to_string();
    let busy = editor.connecting_oauth || editor.importing_codex;

    div()
        .w_full()
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .px(px(16.0))
        .py(px(18.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(30.0))
                .rounded_full()
                .bg(palette.icon_slot_bg)
                .border_1()
                .border_color(palette.icon_slot_border)
                .flex()
                .items_center()
                .justify_center()
                .child(lucide_icon(IconName::Zap, 14.0, palette.muted_foreground)),
        )
        .child(
            div()
                .text_size(px(11.5))
                .text_color(palette.muted_foreground)
                .text_center()
                .child(if is_chatgpt {
                    locale.pick(
                        "连接 ChatGPT Plus / Pro / Team 账号，或直接导入本地 Codex CLI 登录凭据",
                        "Connect your ChatGPT subscription or import local Codex CLI credentials",
                    )
                } else {
                    locale.pick(
                        "授权 Google 账号以接入 Antigravity 官方订阅模型池",
                        "Sign in with Google to access Antigravity subscription models",
                    )
                }),
        )
        .child(if is_chatgpt {
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .flex_wrap()
                .child(
                    NativeButton::new(
                        locale.pick("从本地 Codex 导入", "Import from Codex"),
                        palette,
                    )
                    .outline()
                    .sm()
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.editor_import_codex_account(cx);
                    })),
                )
                .child(
                    NativeButton::new(
                        locale.pick("网页授权登录", "Browser Sign In"),
                        palette,
                    )
                    .primary()
                    .sm()
                    .icon(IconName::ExternalLink)
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.editor_start_subscription_oauth(acct_kind.clone(), cx);
                    })),
                )
        } else {
            div().child(
                NativeButton::new(
                    locale.pick("立即连接账号", "Connect Account"),
                    palette,
                )
                .primary()
                .sm()
                .icon(IconName::ExternalLink)
                .disabled(busy)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.editor_start_subscription_oauth(acct_kind.clone(), cx);
                })),
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn render_account_row(
    acc: &SubscriptionAccount,
    has_top_border: bool,
    account_type: &str,
    now_ts: i64,
    editor: &ProviderEditorState,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let acc_id = acc.id.clone();
    let quota = editor.sub_quotas.get(&acc_id);
    let is_loading_quota = editor.loading_quotas.get(&acc_id).copied().unwrap_or(false);
    let is_expanded = editor.expanded_quota_account_id.as_deref() == Some(acc_id.as_str());

    let auth_expired = is_account_auth_expired(acc, quota, now_ts);
    let cooldown_remaining = (acc.cooldown_until - now_ts).max(0);
    let in_cooldown = acc.enabled && !auth_expired && cooldown_remaining > 0;

    let dot_color = if !acc.enabled {
        palette.subtext
    } else if auth_expired {
        palette.rose_text
    } else if in_cooldown {
        palette.amber_text
    } else {
        palette.emerald_text
    };

    let email_label = acc
        .email
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let short = acc.id.chars().take(8).collect::<String>();
            format!("Account {}", short)
        });

    let plan_badge = acc
        .plan_type
        .as_deref()
        .or_else(|| quota.and_then(|q| q.plan_type.as_deref()))
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let toggle_acc_id = acc_id.clone();
    let next_enabled = !acc.enabled;
    let expand_acc_id = acc_id.clone();
    let delete_acc_id = acc_id.clone();
    let reauth_kind = account_type.to_string();

    div()
        .flex()
        .flex_col()
        .when(has_top_border, |el| {
            el.border_t_1().border_color(palette.border_subtle)
        })
        // 主行
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .px(px(12.0))
                .py(px(9.0))
                .when(!acc.enabled, |el| el.opacity(0.6))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .size(px(7.0))
                                .rounded_full()
                                .flex_shrink_0()
                                .bg(dot_color),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .truncate()
                                .child(email_label),
                        )
                        .when_some(plan_badge, |el, plan| {
                            let lower = plan.to_ascii_lowercase();
                            let is_pro = lower.contains("pro")
                                || lower.contains("team")
                                || lower.contains("enterprise");
                            let (bg, border, fg) = if is_pro {
                                (
                                    palette.blue_soft_bg,
                                    palette.blue_soft_border,
                                    palette.blue_text,
                                )
                            } else {
                                (
                                    palette.secondary,
                                    palette.border_subtle,
                                    palette.muted_foreground,
                                )
                            };
                            el.child(
                                div()
                                    .px(px(5.0))
                                    .py(px(1.0))
                                    .rounded(px(4.0))
                                    .bg(bg)
                                    .border_1()
                                    .border_color(border)
                                    .text_size(px(10.0))
                                    .text_color(fg)
                                    .flex_shrink_0()
                                    .child(plan.to_string()),
                            )
                        })
                        .when(auth_expired, |el| {
                            el.child(
                                div()
                                    .px(px(5.0))
                                    .py(px(1.0))
                                    .rounded(px(4.0))
                                    .bg(palette.rose_soft_bg)
                                    .border_1()
                                    .border_color(palette.rose_soft_border)
                                    .text_size(px(10.0))
                                    .text_color(palette.rose_text)
                                    .flex_shrink_0()
                                    .child(locale.pick("授权失效", "Expired")),
                            )
                        })
                        .when(in_cooldown, |el| {
                            el.child(
                                div()
                                    .px(px(5.0))
                                    .py(px(1.0))
                                    .rounded(px(4.0))
                                    .bg(palette.amber_soft_bg)
                                    .border_1()
                                    .border_color(palette.amber_soft_border)
                                    .text_size(px(10.0))
                                    .text_color(palette.amber_text)
                                    .flex_shrink_0()
                                    .child(format!(
                                        "{} {}s",
                                        locale.pick("冷却", "Cooldown"),
                                        cooldown_remaining
                                    )),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .flex_shrink_0()
                        .child(if auth_expired {
                            NativeButton::new(locale.pick("重新授权", "Re-auth"), palette)
                                .outline()
                                .xs()
                                .disabled(editor.connecting_oauth)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.editor_start_subscription_oauth(reauth_kind.clone(), cx);
                                }))
                                .into_any_element()
                        } else {
                            NativeButton::new(
                                if is_expanded {
                                    locale.pick("收起配额", "Hide Quota")
                                } else {
                                    locale.pick("查看配额", "Quota")
                                },
                                palette,
                            )
                            .ghost()
                            .xs()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.editor_toggle_account_quota(expand_acc_id.clone(), cx);
                            }))
                            .into_any_element()
                        })
                        .child(
                            NativeSwitch::new(
                                format!("sub_acc_switch_{}", acc_id),
                                acc.enabled,
                                palette,
                            )
                            .xs()
                            .on_toggle(cx.listener(move |this, _, _, cx| {
                                this.editor_toggle_subscription_account(
                                    toggle_acc_id.clone(),
                                    next_enabled,
                                    cx,
                                );
                            })),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("del_sub_acc_{}", acc_id)))
                                .size(px(22.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_color(palette.muted_foreground)
                                .hover(move |s| {
                                    s.bg(palette.destructive_soft_bg)
                                        .text_color(palette.destructive_text)
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.editor_delete_subscription_account(
                                            delete_acc_id.clone(),
                                            cx,
                                        );
                                    }),
                                )
                                .child(lucide_icon(
                                    IconName::X,
                                    12.5,
                                    palette.muted_foreground,
                                )),
                        ),
                ),
        )
        // 展开配额与重置卡详情面板
        .when(is_expanded, |el| {
            el.child(render_expanded_quota_panel(
                &acc_id,
                quota,
                is_loading_quota,
                editor.consuming_credit_id.as_deref(),
                locale,
                palette,
                cx,
            ))
        })
}

fn render_expanded_quota_panel(
    account_id: &str,
    quota: Option<&AccountQuota>,
    is_loading: bool,
    consuming_credit_id: Option<&str>,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let refresh_acc_id = account_id.to_string();

    let mut panel = div()
        .px(px(12.0))
        .py(px(10.0))
        .bg(palette.muted_subtle)
        .border_t_1()
        .border_color(palette.border_subtle)
        .flex()
        .flex_col()
        .gap(px(8.0))
        // 面板小标题 + 刷新
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.muted_foreground)
                        .child(locale.pick("实时配额余量", "Live Quota")),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("refresh_sub_quota_{}", account_id)))
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .hover(move |s| s.bg(palette.accent))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.editor_refresh_account_quota(refresh_acc_id.clone(), cx);
                            }),
                        )
                        .child(if is_loading {
                            lucide_spinner(
                                format!("quota_spin_{}", account_id),
                                11.0,
                                palette.muted_foreground,
                            )
                            .into_any_element()
                        } else {
                            lucide_icon(IconName::RefreshCw, 11.0, palette.muted_foreground)
                                .into_any_element()
                        })
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(palette.muted_foreground)
                                .child(locale.pick("刷新", "Refresh")),
                        ),
                ),
        );

    if is_loading && quota.is_none() {
        return panel.child(
            div()
                .py(px(8.0))
                .text_size(px(11.0))
                .text_color(palette.muted_foreground)
                .child(locale.pick("正在查询上游实时配额…", "Fetching live quota…")),
        );
    }

    if let Some(q) = quota {
        if let Some(err) = q.error.as_deref().filter(|s| !s.is_empty()) {
            panel = panel.child(
                div()
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(5.0))
                    .bg(palette.rose_soft_bg)
                    .border_1()
                    .border_color(palette.rose_soft_border)
                    .text_size(px(11.0))
                    .text_color(palette.rose_text)
                    .child(err.to_string()),
            );
        } else if q.buckets.is_empty() {
            panel = panel.child(
                div()
                    .py(px(4.0))
                    .text_size(px(11.0))
                    .text_color(palette.muted_foreground)
                    .child(locale.pick("当前暂无细分额度限制", "No quota limits reported")),
            );
        } else {
            // 双列配额进度条网格
            let mut grid = div().flex().flex_col().gap(px(6.0));
            for pair in q.buckets.chunks(2) {
                let mut row = div().flex().items_stretch().gap(px(6.0));
                for bucket in pair {
                    row = row.child(render_quota_bucket_cell(bucket, locale, palette));
                }
                if pair.len() == 1 {
                    row = row.child(div().flex_1());
                }
                grid = grid.child(row);
            }
            panel = panel.child(grid);
        }

        // ChatGPT 限流重置卡支持
        if let Some(credits) = q.reset_credits.as_ref().filter(|c| !c.is_empty()) {
            panel = panel.child(render_reset_credits_box(
                account_id,
                credits,
                consuming_credit_id,
                locale,
                palette,
                cx,
            ));
        }
    }

    panel
}

fn render_quota_bucket_cell(bucket: &QuotaBucket, locale: Locale, palette: ThemePalette) -> Div {
    let pct = (bucket.remaining_fraction.clamp(0.0, 1.0) * 100.0).round() as i32;
    let bar_color = if bucket.is_exhausted || pct <= 15 {
        palette.rose_text
    } else if pct <= 40 {
        palette.amber_text
    } else {
        palette.emerald_text
    };
    let reset_hint = format_short_reset_time(bucket.reset_time.as_deref(), locale);

    div()
        .flex_1()
        .min_w_0()
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border_subtle)
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(10.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.foreground)
                        .truncate()
                        .child(clean_quota_label(&bucket.label)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .flex_shrink_0()
                        .when_some(reset_hint, |el, hint| {
                            el.child(
                                div()
                                    .text_size(px(9.5))
                                    .text_color(palette.subtext)
                                    .child(format!("↻{}", hint)),
                            )
                        })
                        .child(
                            div()
                                .text_size(px(10.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(bar_color)
                                .child(format!("{}%", pct)),
                        ),
                ),
        )
        .child(
            div()
                .w_full()
                .h(px(4.0))
                .rounded_full()
                .bg(palette.muted)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .rounded_full()
                        .bg(bar_color)
                        .w(relative((pct as f32 / 100.0).clamp(0.0, 1.0))),
                ),
        )
}

fn render_reset_credits_box(
    account_id: &str,
    credits: &[ResetCredit],
    consuming_credit_id: Option<&str>,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Div {
    let available: Vec<&ResetCredit> = credits
        .iter()
        .filter(|c| {
            let st = c.status.to_lowercase();
            st == "available" || st == "active" || st == "unused" || st.is_empty()
        })
        .collect();

    if available.is_empty() {
        return div();
    }

    let first_credit = available[0];
    let credit_id = first_credit.id.clone();
    let acc_id = account_id.to_string();
    let is_consuming = consuming_credit_id == Some(credit_id.as_str());
    let exp_short = format_short_reset_time(Some(first_credit.expires_at.as_str()), locale);

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border_subtle)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .min_w_0()
                .child(lucide_icon(IconName::Sparkles, 12.0, palette.amber_text))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(palette.foreground)
                        .truncate()
                        .child(format!(
                            "{} × {}",
                            locale.pick("限流重置卡", "Reset Credits"),
                            available.len()
                        )),
                )
                .when_some(exp_short, |el, exp| {
                    el.child(
                        div()
                            .text_size(px(10.0))
                            .text_color(palette.muted_foreground)
                            .child(format!(
                                "({} {})",
                                exp,
                                locale.pick("后过期", "left")
                            )),
                    )
                }),
        )
        .child(
            NativeButton::new(
                if is_consuming {
                    locale.pick("重置中…", "Applying…")
                } else {
                    locale.pick("使用重置卡", "Use Credit")
                },
                palette,
            )
            .outline()
            .xs()
            .disabled(consuming_credit_id.is_some())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.editor_consume_reset_credit(acc_id.clone(), credit_id.clone(), cx);
            })),
        )
}
