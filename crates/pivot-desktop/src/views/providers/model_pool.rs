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
use pivot_core::model_catalog::{ModelDisplayInfo, ModelEffort};
use std::collections::{HashMap, HashSet};

struct FamilyGroup {
    label: &'static str,
    members: Vec<String>,
}

type FamilySpec = (&'static str, fn(&str) -> bool);

#[derive(Debug, Clone, PartialEq, Eq)]
struct VariantItem {
    model_id: String,
    effort: ModelEffort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PoolRow {
    VariantGroup {
        group_key: String,
        base_name: String,
        extra_tags: Vec<String>,
        variants: Vec<VariantItem>,
        disambiguation: Option<String>,
    },
    Single {
        model_id: String,
        base_name: String,
        effort: Option<ModelEffort>,
        extra_tags: Vec<String>,
        disambiguation: Option<String>,
    },
}

fn build_pool_rows(
    models: &[String],
    resolve_info: impl Fn(&str) -> ModelDisplayInfo,
) -> Vec<PoolRow> {
    let mut slots: Vec<Vec<(String, ModelDisplayInfo)>> = Vec::new();
    let mut slot_map: HashMap<(String, Vec<String>), usize> = HashMap::new();

    for m in models {
        let info = resolve_info(m);
        let key = (info.group_key.clone(), info.extra_tags.clone());
        if let Some(&idx) = slot_map.get(&key) {
            slots[idx].push((m.clone(), info));
        } else {
            let idx = slots.len();
            slot_map.insert(key, idx);
            slots.push(vec![(m.clone(), info)]);
        }
    }

    let mut rows = Vec::new();
    for members in slots {
        let eff_members: Vec<&(String, ModelDisplayInfo)> = members
            .iter()
            .filter(|(_, info)| info.effort.is_some())
            .collect();
        let unique_efforts: HashSet<ModelEffort> = eff_members
            .iter()
            .filter_map(|(_, info)| info.effort)
            .collect();

        let can_group_efforts =
            eff_members.len() >= 2 && unique_efforts.len() == eff_members.len();

        if can_group_efforts {
            let mut variants: Vec<VariantItem> = eff_members
                .iter()
                .filter_map(|(id, info)| {
                    info.effort.map(|effort| VariantItem {
                        model_id: id.clone(),
                        effort,
                    })
                })
                .collect();
            variants.sort_by_key(|v| v.effort);

            let first_eff = &eff_members[0].1;
            let group_row = PoolRow::VariantGroup {
                group_key: first_eff.group_key.clone(),
                base_name: first_eff.base_name.clone(),
                extra_tags: first_eff.extra_tags.clone(),
                variants,
                disambiguation: None,
            };

            let mut emitted_group = false;
            for (id, info) in members {
                if info.effort.is_none() {
                    rows.push(PoolRow::Single {
                        model_id: id,
                        base_name: info.base_name,
                        effort: None,
                        extra_tags: info.extra_tags,
                        disambiguation: None,
                    });
                } else if !emitted_group {
                    rows.push(group_row.clone());
                    emitted_group = true;
                }
            }
        } else {
            for (id, info) in members {
                rows.push(PoolRow::Single {
                    model_id: id,
                    base_name: info.base_name,
                    effort: info.effort,
                    extra_tags: info.extra_tags,
                    disambiguation: None,
                });
            }
        }
    }

    // 列表级同名冲突自动消歧：绝不允许任意两行呈现完全一致的视觉文本
    let mut single_sig_counts: HashMap<(String, Option<ModelEffort>, Vec<String>), usize> =
        HashMap::new();
    let mut group_sig_counts: HashMap<(String, Vec<String>), usize> = HashMap::new();

    for row in &rows {
        match row {
            PoolRow::Single {
                base_name,
                effort,
                extra_tags,
                ..
            } => {
                let sig = (base_name.to_lowercase(), *effort, extra_tags.clone());
                *single_sig_counts.entry(sig).or_insert(0) += 1;
            }
            PoolRow::VariantGroup {
                base_name,
                extra_tags,
                ..
            } => {
                let sig = (base_name.to_lowercase(), extra_tags.clone());
                *group_sig_counts.entry(sig).or_insert(0) += 1;
            }
        }
    }

    for row in &mut rows {
        match row {
            PoolRow::Single {
                model_id,
                base_name,
                effort,
                extra_tags,
                disambiguation,
            } => {
                let sig = (base_name.to_lowercase(), *effort, extra_tags.clone());
                if single_sig_counts.get(&sig).copied().unwrap_or(0) > 1 {
                    *disambiguation = Some(model_id.clone());
                }
            }
            PoolRow::VariantGroup {
                group_key,
                base_name,
                extra_tags,
                disambiguation,
                ..
            } => {
                let sig = (base_name.to_lowercase(), extra_tags.clone());
                if group_sig_counts.get(&sig).copied().unwrap_or(0) > 1 {
                    *disambiguation = Some(group_key.clone());
                }
            }
        }
    }

    rows
}

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

    let mut info_cache: HashMap<String, ModelDisplayInfo> =
        HashMap::with_capacity(editor.model_pool.len());
    for m in &editor.model_pool {
        info_cache.insert(m.clone(), app.model_display_info(m));
    }

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
            let info = info_cache
                .get(*m)
                .cloned()
                .unwrap_or_else(|| app.model_display_info(m));
            let match_id = m.to_lowercase().contains(&search_q);
            let match_disp = info.display_name.to_lowercase().contains(&search_q);
            let match_base = info.base_name.to_lowercase().contains(&search_q);
            let match_effort = info
                .effort
                .map(|e| {
                    e.short_label().to_lowercase().contains(&search_q)
                        || e.full_label().to_lowercase().contains(&search_q)
                })
                .unwrap_or(false);
            let match_tags = info
                .extra_tags
                .iter()
                .any(|t| t.to_lowercase().contains(&search_q));
            match_id || match_disp || match_base || match_effort || match_tags
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
        // 2. 一体化紧凑模型池卡片容器（自适应高度 + max_h 176px 无轨道滚动）
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
                    let pool_rows = build_pool_rows(&filtered_models, |id| {
                        info_cache
                            .get(id)
                            .cloned()
                            .unwrap_or_else(|| app.model_display_info(id))
                    });

                    let mut scroll_list = div()
                        .id("editor_model_pool_scroll")
                        .max_h(px(176.0))
                        .overflow_y_scroll()
                        .track_scroll(&editor.pool_scroll_handle)
                        .p(px(4.0))
                        .flex()
                        .flex_col()
                        .gap(px(1.5));

                    for (idx, row) in pool_rows.into_iter().enumerate() {
                        match row {
                            PoolRow::VariantGroup {
                                group_key,
                                base_name,
                                extra_tags,
                                variants,
                                disambiguation,
                            } => {
                                let group_ids: Vec<String> =
                                    variants.iter().map(|v| v.model_id.clone()).collect();
                                let checked_count = group_ids
                                    .iter()
                                    .filter(|id| selected_set.contains(id.as_str()))
                                    .count();
                                let all_checked = checked_count == group_ids.len();
                                let any_checked = checked_count > 0;
                                let partial_checked = any_checked && !all_checked;

                                let hover_key = format!("group:{}", group_key);
                                let is_hovered =
                                    editor.hovered_pool_model.as_deref() == Some(hover_key.as_str());
                                let wrap_pills = (base_name.chars().count() > 22
                                    && variants.len() >= 4)
                                    || (disambiguation.is_some() && variants.len() >= 3);

                                let m_hover_enter = hover_key.clone();
                                let m_hover_leave = hover_key;
                                let group_ids_toggle = group_ids.clone();
                                let next_group_select = !all_checked;

                                let mut pills_row = div()
                                    .flex()
                                    .items_center()
                                    .gap(px(3.5))
                                    .flex_shrink_0();

                                for (v_idx, variant) in variants.iter().enumerate() {
                                    let is_v_checked =
                                        selected_set.contains(variant.model_id.as_str());
                                    let v_stale = is_model_stale(&variant.model_id);
                                    let v_no_acc = is_model_no_account(&variant.model_id);
                                    let has_warn = is_v_checked && (v_stale || v_no_acc);

                                    let copy_key = format!("pool_copy_{}", variant.model_id);
                                    let is_v_copied =
                                        app.copied_id.as_deref() == Some(copy_key.as_str());

                                    let tip_text = if is_v_checked && v_stale {
                                        format!(
                                            "{} · {}",
                                            variant.model_id,
                                            locale.pick("上游未见", "Unlisted")
                                        )
                                    } else if is_v_checked && v_no_acc {
                                        format!(
                                            "{} · {}",
                                            variant.model_id,
                                            locale.pick("无可用账号", "No active account")
                                        )
                                    } else {
                                        variant.model_id.clone()
                                    };

                                    let m_toggle_v = variant.model_id.clone();
                                    let m_copy_val = variant.model_id.clone();
                                    let m_copy_key = copy_key;

                                    let copied_check_color = if is_v_checked {
                                        if palette.is_dark {
                                            rgb(0x059669)
                                        } else {
                                            rgb(0x34d399)
                                        }
                                    } else {
                                        palette.emerald_text
                                    };

                                    pills_row = pills_row.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "pool_var_{}_{}",
                                                idx, v_idx
                                            )))
                                            .relative()
                                            .h(px(19.0))
                                            .px(px(5.5))
                                            .rounded(px(4.0))
                                            .border_1()
                                            .flex()
                                            .items_center()
                                            .gap(px(3.0))
                                            .cursor_pointer()
                                            .border_color(if is_v_checked {
                                                palette.primary
                                            } else {
                                                palette.border
                                            })
                                            .bg(if is_v_checked {
                                                palette.primary
                                            } else {
                                                palette.background
                                            })
                                            .text_size(px(10.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(if is_v_checked {
                                                palette.primary_foreground
                                            } else {
                                                palette.muted_foreground
                                            })
                                            .when(!is_v_checked, |el| {
                                                el.hover(move |s| {
                                                    s.bg(palette.secondary)
                                                        .text_color(palette.foreground)
                                                })
                                            })
                                            .tooltip(move |w, cx| {
                                                Tooltip::new(tip_text.clone()).build(w, cx)
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.editor_toggle_single_model(
                                                        m_toggle_v.clone(),
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.copy_to_clipboard(
                                                        m_copy_key.clone(),
                                                        m_copy_val.clone(),
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(3.0))
                                                    .when(is_v_copied, |el| el.opacity(0.0))
                                                    .child(variant.effort.short_label())
                                                    .when(has_warn, |el| {
                                                        el.child(
                                                            div()
                                                                .size(px(4.5))
                                                                .rounded_full()
                                                                .bg(palette.amber_text),
                                                        )
                                                    }),
                                            )
                                            .when(is_v_copied, |el| {
                                                el.child(
                                                    div()
                                                        .absolute()
                                                        .inset_0()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(lucide_icon(
                                                            IconName::Check,
                                                            10.0,
                                                            copied_check_color,
                                                        )),
                                                )
                                            }),
                                    );
                                }

                                let mut header_left = div()
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
                                            .child(base_name),
                                    );

                                for tag in extra_tags {
                                    header_left = header_left.child(render_compact_tag(&tag, palette));
                                }

                                if let Some(ref disambig) = disambiguation {
                                    header_left = header_left.child(
                                        div()
                                            .min_w_0()
                                            .text_size(px(10.0))
                                            .text_color(palette.subtext)
                                            .truncate()
                                            .child(disambig.clone()),
                                    );
                                }

                                let row_el = div()
                                    .id(SharedString::from(format!(
                                        "pool_group_row_{}_{}",
                                        idx, group_key
                                    )))
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
                                    );

                                let row_el = if wrap_pills {
                                    row_el
                                        .py(px(5.0))
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .min_w_0()
                                                .child(header_left),
                                        )
                                        .child(
                                            div()
                                                .pl(px(20.0))
                                                .flex()
                                                .items_center()
                                                .child(pills_row),
                                        )
                                } else {
                                    row_el
                                        .min_h(px(28.0))
                                        .py(px(3.0))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(6.0))
                                        .child(header_left)
                                        .child(pills_row)
                                };

                                scroll_list = scroll_list.child(row_el);
                            }
                            PoolRow::Single {
                                model_id,
                                base_name,
                                effort,
                                extra_tags,
                                disambiguation,
                            } => {
                                let is_checked = selected_set.contains(model_id.as_str());
                                let is_stale = is_model_stale(&model_id);
                                let no_supporting_account = is_model_no_account(&model_id);
                                let is_hovered = editor.hovered_pool_model.as_deref()
                                    == Some(model_id.as_str());
                                let copy_key = format!("pool_copy_{}", model_id);
                                let is_copied =
                                    app.copied_id.as_deref() == Some(copy_key.as_str());

                                let needs_tooltip = base_name != model_id
                                    || effort.is_some()
                                    || !extra_tags.is_empty();
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
                                            .child(base_name),
                                    );

                                for tag in extra_tags {
                                    left_content =
                                        left_content.child(render_compact_tag(&tag, palette));
                                }

                                if let Some(eff) = effort {
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

                                left_content = left_content
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
                                    );

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
                                        .when(needs_tooltip, |el| {
                                            el.tooltip(move |w, cx| {
                                                Tooltip::new(tip_id.clone()).build(w, cx)
                                            })
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
                                                    .hover(move |s| {
                                                        s.bg(palette.destructive_soft_bg)
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
                                            )
                                        }),
                                );
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

    fn test_resolve(id: &str) -> ModelDisplayInfo {
        let base_map = |stem: &str| -> Option<&'static str> {
            match stem {
                "gemini-4-neon" => Some("Gemini 4 Neon"),
                "gemini-4-hydrogen" => Some("Gemini 4 Hydrogen"),
                "gemini-4-hydrogen-tiered" => Some("Gemini 4 Hydrogen Tiered"),
                "gemini-4-argon" => Some("Gemini 4 Argon"),
                "gemini-4-carbon" => Some("Gemini 4 Carbon"),
                "gemini-3.8-flash" => Some("Gemini 3.8 Flash"),
                "claude-sonnet-5-5" => Some("Claude Sonnet 5.5"),
                "claude-opus-5-5" => Some("Claude Opus 5.5"),
                "claude-opus-4-6-thinking" => Some("Claude Opus 4.6 Thinking"),
                "claude-sonnet-4-6" => Some("Claude Sonnet 4.6"),
                "gpt-oss-120b" => Some("GPT-OSS 120B"),
                _ => None,
            }
        };

        let (stem, effort) = match ModelEffort::from_suffix(id) {
            Some((s, e)) => (s, Some(e)),
            None => (id, None),
        };
        let base_name = base_map(stem).unwrap_or(stem).to_string();
        let display_name = match effort {
            Some(e) => format!("{} ({})", base_name, e.full_label()),
            None => base_name.clone(),
        };
        ModelDisplayInfo {
            raw_id: id.to_string(),
            group_key: stem.to_lowercase(),
            base_name,
            effort,
            extra_tags: Vec::new(),
            display_name,
        }
    }

    #[::core::prelude::v1::test]
    fn antigravity_pool_groups_and_sorts_efforts_while_keeping_single_effort_single() {
        let pool: Vec<String> = vec![
            "claude-opus-4-6-thinking",
            "gemini-3.8-flash-high",
            "gemini-4-hydrogen-low",
            "gemini-4-hydrogen-high",
            "claude-sonnet-4-6",
            "gpt-oss-120b-medium",
            "gemini-3.8-flash-low",
            "gemini-3.8-flash-medium",
            "claude-sonnet-5-5-low",
            "claude-sonnet-5-5-medium",
            "claude-sonnet-5-5-high",
            "gemini-4-hydrogen-medium",
            "gemini-4-hydrogen-xhigh",
            "gemini-4-neon-medium",
            "gemini-4-neon-xhigh",
            "gemini-4-neon-low",
            "gemini-4-hydrogen-tiered-xhigh",
            "gemini-4-neon-high",
            "gemini-4-hydrogen-tiered-high",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let rows = build_pool_rows(&pool, test_resolve);

        // 7 rows: claude-opus-4-6-thinking (Single), gemini-3.8-flash (Group 3),
        // gemini-4-hydrogen (Group 4), claude-sonnet-4-6 (Single), gpt-oss-120b-medium (Single),
        // claude-sonnet-5-5 (Group 3), gemini-4-neon (Group 4), gemini-4-hydrogen-tiered (Group 2) -> 8 rows
        assert_eq!(rows.len(), 8);

        // 首次出现顺序聚合：gemini-3.8-flash 在 index 1
        match &rows[1] {
            PoolRow::VariantGroup {
                group_key,
                base_name,
                variants,
                ..
            } => {
                assert_eq!(group_key, "gemini-3.8-flash");
                assert_eq!(base_name, "Gemini 3.8 Flash");
                let efforts: Vec<ModelEffort> = variants.iter().map(|v| v.effort).collect();
                assert_eq!(
                    efforts,
                    vec![ModelEffort::Low, ModelEffort::Medium, ModelEffort::High]
                );
            }
            other => panic!("expected VariantGroup, got {other:?}"),
        }

        // 单成员 gpt-oss-120b-medium 不合组
        assert_eq!(
            rows[4],
            PoolRow::Single {
                model_id: "gpt-oss-120b-medium".to_string(),
                base_name: "GPT-OSS 120B".to_string(),
                effort: Some(ModelEffort::Medium),
                extra_tags: Vec::new(),
                disambiguation: None,
            }
        );

        // gemini-4-neon 按 Low -> Medium -> High -> XHigh 排序
        match &rows[6] {
            PoolRow::VariantGroup { variants, .. } => {
                let ids: Vec<&str> = variants.iter().map(|v| v.model_id.as_str()).collect();
                assert_eq!(
                    ids,
                    vec![
                        "gemini-4-neon-low",
                        "gemini-4-neon-medium",
                        "gemini-4-neon-high",
                        "gemini-4-neon-xhigh",
                    ]
                );
            }
            other => panic!("expected VariantGroup for neon, got {other:?}"),
        }

        // gemini-4-hydrogen-tiered 独立成组，不与 gemini-4-hydrogen 混淆
        match &rows[7] {
            PoolRow::VariantGroup {
                base_name,
                variants,
                ..
            } => {
                assert_eq!(base_name, "Gemini 4 Hydrogen Tiered");
                let ids: Vec<&str> = variants.iter().map(|v| v.model_id.as_str()).collect();
                assert_eq!(
                    ids,
                    vec![
                        "gemini-4-hydrogen-tiered-high",
                        "gemini-4-hydrogen-tiered-xhigh",
                    ]
                );
            }
            other => panic!("expected VariantGroup for tiered, got {other:?}"),
        }
    }

    #[::core::prelude::v1::test]
    fn filtered_single_variant_renders_as_single_with_effort_badge() {
        let filtered = vec!["gemini-4-neon-xhigh".to_string()];
        let rows = build_pool_rows(&filtered, test_resolve);
        assert_eq!(
            rows,
            vec![PoolRow::Single {
                model_id: "gemini-4-neon-xhigh".to_string(),
                base_name: "Gemini 4 Neon".to_string(),
                effort: Some(ModelEffort::XHigh),
                extra_tags: Vec::new(),
                disambiguation: None,
            }]
        );
    }

    #[::core::prelude::v1::test]
    fn collision_disambiguates_identical_display_rows() {
        let models = vec![
            "claude-3-5-sonnet-20240620".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
        ];
        let rows = build_pool_rows(&models, |id| ModelDisplayInfo {
            raw_id: id.to_string(),
            group_key: id.to_string(),
            base_name: "Claude 3.5 Sonnet".to_string(),
            effort: None,
            extra_tags: Vec::new(),
            display_name: "Claude 3.5 Sonnet".to_string(),
        });
        assert_eq!(rows.len(), 2);
        match (&rows[0], &rows[1]) {
            (
                PoolRow::Single {
                    disambiguation: d1, ..
                },
                PoolRow::Single {
                    disambiguation: d2, ..
                },
            ) => {
                assert_eq!(d1.as_deref(), Some("claude-3-5-sonnet-20240620"));
                assert_eq!(d2.as_deref(), Some("claude-3-5-sonnet-20241022"));
            }
            _ => panic!("expected two Single rows with disambiguation"),
        }
    }

    #[::core::prelude::v1::test]
    fn mixed_base_and_effort_group_splits_cleanly_without_conflict() {
        let models = vec![
            "claude-opus-5-5".to_string(),
            "claude-opus-5-5-high".to_string(),
            "claude-opus-5-5-low".to_string(),
        ];
        let rows = build_pool_rows(&models, test_resolve);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0],
            PoolRow::Single {
                model_id: "claude-opus-5-5".to_string(),
                base_name: "Claude Opus 5.5".to_string(),
                effort: None,
                extra_tags: Vec::new(),
                disambiguation: None,
            }
        );
        match &rows[1] {
            PoolRow::VariantGroup { variants, .. } => {
                let ids: Vec<&str> = variants.iter().map(|v| v.model_id.as_str()).collect();
                assert_eq!(ids, vec!["claude-opus-5-5-low", "claude-opus-5-5-high"]);
            }
            other => panic!("expected VariantGroup, got {other:?}"),
        }
    }
}
