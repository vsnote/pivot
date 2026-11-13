use crate::components::button::{render_segmented_tabs, NativeButton, SegmentedTabsSize};
use crate::components::icons::{lucide_icon, lucide_spinner, render_pivot_app_icon};
use crate::i18n::Locale;
use crate::theme::{ThemePalette, ThemePreference};
use crate::updater::{extract_release_note_items, UpdateCheckState, RELEASE_LATEST_PAGE_URL};
use crate::PivotApp;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Debug, Clone)]
pub struct SettingsBanner {
    pub is_error: bool,
    pub message: String,
}

pub fn render_settings_view(
    app: &PivotApp,
    palette: ThemePalette,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let locale = app.prefs.locale;
    let cur_theme = app.prefs.theme;
    let app_for_theme = cx.entity();
    let app_for_locale = cx.entity();
    let version_str = env!("CARGO_PKG_VERSION");

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        // 备份/恢复结果提示条
        .when_some(app.settings_banner.clone(), |el, banner| {
            let fg = if banner.is_error {
                palette.rose_text
            } else {
                palette.emerald_text
            };
            let bg = if banner.is_error {
                palette.rose_soft_bg
            } else {
                palette.emerald_soft_bg
            };
            let bd = if banner.is_error {
                palette.rose_soft_border
            } else {
                palette.emerald_soft_border
            };

            el.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(7.0))
                    .bg(bg)
                    .border_1()
                    .border_color(bd)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.0))
                            .min_w_0()
                            .flex_1()
                            .child(lucide_icon(
                                if banner.is_error {
                                    IconName::CircleAlert
                                } else {
                                    IconName::CircleCheck
                                },
                                13.5,
                                fg,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(11.5))
                                    .text_color(fg)
                                    .child(banner.message),
                            ),
                    )
                    .child(
                        div()
                            .id("dismiss_settings_banner")
                            .size(px(20.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.settings_banner = None;
                                    cx.notify();
                                }),
                            )
                            .child(lucide_icon(IconName::X, 11.5, fg)),
                    ),
            )
        })
        // 卡片 1：通用偏好（外观 + 语言）
        .child(
            div()
                .w_full()
                .rounded(px(8.0))
                .bg(palette.card)
                .border_1()
                .border_color(palette.border)
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(render_section_header(
                    locale.pick("通用", "General"),
                    palette,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(10.0))
                        .px(px(14.0))
                        .py(px(9.5))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(palette.foreground)
                                .child(locale.pick("外观", "Appearance")),
                        )
                        .child(render_segmented_tabs(
                            vec![
                                (
                                    ThemePreference::Light,
                                    locale.pick("浅色", "Light").to_string(),
                                ),
                                (
                                    ThemePreference::Dark,
                                    locale.pick("深色", "Dark").to_string(),
                                ),
                                (
                                    ThemePreference::System,
                                    locale.pick("跟随系统", "System").to_string(),
                                ),
                            ],
                            cur_theme,
                            SegmentedTabsSize::Sm,
                            palette,
                            move |next_theme, window, app_cx| {
                                app_for_theme.update(app_cx, |this, cx| {
                                    this.set_theme_preference(next_theme, window, cx);
                                });
                            },
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(10.0))
                        .px(px(14.0))
                        .py(px(9.5))
                        .border_t_1()
                        .border_color(palette.border_subtle)
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(palette.foreground)
                                .child(locale.pick("语言", "Language")),
                        )
                        .child(render_segmented_tabs(
                            vec![
                                (Locale::ZhCn, "简体中文".to_string()),
                                (Locale::En, "English".to_string()),
                            ],
                            locale,
                            SegmentedTabsSize::Sm,
                            palette,
                            move |next_locale, _, app_cx| {
                                app_for_locale.update(app_cx, |this, cx| {
                                    this.set_locale_preference(next_locale, cx);
                                });
                            },
                        )),
                ),
        )
        // 卡片 2：快捷键速览
        .child(
            div()
                .w_full()
                .rounded(px(8.0))
                .bg(palette.card)
                .border_1()
                .border_color(palette.border)
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(render_section_header(
                    locale.pick("快捷键", "Shortcuts"),
                    palette,
                ))
                .child(render_shortcut_row(
                    locale.pick("切换导航标签", "Switch navigation tabs"),
                    &["⌘", "1 ~ 4"],
                    false,
                    palette,
                ))
                .child(render_shortcut_row(
                    locale.pick("添加服务商", "Add provider"),
                    &["⌘", "N"],
                    true,
                    palette,
                ))
                .child(render_shortcut_row(
                    locale.pick("关闭弹窗 / 返回列表", "Close dialog / Back"),
                    &["Esc"],
                    true,
                    palette,
                )),
        )
        // 卡片 3：配置备份与迁移
        .child(
            div()
                .w_full()
                .rounded(px(8.0))
                .bg(palette.card)
                .border_1()
                .border_color(palette.border)
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(render_section_header(
                    locale.pick("备份与恢复", "Backup & Restore"),
                    palette,
                ))
                .child(
                    div()
                        .p(px(14.0))
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(palette.muted_foreground)
                                .child(locale.pick(
                                    "导出或恢复所有服务商、API 密钥池与模型配置 (.json)",
                                    "Export or restore providers, API keys, and model pools (.json)",
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .flex_wrap()
                                .child(
                                    NativeButton::new(
                                        locale.pick("导出备份", "Export Backup"),
                                        palette,
                                    )
                                    .outline()
                                    .sm()
                                    .icon(IconName::Download)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.export_backup_json(cx);
                                    })),
                                )
                                .child(
                                    NativeButton::new(
                                        locale.pick("恢复备份", "Restore Backup"),
                                        palette,
                                    )
                                    .outline()
                                    .sm()
                                    .icon(IconName::Upload)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.import_backup_json(cx);
                                    })),
                                ),
                        ),
                ),
        )
        // 卡片 4：关于 Pivot & 版本检查
        .child(
            div()
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
                        .p(px(14.0))
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(render_pivot_app_icon(40.0, palette.is_dark))
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
                                                                .text_size(px(13.0))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(palette.foreground)
                                                                .child("Pivot"),
                                                        )
                                                        .child(
                                                            div()
                                                                .px(px(6.0))
                                                                .py(px(1.0))
                                                                .rounded(px(4.0))
                                                                .bg(palette.muted)
                                                                .text_size(px(10.5))
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(
                                                                    palette.muted_foreground,
                                                                )
                                                                .child(format!("v{}", version_str)),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.0))
                                                        .text_color(palette.muted_foreground)
                                                        .child(locale.pick(
                                                            "AI 编程客户端本地网关与统一路由中枢",
                                                            "Local AI Coding Agent Gateway & Router",
                                                        )),
                                                ),
                                        ),
                                )
                                .child(render_update_action(&app.update_state, locale, palette, cx)),
                        )
                        // 更新详情、进度与重启面板
                        .when_some(
                            render_update_panel(&app.update_state, locale, palette, cx),
                            |el, panel| el.child(panel),
                        ),
                )
                .child(
                    div()
                        .px(px(14.0))
                        .py(px(9.0))
                        .bg(palette.muted_subtle)
                        .border_t_1()
                        .border_color(palette.border_subtle)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(palette.muted_foreground)
                                .truncate()
                                .child(locale.pick(
                                    "纯本地运行 · 数据存储于 ~/.pivot/pivot.db",
                                    "Runs 100% locally · Stored in ~/.pivot/pivot.db",
                                )),
                        )
                        .child(
                            div()
                                .id("open_github_repo_link")
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .cursor_pointer()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.muted_foreground)
                                .hover(move |s| s.text_color(palette.foreground))
                                .on_mouse_down(MouseButton::Left, |_, _, app_cx| {
                                    app_cx.open_url("https://github.com/vsnote/pivot");
                                })
                                .child("GitHub")
                                .child(lucide_icon(
                                    IconName::ExternalLink,
                                    11.0,
                                    palette.muted_foreground,
                                )),
                        ),
                ),
        )
}

