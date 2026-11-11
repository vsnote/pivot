use crate::components::button::NativeButton;
use crate::components::icons::{lucide_icon, lucide_spinner, render_brand_avatar};
use crate::components::switch::NativeSwitch;
use crate::i18n::Locale;
use crate::theme::ThemePalette;
use crate::PivotApp;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pivot_core::db::Provider;

pub const PROVIDER_ROW_HEIGHT: f32 = 42.0;

pub fn is_subscription_provider(p: &Provider) -> bool {
    p.auth_type.as_deref() == Some("subscription_pool")
        || p.preset_id.as_deref() == Some("antigravity")
        || p.preset_id.as_deref() == Some("chatgpt_subscription")
        || p.protocol == "chatgpt"
}

pub fn clean_display_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let no_proto = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let no_slash = no_proto.trim_end_matches('/');
    let no_v1 = no_slash.strip_suffix("/v1").unwrap_or(no_slash);
    no_v1.trim_end_matches('/').to_string()
}

pub fn render_provider_list(
    app: &PivotApp,
    palette: ThemePalette,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let locale = app.prefs.locale;

    if !app.initial_loaded {
        return div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .py(px(48.0))
            .gap(px(8.0))
            .child(lucide_spinner(
                "providers_loading_spinner",
                22.0,
                palette.muted_foreground,
            ))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(palette.muted_foreground)
                    .child(locale.pick("载入配置中…", "Loading configuration…")),
            );
    }

    if app.providers.is_empty() {
        return div()
            .w_full()
            .rounded(px(8.0))
            .bg(palette.card)
            .border_1()
            .border_color(palette.border)
            .px(px(16.0))
            .py(px(40.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(palette.muted_foreground)
                    .child(locale.pick("尚未配置任何服务商渠道", "No provider channels configured yet")),
            )
            .child(
                NativeButton::new(locale.pick("添加服务商", "Add Provider"), palette)
                    .outline()
                    .sm()
                    .icon(IconName::Plus)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_editor(None, window, cx);
                    })),
            );
    }

    // 存在拖拽预览时按 preview_ids 排列
    let active_drag_id = app
        .provider_drag
        .as_ref()
        .filter(|d| d.active)
        .map(|d| d.id.clone());

    let ordered_providers: Vec<Provider> = if let Some(drag) = app.provider_drag.as_ref().filter(|d| d.active) {
        drag.preview_ids
            .iter()
            .filter_map(|id| app.providers.iter().find(|p| &p.id == id).cloned())
            .collect()
    } else {
        app.providers.clone()
    };

    let mut card = div()
        .w_full()
        .rounded(px(8.0))
        .bg(palette.card)
        .border_1()
        .border_color(palette.border)
        .overflow_hidden()
        .flex()
        .flex_col();

    for (idx, provider) in ordered_providers.into_iter().enumerate() {
        let is_dragged_placeholder = active_drag_id.as_deref() == Some(provider.id.as_str());
        card = card.child(render_provider_row(
            app,
            &provider,
            idx,
            is_dragged_placeholder,
            false,
            locale,
            palette,
            cx,
        ));
    }

    // 底部合并的单个全宽「+ 添加服务商」按钮行（消除双层边框）
    card = card.child(
        div()
            .id("add_provider_footer_btn")
            .flex()
            .items_center()
            .justify_center()
            .gap(px(5.0))
            .w_full()
            .py(px(9.5))
            .px(px(14.0))
            .border_t_1()
            .border_color(palette.border)
            .cursor_pointer()
            .text_size(px(11.5))
            .font_weight(FontWeight::NORMAL)
            .text_color(palette.muted_foreground)
            .hover(move |s| s.bg(palette.muted_subtle).text_color(palette.foreground))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.open_editor(None, window, cx);
                }),
            )
            .child(lucide_icon(IconName::Plus, 13.0, palette.muted_foreground))
            .child(locale.pick("添加服务商", "Add Provider")),
    );

    card
}

