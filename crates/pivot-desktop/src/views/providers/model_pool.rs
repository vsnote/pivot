use crate::components::button::{render_segmented_tabs, NativeButton, SegmentedTabsSize};
use crate::components::icons::{lucide_icon, lucide_spinner};
use crate::components::text_input::NativeTextInput;
use crate::theme::ThemePalette;
use crate::views::providers::editor::{ModelFilterTab, ProviderEditorState};
use crate::PivotApp;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::collections::HashSet;

struct FamilyGroup {
    label: &'static str,
    members: Vec<String>,
}

type FamilySpec = (&'static str, fn(&str) -> bool);

fn detect_family_groups(pool: &[String]) -> Vec<FamilyGroup> {
    if pool.len() < 5 {
        return Vec::new();
    }
    let specs: [FamilySpec; 5] = [
        ("Claude", |s| s.contains("claude")),
        ("GPT", |s| {
            s.contains("gpt")
                || s.starts_with("o1")
                || s.starts_with("o3")
                || s.starts_with("o4")
                || s.contains("/o1")
                || s.contains("/o3")
                || s.contains("/o4")
        }),
        ("Gemini", |s| s.contains("gemini")),
        ("DeepSeek", |s| s.contains("deepseek")),
        ("Qwen", |s| s.contains("qwen")),
    ];

    let mut out = Vec::new();
    for (label, pred) in specs {
        let members: Vec<String> = pool
            .iter()
            .filter(|m| pred(&m.to_lowercase()))
            .cloned()
            .collect();
        if members.len() >= 2 {
            out.push(FamilyGroup { label, members });
        }
    }
    out
}

pub fn render_model_pool_section(
    app: &PivotApp,
    editor: &ProviderEditorState,
    is_subscription: bool,
    palette: ThemePalette,
    window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> Div {
    let locale = app.prefs.locale;
    let total_pool = editor.model_pool.len();
    let selected_set: HashSet<&str> = editor.selected_models.iter().map(|s| s.as_str()).collect();
    let selected_count = editor.selected_models.len();

    let upstream_set: Option<HashSet<&str>> = editor
        .upstream_models
        .as_ref()
        .map(|list| list.iter().map(|s| s.as_str()).collect());

    let stale_count = if let Some(ref up) = upstream_set {
        editor
            .selected_models
            .iter()
            .filter(|m| !up.contains(m.as_str()))
            .count()
    } else {
        0
    };

    let search_q = editor
        .model_search_input
        .read(cx)
        .value()
        .trim()
        .to_lowercase();

    let filtered_models: Vec<String> = editor
        .model_pool
        .iter()
        .filter(|m| {
            let is_sel = selected_set.contains(m.as_str());
            match editor.model_filter {
                ModelFilterTab::All => {}
                ModelFilterTab::Selected => {
                    if !is_sel {
                        return false;
                    }
                }
                ModelFilterTab::Unselected => {
                    if is_sel {
                        return false;
                    }
                }
            }
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

    let all_filtered_selected = !filtered_models.is_empty()
        && filtered_models
            .iter()
            .all(|m| selected_set.contains(m.as_str()));

    let family_groups = detect_family_groups(&editor.model_pool);
    let app_entity = cx.entity();

    let can_sync = if is_subscription {
        !editor.sub_accounts.is_empty() && !editor.fetching_models
    } else {
        !editor.fetching_models
    };

    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        // 1. 顶部标签 + 计数胶囊 + 未提供移除胶囊 + 同步按钮
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
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .child(locale.pick("可用模型", "Models")),
                        )
                        .child(
                            div()
                                .px(px(5.5))
                                .py(px(1.0))
                                .rounded(px(4.0))
                                .bg(if selected_count > 0 {
                                    palette.secondary
                                } else {
                                    palette.amber_soft_bg
                                })
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if selected_count > 0 {
                                    palette.foreground
                                } else {
                                    palette.amber_text
                                })
                                .child(format!(
                                    "{} {} / {} {}",
                                    locale.pick("已选", "Selected"),
                                    selected_count,
                                    locale.pick("共", "of"),
                                    total_pool
                                )),
                        )
                        .when(stale_count > 0, |el| {
                            el.child(
                                div()
                                    .id("remove_stale_models_badge")
                                    .px(px(6.0))
                                    .py(px(1.0))
                                    .rounded(px(4.0))
                                    .bg(palette.amber_soft_bg)
                                    .border_1()
                                    .border_color(palette.amber_soft_border)
                                    .cursor_pointer()
                                    .text_size(px(10.0))
                                    .text_color(palette.amber_text)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.editor_remove_stale_models(cx);
                                        }),
                                    )
                                    .child(format!(
                                        "{} {}",
                                        stale_count,
                                        locale.pick(
                                            "个未在上游提供 · 移除",
                                            "missing upstream · Remove"
                                        )
                                    )),
                            )
                        }),
                )
                .child(
                    div()
                        .id("fetch_upstream_models_btn")
                        .flex()
                        .items_center()
                        .gap(px(4.5))
                        .h(px(23.0))
                        .px(px(7.5))
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(palette.input)
                        .bg(palette.background)
                        .when(can_sync, |el| {
                            el.cursor_pointer()
                                .hover(move |s| s.bg(palette.accent))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.editor_fetch_models(cx);
                                    }),
                                )
                        })
                        .when(!can_sync, |el| el.opacity(0.45).cursor_default())
                        .child(if editor.fetching_models {
                            lucide_spinner("fetch_models_spin", 11.0, palette.foreground)
                                .into_any_element()
                        } else {
                            lucide_icon(IconName::RefreshCw, 11.0, palette.muted_foreground)
                                .into_any_element()
                        })
                        .child(
                            div()
                                .text_size(px(10.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.foreground)
                                .child(if editor.fetching_models {
                                    locale.pick("正在获取…", "Fetching…")
                                } else if is_subscription {
                                    locale.pick("同步账号模型", "Sync Models")
                                } else if editor.upstream_models.is_some() {
                                    locale.pick("刷新上游", "Refresh")
                                } else {
                                    locale.pick("获取上游模型", "Fetch Models")
                                }),
                        ),
                ),
        )
        // 2. 一体化紧凑模型池卡片容器（自适应高度 + max_h 136px 无轨道滚动）
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
                // 2.1 搜索与筛选工具栏（仅当池中有模型时显示，保持新建空态极度清爽）
                .when(!editor.model_pool.is_empty(), |card| {
                    card.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .px(px(6.0))
                            .py(px(5.0))
                            .border_b_1()
                            .border_color(palette.border_subtle)
                            .child(
                                div().flex_1().min_w_0().child(
                                    NativeTextInput::search(&editor.model_search_input, palette)
                                        .xs()
                                        .render(window, cx),
                                ),
                            )
                            .child(render_segmented_tabs(
                                vec![
                                    (
                                        ModelFilterTab::All,
                                        locale.pick("全部", "All").to_string(),
                                    ),
                                    (
                                        ModelFilterTab::Selected,
                                        locale.pick("已选", "On").to_string(),
                                    ),
                                    (
                                        ModelFilterTab::Unselected,
                                        locale.pick("未选", "Off").to_string(),
                                    ),
                                ],
                                editor.model_filter,
                                SegmentedTabsSize::Sm,
                                palette,
                                move |tab, _, app_cx| {
                                    app_entity.update(app_cx, |this, cx| {
                                        if let Some(ed) = this.editor.as_mut() {
                                            ed.model_filter = tab;
                                            cx.notify();
                                        }
                                    });
                                },
                            ))
                            .when(!filtered_models.is_empty(), |el| {
                                let batch_targets = filtered_models.clone();
                                let should_select = !all_filtered_selected;
                                el.child(
                                    NativeButton::new(
                                        if all_filtered_selected {
                                            locale.pick("清空", "Clear")
                                        } else {
                                            locale.pick("全选", "All")
                                        },
                                        palette,
                                    )
                                    .ghost()
                                    .xs()
                                    .h_px(26.0)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.editor_set_models_selected(
                                            batch_targets.clone(),
                                            should_select,
                                            cx,
                                        );
                                    })),
                                )
                            }),
                    )
                })
                // 2.2 家族快捷胶囊条
                .when(!family_groups.is_empty(), |el| {
                    let mut bar = div()
                        .flex()
                        .items_center()
                        .gap(px(4.5))
                        .flex_wrap()
                        .px(px(7.0))
                        .py(px(4.5))
                        .border_b_1()
                        .border_color(palette.border_subtle)
                        .bg(palette.muted_subtle);

                    for grp in family_groups {
                        let all_on = grp
                            .members
                            .iter()
                            .all(|m| selected_set.contains(m.as_str()));
                        let any_on = grp
                            .members
                            .iter()
                            .any(|m| selected_set.contains(m.as_str()));
                        let members = grp.members.clone();
                        let next_state = !all_on;

                        bar = bar.child(
                            div()
                                .id(SharedString::from(format!("family_chip_{}", grp.label)))
                                .px(px(6.5))
                                .py(px(1.5))
                                .rounded(px(99.0))
                                .cursor_pointer()
                                .border_1()
                                .border_color(if all_on {
                                    palette.primary
                                } else if any_on {
                                    palette.border
                                } else {
                                    palette.border_subtle
                                })
                                .bg(if all_on {
                                    palette.primary
                                } else {
                                    palette.card
                                })
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if all_on {
                                    palette.primary_foreground
                                } else {
                                    palette.muted_foreground
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.editor_set_models_selected(
                                            members.clone(),
                                            next_state,
                                            cx,
                                        );
                                    }),
                                )
                                .child(format!("{} ({})", grp.label, grp.members.len())),
                        );
                    }
                    el.child(bar)
                })
                // 2.3 模型自适应无轨道滚动列表（空状态轻量紧凑，多模型封顶 136px）
                .child(if editor.model_pool.is_empty() {
                    div()
                        .py(px(18.0))
                        .px(px(14.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(11.0))
                        .text_color(palette.muted_foreground)
                        .text_center()
                        .child(if is_subscription {
                            locale.pick(
                                "连接账号后将自动同步可用订阅模型",
                                "Connect an account to auto-sync subscription models",
                            )
                        } else {
                            locale.pick(
                                "点击右上角「获取上游模型」或在下方手动输入模型 ID",
                                "Click 'Fetch Models' above or add model IDs manually below",
                            )
                        })
                        .into_any_element()
                } else if filtered_models.is_empty() {
                    div()
                        .py(px(16.0))
                        .px(px(14.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(11.0))
                        .text_color(palette.muted_foreground)
                        .child(locale.pick("无匹配的模型", "No matching models"))
                        .into_any_element()
                } else {
                    let mut scroll_list = div()
                        .id("editor_model_pool_scroll")
                        .max_h(px(136.0))
                        .overflow_y_scroll()
                        .track_scroll(&editor.pool_scroll_handle)
                        .p(px(4.0))
                        .flex()
                        .flex_col()
                        .gap(px(1.5));

                    for (idx, model_id) in filtered_models.iter().enumerate() {
                        let is_checked = selected_set.contains(model_id.as_str());
                        let is_stale = upstream_set
                            .as_ref()
                            .map(|up| !up.contains(model_id.as_str()))
                            .unwrap_or(false);
                        let is_hovered =
                            editor.hovered_pool_model.as_deref() == Some(model_id.as_str());
                        let copy_key = format!("pool_copy_{}", model_id);
                        let is_copied = app.copied_id.as_deref() == Some(copy_key.as_str());

                        let display_label = app
                            .model_display_names
                            .get(model_id)
                            .cloned()
                            .unwrap_or_else(|| model_id.clone());

                        let m_toggle = model_id.clone();
                        let m_hover = model_id.clone();
                        let m_copy_id = copy_key.clone();
                        let m_copy_val = model_id.clone();
                        let m_delete = model_id.clone();

                        scroll_list = scroll_list.child(
                            div()
                                .id(SharedString::from(format!("pool_row_{}_{}", idx, model_id)))
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(6.0))
                                .h(px(26.0))
                                .px(px(8.0))
                                .rounded(px(5.0))
                                .flex_shrink_0()
                                .cursor_pointer()
                                .bg(if is_hovered {
                                    palette.accent
                                } else if is_checked {
                                    palette.muted_subtle
                                } else {
                                    rgba(0x00000000)
                                })
                                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                    if let Some(ed) = this.editor.as_mut() {
                                        if *hovered {
                                            ed.hovered_pool_model = Some(m_hover.clone());
                                        } else if ed.hovered_pool_model.as_deref()
                                            == Some(m_hover.as_str())
                                        {
                                            ed.hovered_pool_model = None;
                                        }
                                        cx.notify();
                                    }
                                }))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.editor_toggle_single_model(m_toggle.clone(), cx);
                                    }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(5.0))
                                        .min_w_0()
                                        .flex_1()
                                        // 精致圆角复选框
                                        .child(
                                            div()
                                                .size(px(13.0))
                                                .rounded(px(3.5))
                                                .flex_shrink_0()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .mr(px(2.0))
                                                .border_1()
                                                .border_color(if is_checked {
                                                    palette.primary
                                                } else {
                                                    palette.input
                                                })
                                                .bg(if is_checked {
                                                    palette.primary
                                                } else {
                                                    palette.background
                                                })
                                                .when(is_checked, |box_el| {
                                                    box_el.child(lucide_icon(
                                                        IconName::Check,
                                                        9.5,
                                                        palette.primary_foreground,
                                                    ))
                                                }),
                                        )
                                        // 单一模型名称（不附带冗余 ID，不设 flex_1 以便复制图标紧跟在文字后）
                                        .child(
                                            div()
                                                .min_w_0()
                                                .text_size(px(11.5))
                                                .font_weight(if is_checked {
                                                    FontWeight::MEDIUM
                                                } else {
                                                    FontWeight::NORMAL
                                                })
                                                .text_color(if is_checked {
                                                    palette.foreground
                                                } else {
                                                    palette.muted_foreground
                                                })
                                                .truncate()
                                                .child(display_label),
                                        )
                                        // 紧跟模型名称右侧的复制图标按钮
                                        .when(is_hovered || is_copied, |el| {
                                            el.child(
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "btn_copy_pool_{}",
                                                        idx
                                                    )))
                                                    .size(px(18.0))
                                                    .rounded(px(4.0))
                                                    .flex_shrink_0()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .cursor_pointer()
                                                    .hover(move |s| s.bg(palette.secondary))
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(move |this, _, _, cx| {
                                                            cx.stop_propagation();
                                                            this.copy_to_clipboard(
                                                                m_copy_id.clone(),
                                                                m_copy_val.clone(),
                                                                cx,
                                                            );
                                                        }),
                                                    )
                                                    .child(lucide_icon(
                                                        if is_copied {
                                                            IconName::Check
                                                        } else {
                                                            IconName::Copy
                                                        },
                                                        11.0,
                                                        if is_copied {
                                                            palette.emerald_text
                                                        } else {
                                                            palette.muted_foreground
                                                        },
                                                    )),
                                            )
                                        })
                                        .when(is_stale && is_checked, |el| {
                                            el.child(
                                                div()
                                                    .px(px(4.5))
                                                    .py(px(0.5))
                                                    .rounded(px(3.0))
                                                    .bg(palette.amber_soft_bg)
                                                    .text_size(px(9.5))
                                                    .text_color(palette.amber_text)
                                                    .flex_shrink_0()
                                                    .child(locale.pick("上游未见", "Unlisted")),
                                            )
                                        }),
                                )
                                // 行最右侧仅保留悬停删除按钮
                                .when(is_hovered, |el| {
                                    el.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "btn_del_pool_{}",
                                                idx
                                            )))
                                            .size(px(20.0))
                                            .rounded(px(4.0))
                                            .flex_shrink_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .hover(move |s| s.bg(palette.destructive_soft_bg))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.editor_delete_pool_model(
                                                        m_delete.clone(),
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .child(lucide_icon(
                                                IconName::Trash,
                                                11.0,
                                                palette.muted_foreground,
                                            )),
                                    )
                                }),
                        );
                    }

                    scroll_list.into_any_element()
                })
                // 2.4 底部手动添加自定义模型栏
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(6.0))
                        .py(px(5.0))
                        .border_t_1()
                        .border_color(palette.border_subtle)
                        .bg(palette.muted_subtle)
                        .child(
                            div().flex_1().min_w_0().child(
                                NativeTextInput::new(&editor.custom_model_input, palette)
                                    .compact()
                                    .cleanable(true)
                                    .render(window, cx),
                            ),
                        )
                        .child(
                            NativeButton::new(locale.pick("添加", "Add"), palette)
                                .outline()
                                .sm()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.editor_add_custom_model(window, cx);
                                })),
                        ),
                ),
        )
}