fn render_section_header(title: &str, palette: ThemePalette) -> Div {
    div()
        .px(px(14.0))
        .py(px(6.5))
        .bg(palette.muted_subtle)
        .border_b_1()
        .border_color(palette.border)
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.muted_foreground)
        .child(title.to_string())
}

fn render_shortcut_row(
    label: &str,
    keys: &[&str],
    has_top_border: bool,
    palette: ThemePalette,
) -> Div {
    let mut kbd_group = div().flex().items_center().gap(px(4.0));
    for &k in keys {
        kbd_group = kbd_group.child(
            div()
                .px(px(6.0))
                .py(px(1.5))
                .rounded(px(4.0))
                .bg(palette.muted)
                .border_1()
                .border_color(palette.border)
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.muted_foreground)
                .child(k.to_string()),
        );
    }

    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(14.0))
        .py(px(9.5))
        .when(has_top_border, |el| {
            el.border_t_1().border_color(palette.border_subtle)
        })
        .child(
            div()
                .text_size(px(12.0))
                .text_color(palette.foreground)
                .child(label.to_string()),
        )
        .child(kbd_group)
}

fn render_update_action(
    state: &UpdateCheckState,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> AnyElement {
    match state {
        UpdateCheckState::Checking => div()
            .flex()
            .items_center()
            .gap(px(5.0))
            .px(px(8.0))
            .py(px(4.0))
            .child(lucide_spinner(
                "update_check_spin",
                12.0,
                palette.muted_foreground,
            ))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(palette.muted_foreground)
                    .child(locale.pick("检查中…", "Checking…")),
            )
            .into_any_element(),
        UpdateCheckState::UpToDate => div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .text_size(px(11.0))
                    .text_color(palette.emerald_text)
                    .child(lucide_icon(
                        IconName::CircleCheck,
                        12.0,
                        palette.emerald_text,
                    ))
                    .child(locale.pick("已是最新", "Up to date")),
            )
            .into_any_element(),
        UpdateCheckState::Downloading { .. } | UpdateCheckState::Installing { .. } => {
            NativeButton::new(locale.pick("检查更新", "Check Updates"), palette)
                .outline()
                .xs()
                .icon(IconName::RefreshCw)
                .disabled(true)
                .on_click(|_, _, _| {})
                .into_any_element()
        }
        UpdateCheckState::ReadyToRestart { .. } => div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .text_size(px(11.0))
            .text_color(palette.emerald_text)
            .child(lucide_icon(
                IconName::CircleCheck,
                12.0,
                palette.emerald_text,
            ))
            .child(locale.pick("已就绪", "Ready"))
            .into_any_element(),
        _ => NativeButton::new(locale.pick("检查更新", "Check Updates"), palette)
            .outline()
            .xs()
            .icon(IconName::RefreshCw)
            .on_click(cx.listener(|this, _, _, cx| {
                this.check_for_updates(cx);
            }))
            .into_any_element(),
    }
}