#[allow(clippy::too_many_arguments)]
pub fn render_provider_row(
    app: &PivotApp,
    provider: &Provider,
    row_index: usize,
    is_drag_placeholder: bool,
    is_floating_preview: bool,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> AnyElement {
    let p_id = provider.id.clone();
    let is_sub = is_subscription_provider(provider);
    let sub_stat = app.sub_account_stats.get(&p_id).copied().unwrap_or(crate::app::SubAccountStat {
        total: 1,
        available: 1,
        has_auth_error: false,
    });
    let display_endpoint = clean_display_endpoint(&provider.base_url);
    let is_hovered = !is_floating_preview && app.hovered_provider_id.as_deref() == Some(p_id.as_str());
    let copy_key = format!("prov_url_{}", p_id);
    let is_copied = app.copied_id.as_deref() == Some(copy_key.as_str());
    let is_testing = app.testing_latencies.contains_key(&p_id);
    let latency_opt = app.latencies.get(&p_id).copied();
    let is_toggling = app.toggling_providers.contains_key(&p_id);
    let is_enabled = provider.enabled;

    if is_drag_placeholder {
        return div()
            .w_full()
            .h(px(PROVIDER_ROW_HEIGHT))
            .when(row_index > 0, |el| el.border_t_1().border_color(palette.border))
            .bg(palette.muted_subtle)
            .into_any_element();
    }

    let provider_for_click = provider.clone();
    let provider_for_ctx = provider.clone();
    let id_hover_enter = p_id.clone();
    let id_hover_leave = p_id.clone();
    let id_for_drag = p_id.clone();
    let id_for_copy = p_id.clone();
    let url_for_copy = provider.base_url.clone();
    let id_for_test = p_id.clone();
    let id_for_retest = p_id.clone();
    let id_for_toggle = p_id.clone();

    div()
        .id(ElementId::Name(SharedString::from(if is_floating_preview {
            format!("prov_floating_{}", p_id)
        } else {
            format!("prov_row_{}", p_id)
        })))
        .relative()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(10.0))
        .w_full()
        .min_h(px(PROVIDER_ROW_HEIGHT))
        .px(px(14.0))
        .py(px(9.0))
        .when(!is_floating_preview && row_index > 0, |el| {
            el.border_t_1().border_color(palette.border)
        })
        .when(!is_enabled, |el| el.opacity(0.55))
        .cursor(if is_floating_preview {
            CursorStyle::ClosedHand
        } else {
            CursorStyle::PointingHand
        })
        .when(!is_floating_preview, |el| {
            el.hover(move |s| s.bg(palette.card_hover))
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    if *hovered {
                        if this.hovered_provider_id.as_deref() != Some(id_hover_enter.as_str()) {
                            this.hovered_provider_id = Some(id_hover_enter.clone());
                            cx.notify();
                        }
                    } else if this.hovered_provider_id.as_deref() == Some(id_hover_enter.as_str()) {
                        this.hovered_provider_id = None;
                        cx.notify();
                    }
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        if this.provider_drag.is_none() {
                            this.open_editor(Some(provider_for_click.clone()), window, cx);
                        }
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.open_provider_context_menu(
                            provider_for_ctx.clone(),
                            f32::from(e.position.x),
                            f32::from(e.position.y),
                            window,
                            cx,
                        );
                    }),
                )
        })
        // 左侧绝对定位拖拽手柄：不占正常流空间，保证 24px 品牌图标严丝合缝对齐
        .when(!is_floating_preview && app.providers.len() > 1, |el| {
            el.child(
                div()
                    .absolute()
                    .left(px(2.0))
                    .top(px(11.0))
                    .w(px(12.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor(CursorStyle::OpenHand)
                    .opacity(if is_hovered { 0.75 } else { 0.0 })
                    .hover(|s| s.opacity(1.0))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.begin_provider_drag(
                                id_for_drag.clone(),
                                row_index,
                                f32::from(e.position.x),
                                f32::from(e.position.y),
                                cx,
                            );
                        }),
                    )
                    .child(lucide_icon(
                        IconName::GripVertical,
                        12.0,
                        palette.muted_foreground,
                    )),
            )
        })
        // 左侧主体：24x24 软底槽品牌头像 + 名称 + (订阅 · N 徽章 或 清洗后端点)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .min_w_0()
                .flex_1()
                .child(render_brand_avatar(
                    &provider.name,
                    Some(&provider.protocol),
                    provider.preset_id.as_deref(),
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
                        .flex_1()
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .truncate()
                                .max_w(px(176.0))
                                .child(provider.name.clone()),
                        )
                        .when(is_sub, |el| {
                            let show_expired = sub_stat.total > 0
                                && (sub_stat.has_auth_error || sub_stat.available == 0);
                            if show_expired {
                                el.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.0))
                                        .flex_shrink_0()
                                        .h(px(18.0))
                                        .px(px(6.0))
                                        .rounded(px(4.0))
                                        .bg(palette.rose_soft_bg)
                                        .border_1()
                                        .border_color(palette.rose_soft_border)
                                        .text_size(px(10.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.rose_text)
                                        .child(
                                            div()
                                                .size(px(5.0))
                                                .rounded_full()
                                                .flex_shrink_0()
                                                .bg(palette.rose_text),
                                        )
                                        .child(format!(
                                            "{} · {}/{}",
                                            locale.pick("授权已失效", "Auth Expired"),
                                            sub_stat.available,
                                            sub_stat.total
                                        )),
                                )
                            } else {
                                let (bg, border, fg) = if sub_stat.total > 0 {
                                    (
                                        palette.sub_badge_bg,
                                        palette.sub_badge_border,
                                        palette.sub_badge_text,
                                    )
                                } else {
                                    (
                                        palette.muted_subtle,
                                        palette.border,
                                        palette.muted_foreground,
                                    )
                                };
                                el.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(3.0))
                                        .flex_shrink_0()
                                        .h(px(18.0))
                                        .px(px(6.0))
                                        .rounded(px(4.0))
                                        .bg(bg)
                                        .border_1()
                                        .border_color(border)
                                        .text_size(px(10.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(fg)
                                        .child(format!(
                                            "{} · {}",
                                            locale.pick("订阅", "Subscription"),
                                            sub_stat.total
                                        )),
                                )
                            }
                        })
                        .when(!is_sub && !display_endpoint.is_empty(), |el| {
                            el.child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(11.0))
                                    .text_color(palette.subtext)
                                    .child(display_endpoint),
                            )
                        }),
                ),
        )
        // 右侧操作区：Hover 幽灵复制 + 测速胶囊/Hover 幽灵闪电 + Monochrome NativeSwitch
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .flex_shrink_0()
                .on_mouse_down(MouseButton::Left, |_, _, app| {
                    app.stop_propagation();
                })
                // 1. 非订阅且有 Base URL：仅在 Hover 或刚复制成功时显示单色幽灵复制按钮
                .when(!is_sub && !provider.base_url.is_empty() && (is_hovered || is_copied), |el| {
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(24.0))
                            .rounded(px(5.0))
                            .cursor_pointer()
                            .hover(move |s| s.bg(palette.muted))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.copy_to_clipboard(
                                        format!("prov_url_{}", id_for_copy),
                                        url_for_copy.clone(),
                                        cx,
                                    );
                                }),
                            )
                            .child(if is_copied {
                                lucide_icon(IconName::Check, 13.5, palette.emerald_text)
                            } else {
                                lucide_icon(IconName::Copy, 13.5, palette.muted_foreground)
                            }),
                    )
                })
                // 2. 启用态下的延迟测试药丸或 Hover 闪电幽灵按钮
                .when(is_enabled, |el| {
                    if is_testing {
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(palette.muted_subtle)
                                .text_size(px(11.0))
                                .text_color(palette.muted_foreground)
                                .child(lucide_spinner(
                                    format!("testing_spin_{}", id_hover_leave),
                                    11.5,
                                    palette.muted_foreground,
                                ))
                                .child(locale.pick("测速中", "Testing")),
                        )
                    } else if let Some(ms) = latency_opt {
                        if ms > 0 {
                            let (dot_c, text_c, bg_c) = if ms < 300 {
                                (
                                    rgb(0x10b981),
                                    palette.emerald_text,
                                    palette.emerald_soft_bg,
                                )
                            } else if ms < 1000 {
                                (rgb(0xf59e0b), palette.amber_text, palette.amber_soft_bg)
                            } else {
                                (rgb(0xf97316), rgb(0xfb923c), rgba(0xf973161a))
                            };
                            el.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(bg_c)
                                    .cursor_pointer()
                                    .text_size(px(11.0))
                                    .text_color(text_c)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.test_provider_latency(id_for_retest.clone(), cx);
                                        }),
                                    )
                                    .child(div().size(px(5.5)).rounded_full().bg(dot_c))
                                    .child(format!("{}ms", ms)),
                            )
                        } else {
                            // ms <= 0 (如 -1) 表示无法连接
                            el.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(palette.rose_soft_bg)
                                    .cursor_pointer()
                                    .text_size(px(11.0))
                                    .text_color(palette.rose_text)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.test_provider_latency(id_for_retest.clone(), cx);
                                        }),
                                    )
                                    .child(div().size(px(5.5)).rounded_full().bg(palette.rose_text))
                                    .child(locale.pick("无法连接", "Unreachable")),
                            )
                        }
                    } else if is_hovered {
                        // 未测速时：仅在 Hover 时浮现单色线性闪电按钮，常态绝不占位！
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(24.0))
                                .rounded(px(5.0))
                                .cursor_pointer()
                                .hover(move |s| s.bg(palette.muted))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.test_provider_latency(id_for_test.clone(), cx);
                                    }),
                                )
                                .child(lucide_icon(IconName::Zap, 13.5, palette.muted_foreground)),
                        )
                    } else {
                        el
                    }
                })
                // 3. Monochrome 弹簧动画开关
                .child(
                    NativeSwitch::new(
                        format!("prov_switch_{}", provider.id),
                        is_enabled,
                        palette,
                    )
                    .disabled(is_toggling)
                    .on_toggle(cx.listener(move |this, _, _, cx| {
                        this.toggle_provider(id_for_toggle.clone(), !is_enabled, cx);
                    })),
                ),
        )
        .into_any_element()
}
