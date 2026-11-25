use crate::components::button::{render_segmented_tabs, NativeButton, SegmentedTabsSize};
use crate::components::icons::{lucide_icon, lucide_spinner};
use crate::components::text_input::NativeTextInput;
use crate::theme::ThemePalette;
use crate::views::providers::editor::{ModelFilterTab, ProviderEditorState};
use crate::PivotApp;
use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pivot_core::model_catalog::{collapse_provider_models, LogicalModelEntry};
use std::collections::{HashMap, HashSet};

pub(crate) fn render_compact_tooltip(text: &str, window: &mut Window, cx: &mut App) -> AnyView {
    let label = text.to_string();
    Tooltip::element(move |_, _| {
        div()
            .text_size(px(10.5))
            .line_height(px(13.5))
            .child(label.clone())
    })
    .m(px(4.0))
    .px(px(6.5))
    .py(px(2.5))
    .rounded(px(5.0))
    .text_size(px(10.5))
    .build(window, cx)
}

struct FamilyGroup {
    label: &'static str,
    logical_count: usize,
    raw_members: Vec<String>,
}

type FamilySpec = (&'static str, fn(&str) -> bool);

#[derive(Debug, Clone, PartialEq, Eq)]
struct PoolEntry {
    logical: LogicalModelEntry,
    disambiguation: Option<String>,
}

fn build_pool_entries(logical_models: Vec<LogicalModelEntry>) -> Vec<PoolEntry> {
    let mut single_sig_counts: HashMap<
        (String, Option<pivot_core::model_catalog::ModelEffort>, Vec<String>),
        usize,
    > = HashMap::new();
    let mut group_sig_counts: HashMap<(String, Vec<String>), usize> = HashMap::new();

    for entry in &logical_models {
        if entry.variants.is_empty() {
            let sig = (
                entry.base_name.to_lowercase(),
                entry.single_effort,
                entry.extra_tags.clone(),
            );
            *single_sig_counts.entry(sig).or_insert(0) += 1;
        } else {
            let sig = (entry.base_name.to_lowercase(), entry.extra_tags.clone());
            *group_sig_counts.entry(sig).or_insert(0) += 1;
        }
    }

    logical_models
        .into_iter()
        .map(|entry| {
            let disambiguation = if entry.variants.is_empty() {
                let sig = (
                    entry.base_name.to_lowercase(),
                    entry.single_effort,
                    entry.extra_tags.clone(),
                );
                if single_sig_counts.get(&sig).copied().unwrap_or(0) > 1 {
                    Some(entry.logical_id.clone())
                } else {
                    None
                }
            } else {
                let sig = (entry.base_name.to_lowercase(), entry.extra_tags.clone());
                if group_sig_counts.get(&sig).copied().unwrap_or(0) > 1 {
                    Some(entry.logical_id.clone())
                } else {
                    None
                }
            };
            PoolEntry {
                logical: entry,
                disambiguation,
            }
        })
        .collect()
}

fn entry_matches_query(entry: &LogicalModelEntry, search_q: &str) -> bool {
    if search_q.is_empty() {
        return true;
    }
    if entry.logical_id.to_lowercase().contains(search_q)
        || entry.base_name.to_lowercase().contains(search_q)
    {
        return true;
    }
    if entry
        .extra_tags
        .iter()
        .any(|t| t.to_lowercase().contains(search_q))
    {
        return true;
    }
    if entry
        .single_effort
        .map(|e| {
            e.short_label().to_lowercase().contains(search_q)
                || e.full_label().to_lowercase().contains(search_q)
        })
        .unwrap_or(false)
    {
        return true;
    }
    entry.variants.iter().any(|v| {
        v.model_id.to_lowercase().contains(search_q)
            || v.effort.short_label().to_lowercase().contains(search_q)
            || v.effort.full_label().to_lowercase().contains(search_q)
    })
}

fn filter_pool_entries(
    entries: &[LogicalModelEntry],
    selected_set: &HashSet<&str>,
    filter: ModelFilterTab,
    search_q: &str,
) -> Vec<PoolEntry> {
    let matched: Vec<LogicalModelEntry> = entries
        .iter()
        .filter(|entry| {
            let any_sel = entry
                .raw_ids
                .iter()
                .any(|id| selected_set.contains(id.as_str()));
            match filter {
                ModelFilterTab::All => {}
                ModelFilterTab::Selected => {
                    if !any_sel {
                        return false;
                    }
                }
                ModelFilterTab::Unselected => {
                    if any_sel {
                        return false;
                    }
                }
            }
            entry_matches_query(entry, search_q)
        })
        .cloned()
        .collect();

    build_pool_entries(matched)
}

fn detect_family_groups(logical_pool: &[LogicalModelEntry]) -> Vec<FamilyGroup> {
    if logical_pool.len() < 5 {
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
        let mut logical_count = 0usize;
        let mut raw_members = Vec::new();
        for entry in logical_pool {
            if pred(&entry.logical_id.to_lowercase()) {
                logical_count += 1;
                raw_members.extend(entry.raw_ids.iter().cloned());
            }
        }
        if logical_count >= 2 {
            out.push(FamilyGroup {
                label,
                logical_count,
                raw_members,
            });
        }
    }
    out
}

fn render_compact_tag(tag: &str, palette: ThemePalette) -> Div {
    div()
        .px(px(4.5))
        .py(px(0.5))
        .rounded(px(3.5))
        .bg(palette.secondary)
        .text_size(px(9.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.muted_foreground)
        .flex_shrink_0()
        .child(tag.to_string())
}

fn render_effort_tag(tag: &str, palette: ThemePalette) -> Div {
    div()
        .min_w(px(40.0))
        .px(px(4.5))
        .py(px(0.5))
        .rounded(px(3.5))
        .bg(palette.secondary)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.muted_foreground)
        .flex_shrink_0()
        .child(tag.to_string())
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
    let logical_pool = collapse_provider_models(&editor.model_pool);
    let total_pool = logical_pool.len();
    let selected_set: HashSet<&str> = editor.selected_models.iter().map(|s| s.as_str()).collect();
    let selected_count = logical_pool
        .iter()
        .filter(|entry| {
            entry
                .raw_ids
                .iter()
                .any(|id| selected_set.contains(id.as_str()))
        })
        .count();

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

    let filtered_entries =
        filter_pool_entries(&logical_pool, &selected_set, editor.model_filter, &search_q);
    let filtered_raw_ids: Vec<String> = filtered_entries
        .iter()
        .flat_map(|pe| pe.logical.raw_ids.iter().cloned())
        .collect();
    let all_filtered_selected = !filtered_raw_ids.is_empty()
        && filtered_raw_ids
            .iter()
            .all(|m| selected_set.contains(m.as_str()));

    let family_groups = detect_family_groups(&logical_pool);
    let app_entity = cx.entity();

    let can_sync = if is_subscription {
        !editor.sub_accounts.is_empty() && !editor.fetching_models
    } else {
        !editor.fetching_models
    };

    let is_model_stale = |id: &str| -> bool {
        upstream_set
            .as_ref()
            .map(|up| !up.contains(id))
            .unwrap_or(false)
    };
    let is_model_no_account = |id: &str| -> bool {
        if !is_subscription {
            return false;
        }
        let known = editor.sub_accounts.iter().any(|acc| {
            acc.supported_models.is_some()
                && pivot_core::oauth::account_supports_model(acc, id)
        });
        known
            && !editor.sub_accounts.iter().any(|acc| {
                acc.enabled && pivot_core::oauth::account_supports_model(acc, id)
            })
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
                                .child(locale.pick("模型", "Models")),
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
        // 2. 一体化紧凑模型池卡片容器
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
                // 2.1 搜索与筛选工具栏
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
                            .when(!filtered_raw_ids.is_empty(), |el| {
                                let batch_targets = filtered_raw_ids.clone();
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
                            .raw_members
                            .iter()
                            .all(|m| selected_set.contains(m.as_str()));
                        let any_on = grp
                            .raw_members
                            .iter()
                            .any(|m| selected_set.contains(m.as_str()));
                        let members = grp.raw_members.clone();
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
                                .child(format!("{} ({})", grp.label, grp.logical_count)),
                        );
                    }
                    el.child(bar)
                })
                // 2.3 模型自适应无轨道滚动列表
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
                } else if filtered_entries.is_empty() {
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
                        .max_h(px(176.0))
                        .overflow_y_scroll()
                        .track_scroll(&editor.pool_scroll_handle)
                        .p(px(4.0))
                        .flex()
                        .flex_col()
                        .gap(px(1.5));

                    for (idx, pool_entry) in filtered_entries.into_iter().enumerate() {
                        let entry = pool_entry.logical;
                        let disambiguation = pool_entry.disambiguation;

                        if entry.variants.is_empty() {
                            let model_id = entry.logical_id;
                            let is_checked = selected_set.contains(model_id.as_str());
                            let is_stale = is_model_stale(&model_id);
                            let no_supporting_account = is_model_no_account(&model_id);
                            let is_hovered =
                                editor.hovered_pool_model.as_deref() == Some(model_id.as_str());
                            let copy_key = format!("pool_copy_{}", model_id);
                            let is_copied = app.copied_id.as_deref() == Some(copy_key.as_str());

                            let needs_copy_tip = entry.base_name != model_id
                                || entry.single_effort.is_some()
                                || !entry.extra_tags.is_empty();
                            let tip_id = model_id.clone();

                            let m_toggle = model_id.clone();
                            let m_hover = model_id.clone();
                            let m_copy_id = copy_key.clone();
                            let m_copy_val = model_id.clone();
                            let m_delete = model_id.clone();

                            let mut left_content = div()
                                .flex()
                                .items_center()
                                .gap(px(5.0))
                                .min_w_0()
                                .flex_1()
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
                                        .child(entry.base_name),
                                );

                            for tag in entry.extra_tags {
                                left_content =
                                    left_content.child(render_compact_tag(&tag, palette));
                            }

                            if let Some(eff) = entry.single_effort {
                                left_content = left_content
                                    .child(render_compact_tag(eff.short_label(), palette));
                            }

                            if let Some(disambig) = disambiguation {
                                left_content = left_content.child(
                                    div()
                                        .min_w_0()
                                        .text_size(px(10.0))
                                        .text_color(palette.subtext)
                                        .truncate()
                                        .child(disambig),
                                );
                            }

                            let mut right_content = div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .flex_shrink_0()
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
                                })
                                .when(
                                    is_checked && !is_stale && no_supporting_account,
                                    |el| {
                                        el.child(
                                            div()
                                                .px(px(4.5))
                                                .py(px(0.5))
                                                .rounded(px(3.0))
                                                .bg(palette.amber_soft_bg)
                                                .text_size(px(9.5))
                                                .text_color(palette.amber_text)
                                                .flex_shrink_0()
                                                .child(locale.pick(
                                                    "无可用账号",
                                                    "No active account",
                                                )),
                                        )
                                    },
                                )
                                .when(is_hovered || is_copied, |el| {
                                    el.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "btn_copy_pool_{}",
                                                idx
                                            )))
                                            .size(px(20.0))
                                            .rounded(px(4.0))
                                            .flex_shrink_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .hover(move |s| s.bg(palette.secondary))
                                            .when(needs_copy_tip, |btn| {
                                                btn.tooltip(move |w, cx| {
                                                    render_compact_tooltip(&tip_id, w, cx)
                                                })
                                            })
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
                                });

                            if is_hovered {
                                let del_tip = locale
                                    .pick("移除该模型", "Remove model")
                                    .to_string();
                                right_content = right_content.child(
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
                                        .tooltip(move |w, cx| {
                                            render_compact_tooltip(&del_tip, w, cx)
                                        })
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
                                );
                            }

                            scroll_list = scroll_list.child(
                                div()
                                    .id(SharedString::from(format!(
                                        "pool_row_{}_{}",
                                        idx, model_id
                                    )))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(6.0))
                                    .h(px(27.0))
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
                                    .on_hover(cx.listener(
                                        move |this, hovered: &bool, _, cx| {
                                            if let Some(ed) = this.editor.as_mut() {
                                                if *hovered {
                                                    ed.hovered_pool_model =
                                                        Some(m_hover.clone());
                                                } else if ed.hovered_pool_model.as_deref()
                                                    == Some(m_hover.as_str())
                                                {
                                                    ed.hovered_pool_model = None;
                                                }
                                                cx.notify();
                                            }
                                        },
                                    ))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.editor_toggle_single_model(
                                                m_toggle.clone(),
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(left_content)
                                    .child(right_content),
                            );
                        } else {
                            let group_ids = entry.raw_ids;
                            let checked_count = group_ids
                                .iter()
                                .filter(|id| selected_set.contains(id.as_str()))
                                .count();
                            let all_checked = checked_count == group_ids.len();
                            let any_checked = checked_count > 0;
                            let partial_checked = any_checked && !all_checked;

                            let expand_key = format!(
                                "{}|{}",
                                entry.logical_id,
                                entry.extra_tags.join(",")
                            );
                            let hover_key = format!("group:{}", expand_key);
                            let is_hovered =
                                editor.hovered_pool_model.as_deref() == Some(hover_key.as_str());
                            let is_expanded =
                                editor.expanded_pool_groups.contains(&expand_key);

                            let mut has_stale_checked = false;
                            let mut has_no_acc_checked = false;
                            for v in &entry.variants {
                                if selected_set.contains(v.model_id.as_str()) {
                                    if is_model_stale(&v.model_id) {
                                        has_stale_checked = true;
                                    } else if is_model_no_account(&v.model_id) {
                                        has_no_acc_checked = true;
                                    }
                                }
                            }
                            let warn_tip = if has_stale_checked {
                                Some(locale.pick("部分档位上游未见", "Some tiers unlisted").to_string())
                            } else if has_no_acc_checked {
                                Some(locale.pick("部分档位无可用账号", "Some tiers have no active account").to_string())
                            } else {
                                None
                            };

                            let group_logical_id = entry.logical_id;
                            let group_copy_key = format!("pool_copy_{}", group_logical_id);
                            let is_group_copied =
                                app.copied_id.as_deref() == Some(group_copy_key.as_str());
                            let needs_group_copy_tip = entry.base_name != group_logical_id
                                || !entry.extra_tags.is_empty();
                            let group_tip_id = group_logical_id.clone();
                            let group_copy_val = group_logical_id.clone();

                            let m_hover_enter = hover_key.clone();
                            let m_hover_leave = hover_key;
                            let group_ids_toggle = group_ids.clone();
                            let group_ids_del = group_ids;
                            let expand_key_btn = expand_key.clone();
                            let next_group_select = !all_checked;

                            let mut left_content = div()
                                .flex()
                                .items_center()
                                .gap(px(5.0))
                                .min_w_0()
                                .flex_1()
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
                                        .border_color(if any_checked {
                                            palette.primary
                                        } else {
                                            palette.input
                                        })
                                        .bg(if any_checked {
                                            palette.primary
                                        } else {
                                            palette.background
                                        })
                                        .when(all_checked, |box_el| {
                                            box_el.child(lucide_icon(
                                                IconName::Check,
                                                9.5,
                                                palette.primary_foreground,
                                            ))
                                        })
                                        .when(partial_checked, |box_el| {
                                            box_el.child(
                                                div()
                                                    .w(px(6.5))
                                                    .h(px(1.5))
                                                    .rounded(px(1.0))
                                                    .bg(palette.primary_foreground),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_size(px(11.5))
                                        .font_weight(if any_checked {
                                            FontWeight::MEDIUM
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(if any_checked {
                                            palette.foreground
                                        } else {
                                            palette.muted_foreground
                                        })
                                        .truncate()
                                        .child(entry.base_name),
                                );

                            for tag in entry.extra_tags {
                                left_content =
                                    left_content.child(render_compact_tag(&tag, palette));
                            }

                            if let Some(disambig) = disambiguation {
                                left_content = left_content.child(
                                    div()
                                        .min_w_0()
                                        .text_size(px(10.0))
                                        .text_color(palette.subtext)
                                        .truncate()
                                        .child(disambig),
                                );
                            }

                            left_content = left_content.child(
                                div()
                                    .id(SharedString::from(format!(
                                        "btn_expand_group_{}",
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
                                            this.editor_toggle_pool_group_expanded(
                                                expand_key_btn.clone(),
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(lucide_icon(
                                        if is_expanded {
                                            IconName::ChevronUp
                                        } else {
                                            IconName::ChevronDown
                                        },
                                        11.0,
                                        if is_hovered || is_expanded {
                                            palette.muted_foreground
                                        } else {
                                            palette.subtext
                                        },
                                    )),
                            );

                            let mut right_content = div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .flex_shrink_0();

                            if let Some(wtip) = warn_tip {
                                right_content = right_content.child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "group_warn_dot_{}",
                                            idx
                                        )))
                                        .size(px(5.0))
                                        .rounded_full()
                                        .bg(palette.amber_text)
                                        .tooltip(move |w, cx| {
                                            render_compact_tooltip(&wtip, w, cx)
                                        }),
                                );
                            }

                            right_content = right_content.when(
                                is_hovered || is_group_copied,
                                |el| {
                                    el.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "btn_copy_group_{}",
                                                idx
                                            )))
                                            .size(px(20.0))
                                            .rounded(px(4.0))
                                            .flex_shrink_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .hover(move |s| s.bg(palette.secondary))
                                            .when(needs_group_copy_tip, |btn| {
                                                btn.tooltip(move |w, cx| {
                                                    render_compact_tooltip(
                                                        &group_tip_id,
                                                        w,
                                                        cx,
                                                    )
                                                })
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.copy_to_clipboard(
                                                        group_copy_key.clone(),
                                                        group_copy_val.clone(),
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .child(lucide_icon(
                                                if is_group_copied {
                                                    IconName::Check
                                                } else {
                                                    IconName::Copy
                                                },
                                                11.0,
                                                if is_group_copied {
                                                    palette.emerald_text
                                                } else {
                                                    palette.muted_foreground
                                                },
                                            )),
                                    )
                                },
                            );

                            if is_hovered {
                                let del_tip = locale
                                    .pick("移除该组模型", "Remove group")
                                    .to_string();
                                right_content = right_content.child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "btn_del_group_{}",
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
                                        .tooltip(move |w, cx| {
                                            render_compact_tooltip(&del_tip, w, cx)
                                        })
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.editor_delete_pool_models(
                                                    group_ids_del.clone(),
                                                    cx,
                                                );
                                            }),
                                        )
                                        .child(lucide_icon(
                                            IconName::Trash,
                                            11.0,
                                            palette.muted_foreground,
                                        )),
                                );
                            }

                            scroll_list = scroll_list.child(
                                div()
                                    .id(SharedString::from(format!(
                                        "pool_group_row_{}_{}",
                                        idx, group_logical_id
                                    )))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(6.0))
                                    .h(px(27.0))
                                    .px(px(8.0))
                                    .rounded(px(5.0))
                                    .flex_shrink_0()
                                    .cursor_pointer()
                                    .bg(if is_hovered {
                                        palette.accent
                                    } else if any_checked {
                                        palette.muted_subtle
                                    } else {
                                        rgba(0x00000000)
                                    })
                                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                        if let Some(ed) = this.editor.as_mut() {
                                            if *hovered {
                                                ed.hovered_pool_model =
                                                    Some(m_hover_enter.clone());
                                            } else if ed.hovered_pool_model.as_deref()
                                                == Some(m_hover_leave.as_str())
                                            {
                                                ed.hovered_pool_model = None;
                                            }
                                            cx.notify();
                                        }
                                    }))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.editor_set_models_selected(
                                                group_ids_toggle.clone(),
                                                next_group_select,
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(left_content)
                                    .child(right_content),
                            );

                            if is_expanded {
                                let mut sub_list = div()
                                    .ml(px(14.0))
                                    .pl(px(8.0))
                                    .border_l_1()
                                    .border_color(palette.border_subtle)
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.0));

                                for (v_idx, variant) in entry.variants.iter().enumerate() {
                                    let is_sub_checked =
                                        selected_set.contains(variant.model_id.as_str());
                                    let sub_stale = is_model_stale(&variant.model_id);
                                    let sub_no_acc = is_model_no_account(&variant.model_id);
                                    let sub_hover_key =
                                        format!("sub:{}|{}", expand_key, variant.model_id);
                                    let is_sub_hovered = editor.hovered_pool_model.as_deref()
                                        == Some(sub_hover_key.as_str());
                                    let sub_hover_enter = sub_hover_key.clone();
                                    let sub_hover_leave = sub_hover_key;
                                    let sub_copy_key = format!("pool_copy_{}", variant.model_id);
                                    let is_sub_copied =
                                        app.copied_id.as_deref() == Some(sub_copy_key.as_str());
                                    let m_sub_copy_val = variant.model_id.clone();

                                    let sub_left = div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.0))
                                        .min_w_0()
                                        .flex_1()
                                        .child(render_effort_tag(
                                            variant.effort.short_label(),
                                            palette,
                                        ))
                                        .child(
                                            div()
                                                .min_w_0()
                                                .text_size(px(10.5))
                                                .text_color(palette.muted_foreground)
                                                .truncate()
                                                .child(variant.model_id.clone()),
                                        )
                                        .when(sub_stale && is_sub_checked, |el| {
                                            el.child(
                                                div()
                                                    .px(px(4.0))
                                                    .py(px(0.5))
                                                    .rounded(px(3.0))
                                                    .bg(palette.amber_soft_bg)
                                                    .text_size(px(9.0))
                                                    .text_color(palette.amber_text)
                                                    .flex_shrink_0()
                                                    .child(locale.pick("上游未见", "Unlisted")),
                                            )
                                        })
                                        .when(
                                            is_sub_checked && !sub_stale && sub_no_acc,
                                            |el| {
                                                el.child(
                                                    div()
                                                        .px(px(4.0))
                                                        .py(px(0.5))
                                                        .rounded(px(3.0))
                                                        .bg(palette.amber_soft_bg)
                                                        .text_size(px(9.0))
                                                        .text_color(palette.amber_text)
                                                        .flex_shrink_0()
                                                        .child(locale.pick(
                                                            "无可用账号",
                                                            "No active account",
                                                        )),
                                                )
                                            },
                                        );

                                    sub_list = sub_list.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "pool_sub_row_{}_{}_{}",
                                                idx, v_idx, variant.model_id
                                            )))
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap(px(6.0))
                                            .h(px(22.0))
                                            .px(px(6.0))
                                            .rounded(px(4.0))
                                            .flex_shrink_0()
                                            .cursor_pointer()
                                            .bg(if is_sub_hovered {
                                                palette.accent
                                            } else {
                                                rgba(0x00000000)
                                            })
                                            .on_hover(cx.listener(
                                                move |this, hovered: &bool, _, cx| {
                                                    if let Some(ed) = this.editor.as_mut() {
                                                        if *hovered {
                                                            ed.hovered_pool_model =
                                                                Some(sub_hover_enter.clone());
                                                        } else if ed.hovered_pool_model.as_deref()
                                                            == Some(sub_hover_leave.as_str())
                                                        {
                                                            ed.hovered_pool_model = None;
                                                        }
                                                        cx.notify();
                                                    }
                                                },
                                            ))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.copy_to_clipboard(
                                                        sub_copy_key.clone(),
                                                        m_sub_copy_val.clone(),
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .child(sub_left)
                                            .when(is_sub_hovered || is_sub_copied, |el| {
                                                el.child(
                                                    div()
                                                        .size(px(20.0))
                                                        .rounded(px(4.0))
                                                        .flex_shrink_0()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(lucide_icon(
                                                            if is_sub_copied {
                                                                IconName::Check
                                                            } else {
                                                                IconName::Copy
                                                            },
                                                            11.0,
                                                            if is_sub_copied {
                                                                palette.emerald_text
                                                            } else {
                                                                palette.muted_foreground
                                                            },
                                                        )),
                                                )
                                            }),
                                    );
                                }

                                scroll_list = scroll_list.child(sub_list);
                            }
                        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use pivot_core::model_catalog::ModelEffort;

    #[::core::prelude::v1::test]
    fn logical_pool_counts_collapses_variants_and_searches_efforts() {
        let pool: Vec<String> = vec![
            "gemini-4-neon-low",
            "gemini-4-neon-medium",
            "gemini-4-neon-high",
            "gemini-4-neon-xhigh",
            "claude-opus-4-6-thinking",
            "gpt-oss-120b-medium",
            "custom-foo-low",
            "custom-foo-high",
            "custom-foo-max",
            "qwen-max",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let logical = collapse_provider_models(&pool);
        // 4 neon -> 1, opus-4-6-thinking -> 1, gpt-oss -> 1, 3 custom-foo -> 1, qwen-max -> 1 => 5 logical models
        assert_eq!(logical.len(), 5);

        let selected: HashSet<&str> = ["gemini-4-neon-xhigh"].into_iter().collect();
        let filtered_xhigh = filter_pool_entries(&logical, &selected, ModelFilterTab::All, "xhigh");
        assert_eq!(filtered_xhigh.len(), 1);
        assert_eq!(filtered_xhigh[0].logical.logical_id, "gemini-4-neon");

        let filtered_max = filter_pool_entries(&logical, &selected, ModelFilterTab::All, "max");
        assert_eq!(filtered_max.len(), 2);
        assert_eq!(filtered_max[0].logical.logical_id, "custom-foo");
        assert_eq!(filtered_max[1].logical.logical_id, "qwen-max");

        let filtered_on = filter_pool_entries(&logical, &selected, ModelFilterTab::Selected, "");
        assert_eq!(filtered_on.len(), 1);
        assert_eq!(filtered_on[0].logical.logical_id, "gemini-4-neon");
    }

    #[::core::prelude::v1::test]
    fn build_pool_entries_disambiguates_identical_display_names() {
        let entries = vec![
            LogicalModelEntry {
                logical_id: "claude-3-5-sonnet-20240620".to_string(),
                base_name: "Claude 3.5 Sonnet".to_string(),
                extra_tags: Vec::new(),
                single_effort: None,
                variants: Vec::new(),
                raw_ids: vec!["claude-3-5-sonnet-20240620".to_string()],
            },
            LogicalModelEntry {
                logical_id: "claude-3-5-sonnet-20241022".to_string(),
                base_name: "Claude 3.5 Sonnet".to_string(),
                extra_tags: Vec::new(),
                single_effort: None,
                variants: Vec::new(),
                raw_ids: vec!["claude-3-5-sonnet-20241022".to_string()],
            },
            LogicalModelEntry {
                logical_id: "gpt-oss-120b-medium".to_string(),
                base_name: "GPT-OSS 120B".to_string(),
                extra_tags: Vec::new(),
                single_effort: Some(ModelEffort::Medium),
                variants: Vec::new(),
                raw_ids: vec!["gpt-oss-120b-medium".to_string()],
            },
        ];

        let built = build_pool_entries(entries);
        assert_eq!(
            built[0].disambiguation.as_deref(),
            Some("claude-3-5-sonnet-20240620")
        );
        assert_eq!(
            built[1].disambiguation.as_deref(),
            Some("claude-3-5-sonnet-20241022")
        );
        assert_eq!(built[2].disambiguation, None);
    }
}