fn render_update_panel(
    state: &UpdateCheckState,
    locale: Locale,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> Option<AnyElement> {
    match state {
        UpdateCheckState::Available(info) => {
            let ver = info.version.clone();
            let note_items = extract_release_note_items(info.notes.as_deref());
            let release_url = info.release_url.clone();
            let action_btn = if info.can_self_update() {
                let info_for_click = info.clone();
                NativeButton::new(locale.pick("立即更新", "Update Now"), palette)
                    .primary()
                    .xs()
                    .icon(IconName::Download)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.start_self_update(info_for_click.clone(), cx);
                    }))
            } else {
                let dl_url = info.release_url.clone();
                NativeButton::new(locale.pick("前往下载", "Download"), palette)
                    .primary()
                    .xs()
                    .icon(IconName::ExternalLink)
                    .on_click(move |_, _, app_cx| {
                        app_cx.open_url(&dl_url);
                    })
            };

            Some(
                div()
                    .p(px(10.0))
                    .rounded(px(6.0))
                    .bg(palette.blue_soft_bg)
                    .border_1()
                    .border_color(palette.blue_soft_border)
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("open_release_notes_link")
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .cursor_pointer()
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(palette.blue_text)
                                    .on_mouse_down(MouseButton::Left, move |_, _, app_cx| {
                                        app_cx.open_url(&release_url);
                                    })
                                    .child(format!(
                                        "{} v{}",
                                        locale.pick("发现新版本", "New release available:"),
                                        ver
                                    ))
                                    .child(lucide_icon(
                                        IconName::ExternalLink,
                                        11.0,
                                        palette.blue_text,
                                    )),
                            )
                            .child(action_btn),
                    )
                    .when(!note_items.is_empty(), |box_el| {
                        let mut list = div()
                            .pt(px(2.0))
                            .flex()
                            .flex_col()
                            .gap(px(4.0));
                        for item in note_items {
                            list = list.child(
                                div()
                                    .flex()
                                    .items_start()
                                    .gap(px(6.0))
                                    .text_size(px(11.5))
                                    .line_height(rems(1.4))
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_color(palette.blue_text)
                                            .child("•"),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_color(palette.secondary_foreground)
                                            .child(item),
                                    ),
                            );
                        }
                        box_el.child(list)
                    })
                    .into_any_element(),
            )
        }
        UpdateCheckState::Downloading { info, progress } => {
            let pct = (*progress).min(100);
            let label = format!(
                "{} v{}…",
                locale.pick("正在下载", "Downloading"),
                info.version
            );
            Some(
                div()
                    .p(px(10.0))
                    .rounded(px(6.0))
                    .bg(palette.blue_soft_bg)
                    .border_1()
                    .border_color(palette.blue_soft_border)
                    .flex()
                    .flex_col()
                    .gap(px(7.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.blue_text)
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.muted_foreground)
                                    .child(format!("{}%", pct)),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(5.0))
                            .rounded(px(99.0))
                            .bg(palette.muted)
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .rounded(px(99.0))
                                    .bg(palette.blue_text)
                                    .w(relative((pct as f32 / 100.0).clamp(0.0, 1.0))),
                            ),
                    )
                    .into_any_element(),
            )
        }
        UpdateCheckState::Installing { info } => {
            let label = format!(
                "{} v{}…",
                locale.pick("正在安装", "Installing"),
                info.version
            );
            Some(
                div()
                    .p(px(10.0))
                    .rounded(px(6.0))
                    .bg(palette.blue_soft_bg)
                    .border_1()
                    .border_color(palette.blue_soft_border)
                    .flex()
                    .flex_col()
                    .gap(px(7.0))
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
                                    .gap(px(5.0))
                                    .child(lucide_spinner(
                                        "update_install_spin",
                                        12.0,
                                        palette.blue_text,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(palette.blue_text)
                                            .child(label),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.muted_foreground)
                                    .child("100%"),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(5.0))
                            .rounded(px(99.0))
                            .bg(palette.muted)
                            .overflow_hidden()
                            .child(div().h_full().w_full().rounded(px(99.0)).bg(palette.blue_text)),
                    )
                    .into_any_element(),
            )
        }
        UpdateCheckState::ReadyToRestart {
            version,
            app_bundle,
        } => {
            let ver = version.clone();
            let bundle_for_click = app_bundle.clone();
            Some(
                div()
                    .p(px(10.0))
                    .rounded(px(6.0))
                    .bg(palette.emerald_soft_bg)
                    .border_1()
                    .border_color(palette.emerald_soft_border)
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
                            .child(lucide_icon(
                                IconName::CircleCheck,
                                13.0,
                                palette.emerald_text,
                            ))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(palette.emerald_text)
                                    .child(format!(
                                        "v{} {}",
                                        ver,
                                        locale.pick("已安装", "installed")
                                    )),
                            ),
                    )
                    .child(
                        NativeButton::new(locale.pick("立即重启", "Restart Now"), palette)
                            .primary()
                            .xs()
                            .icon(IconName::RefreshCw)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.restart_for_update(bundle_for_click.clone(), cx);
                            })),
                    )
                    .into_any_element(),
            )
        }
        UpdateCheckState::Failed { error, retry_info } => {
            let err_msg = error.clone();
            let retry_target = retry_info.clone();
            let dl_url = retry_info
                .as_ref()
                .map(|i| i.release_url.clone())
                .unwrap_or_else(|| RELEASE_LATEST_PAGE_URL.to_string());

            Some(
                div()
                    .p(px(10.0))
                    .rounded(px(6.0))
                    .bg(palette.rose_soft_bg)
                    .border_1()
                    .border_color(palette.rose_soft_border)
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(lucide_icon(
                                IconName::CircleAlert,
                                13.0,
                                palette.rose_text,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(11.0))
                                    .text_color(palette.rose_text)
                                    .child(err_msg),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                NativeButton::new(locale.pick("重试", "Retry"), palette)
                                    .outline()
                                    .xs()
                                    .icon(IconName::RefreshCw)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(info) = retry_target.clone() {
                                            if info.can_self_update() {
                                                this.start_self_update(info, cx);
                                                return;
                                            }
                                        }
                                        this.check_for_updates(cx);
                                    })),
                            )
                            .child(
                                NativeButton::new(locale.pick("前往下载", "Download"), palette)
                                    .outline()
                                    .xs()
                                    .icon(IconName::ExternalLink)
                                    .on_click(move |_, _, app_cx| {
                                        app_cx.open_url(&dl_url);
                                    }),
                            ),
                    )
                    .into_any_element(),
            )
        }
        _ => None,
    }
}
