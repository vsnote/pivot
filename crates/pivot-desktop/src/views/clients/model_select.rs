use crate::components::icons::lucide_icon;
use crate::components::text_input::NativeTextInput;
use crate::theme::ThemePalette;
use crate::PivotApp;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pivot_core::db::Provider;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct GroupedModelOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct GroupedModel {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub preset_id: Option<String>,
    pub models: Vec<GroupedModelOption>,
}

pub struct ClientModelSelectState {
    pub client_id: String,
    pub current_value: String,
    pub trigger_bounds: Bounds<Pixels>,
    pub search_input: Entity<InputState>,
    pub scroll_handle: ScrollHandle,
}

fn provider_slug_prefix(name: &str) -> String {
    pivot_core::gateway::router::provider_route_prefix(name)
}

pub fn build_grouped_models(providers: &[Provider]) -> Vec<GroupedModel> {
    let mut model_count: HashMap<String, usize> = HashMap::new();
    for p in providers {
        if !p.enabled {
            continue;
        }
        for m in &p.models {
            let trimmed = m.trim();
            if !trimmed.is_empty() {
                *model_count.entry(trimmed.to_string()).or_insert(0) += 1;
            }
        }
    }

    let mut groups = Vec::new();
    for p in providers {
        if !p.enabled || p.models.is_empty() {
            continue;
        }
        let mut seen = HashSet::new();
        let prefix = provider_slug_prefix(&p.name);
        let mut items = Vec::new();

        for m in &p.models {
            let trimmed = m.trim();
            if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
                continue;
            }
            let is_dup = model_count.get(trimmed).copied().unwrap_or(0) > 1;
            let value = if is_dup {
                format!("{}/{}", prefix, trimmed)
            } else {
                trimmed.to_string()
            };
            items.push(GroupedModelOption {
                value,
                label: trimmed.to_string(),
            });
        }

        if !items.is_empty() {
            groups.push(GroupedModel {
                id: p.id.clone(),
                name: p.name.clone(),
                protocol: p.protocol.clone(),
                preset_id: p.preset_id.clone(),
                models: items,
            });
        }
    }

    groups
}

pub fn resolve_default_model_for_client(client_id: &str, providers: &[Provider]) -> Option<String> {
    let groups = build_grouped_models(providers);
    if groups.is_empty() {
        return None;
    }

    let find_first_value = |grp: &GroupedModel| -> Option<String> {
        grp.models.first().map(|m| m.value.clone())
    };
    let find_matching_value = |grp: &GroupedModel, pred: fn(&str) -> bool| -> Option<String> {
        grp.models
            .iter()
            .find(|m| pred(&m.label))
            .map(|m| m.value.clone())
    };

    match client_id {
        "claude_code" => {
            for grp in &groups {
                if grp.protocol == "anthropic" || grp.name.to_lowercase().contains("anthropic") {
                    if let Some(v) = find_first_value(grp) {
                        return Some(v);
                    }
                }
                if let Some(v) = find_matching_value(grp, |name| name.to_lowercase().contains("claude")) {
                    return Some(v);
                }
            }
        }
        "codex_cli" => {
            for grp in &groups {
                let lower_name = grp.name.to_lowercase();
                if grp.protocol == "openai"
                    || grp.protocol == "chatgpt"
                    || lower_name.contains("openai")
                    || lower_name.contains("chatgpt")
                {
                    if let Some(v) = find_first_value(grp) {
                        return Some(v);
                    }
                }
                if let Some(v) = find_matching_value(grp, |name| {
                    let lower = name.to_lowercase();
                    lower.contains("gpt") || lower.contains("codex")
                }) {
                    return Some(v);
                }
            }
        }
        "gemini_cli" => {
            for grp in &groups {
                if grp.protocol == "gemini" || grp.name.to_lowercase().contains("gemini") {
                    if let Some(v) = find_first_value(grp) {
                        return Some(v);
                    }
                }
                if let Some(v) = find_matching_value(grp, |name| name.to_lowercase().contains("gemini")) {
                    return Some(v);
                }
            }
        }
        _ => {}
    }

    groups.first().and_then(find_first_value)
}

pub fn resolve_display_label(
    value: &str,
    groups: &[GroupedModel],
    placeholder: &str,
) -> (String, bool) {
    if value.trim().is_empty() {
        return (placeholder.to_string(), true);
    }
    for g in groups {
        for opt in &g.models {
            if opt.value == value || opt.label == value {
                return (opt.label.clone(), true);
            }
        }
    }
    (value.to_string(), false)
}

#[allow(clippy::too_many_arguments)]
pub fn render_client_model_trigger(
    client_id: &str,
    current_value: &str,
    disabled: bool,
    placeholder: &str,
    groups: &[GroupedModel],
    is_open: bool,
    bounds_store: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> AnyElement {
    let (display_text, in_pool) = resolve_display_label(current_value, groups, placeholder);
    let has_value = !current_value.trim().is_empty();
    let cid = client_id.to_string();
    let cid_for_bounds = client_id.to_string();
    let cur = current_value.to_string();
    let store_for_click = bounds_store.clone();

    let trigger_pill = div()
        .id(SharedString::from(format!("client_model_trigger_{}", client_id)))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(6.0))
        .h(px(28.0))
        .max_w(px(188.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(if has_value && !in_pool {
            palette.amber_soft_border
        } else {
            palette.input
        })
        .bg(if has_value && !in_pool {
            palette.amber_soft_bg
        } else if is_open {
            palette.accent
        } else {
            palette.background
        })
        .when(!disabled, |el| {
            el.cursor_pointer()
                .hover(move |s| {
                    if has_value && !in_pool {
                        s
                    } else {
                        s.bg(palette.accent)
                    }
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        if this
                            .client_model_select
                            .as_ref()
                            .map(|s| s.client_id == cid)
                            .unwrap_or(false)
                        {
                            this.client_model_select = None;
                            cx.notify();
                            return;
                        }
                        let fallback = Bounds::new(
                            point(e.position.x - px(90.0), e.position.y - px(14.0)),
                            size(px(180.0), px(28.0)),
                        );
                        let measured = store_for_click
                            .borrow()
                            .get(&cid)
                            .copied()
                            .unwrap_or(fallback);
                        this.open_client_model_select(
                            cid.clone(),
                            cur.clone(),
                            measured,
                            window,
                            cx,
                        );
                    }),
                )
        })
        .when(disabled, |el| el.opacity(0.5).cursor_default())
        .child(
            div()
                .text_size(px(11.5))
                .font_weight(FontWeight::NORMAL)
                .text_color(if !has_value {
                    palette.muted_foreground
                } else if !in_pool {
                    palette.amber_text
                } else {
                    palette.foreground
                })
                .truncate()
                .child(display_text),
        )
        .child(lucide_icon(
            if is_open {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            },
            11.5,
            palette.muted_foreground,
        ));

    div()
        .flex_shrink_0()
        .on_children_prepainted(move |children_bounds, _, _| {
            if let Some(&b) = children_bounds.first() {
                bounds_store
                    .borrow_mut()
                    .insert(cid_for_bounds.clone(), b);
            }
        })
        .child(trigger_pill)
        .into_any_element()
}

const HEADER_SAFE_TOP: f32 = 54.0;
const VIEWPORT_MARGIN: f32 = 10.0;
const TRIGGER_OFFSET: f32 = 4.0;
const POPOVER_CHROME_H: f32 = 43.0;
const IDEAL_LIST_MAX_H: f32 = 200.0;
const MIN_LIST_MAX_H: f32 = 56.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopoverGeometry {
    pub width: f32,
    pub left: f32,
    /// Some(top) 表示向下弹出锚定 top；None 表示向上弹出锚定 bottom
    pub top: Option<f32>,
    pub bottom: Option<f32>,
    pub list_max_h: f32,
}

pub fn is_model_option_selected(cur_val: &str, opt: &GroupedModelOption) -> bool {
    let has_dup = opt.value != opt.label;
    cur_val == opt.value || (!has_dup && cur_val == opt.label && !cur_val.contains('/'))
}

pub fn find_selected_flat_index(cur_val: &str, groups: &[GroupedModel]) -> Option<usize> {
    let mut flat_idx = 0usize;
    for grp in groups {
        // 分组标题占 1 个子元素槽位
        flat_idx += 1;
        for opt in &grp.models {
            if is_model_option_selected(cur_val, opt) {
                return Some(flat_idx);
            }
            flat_idx += 1;
        }
    }
    None
}

fn estimate_list_height(groups: &[GroupedModel]) -> f32 {
    if groups.is_empty() {
        return 52.0;
    }
    let mut h = 8.0; // 滚动列表上下 padding (4px * 2)
    for (g_idx, grp) in groups.iter().enumerate() {
        if g_idx > 0 {
            h += 9.0; // mt(4) + 1px border + extra pt(4)
        }
        h += 22.0; // 分组标题高度
        h += grp.models.len() as f32 * 28.0; // 每项 27px + mt(1)
    }
    h
}

pub fn compute_popover_geometry(
    win_w: f32,
    win_h: f32,
    trigger: Bounds<Pixels>,
    unfiltered_groups: &[GroupedModel],
) -> PopoverGeometry {
    let win_w = win_w.max(320.0);
    let win_h = win_h.max(320.0);
    let width = 244.0f32.min(win_w - 20.0);

    let trig_top = f32::from(trigger.top());
    let trig_bottom = f32::from(trigger.bottom());
    let trig_right = f32::from(trigger.right());

    let left = (trig_right - width).clamp(
        VIEWPORT_MARGIN,
        (win_w - width - VIEWPORT_MARGIN).max(VIEWPORT_MARGIN),
    );

    let bottom_bound = (win_h - VIEWPORT_MARGIN).max(HEADER_SAFE_TOP + 80.0);
    let space_below = (bottom_bound - trig_bottom - TRIGGER_OFFSET).max(0.0);
    let space_above = (trig_top - TRIGGER_OFFSET - HEADER_SAFE_TOP).max(0.0);

    let desired_list_h = estimate_list_height(unfiltered_groups).min(IDEAL_LIST_MAX_H);
    let desired_panel_h = POPOVER_CHROME_H + desired_list_h;

    let place_above = space_below < desired_panel_h && space_above > space_below;
    let avail_panel_h = if place_above {
        space_above
    } else {
        space_below
    };
    let list_max_h = (avail_panel_h - POPOVER_CHROME_H)
        .min(IDEAL_LIST_MAX_H)
        .max(MIN_LIST_MAX_H);

    if place_above {
        let anchor_bottom = (win_h - (trig_top - TRIGGER_OFFSET)).max(VIEWPORT_MARGIN);
        PopoverGeometry {
            width,
            left,
            top: None,
            bottom: Some(anchor_bottom),
            list_max_h,
        }
    } else {
        let anchor_top = (trig_bottom + TRIGGER_OFFSET).max(HEADER_SAFE_TOP);
        PopoverGeometry {
            width,
            left,
            top: Some(anchor_top),
            bottom: None,
            list_max_h,
        }
    }
}

pub fn render_client_model_popover(
    app: &PivotApp,
    state: &ClientModelSelectState,
    palette: ThemePalette,
    window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> AnyElement {
    let locale = app.prefs.locale;
    let groups = build_grouped_models(&app.providers);
    let raw_query = state.search_input.read(cx).value().trim().to_string();
    let q = raw_query.to_lowercase();

    let mut filtered_groups: Vec<GroupedModel> = Vec::new();

    for g in &groups {
        let mut matched = Vec::new();
        for opt in &g.models {
            if q.is_empty()
                || opt.label.to_lowercase().contains(&q)
                || opt.value.to_lowercase().contains(&q)
                || g.name.to_lowercase().contains(&q)
                || app.model_display_name(&opt.label).to_lowercase().contains(&q)
            {
                matched.push(opt.clone());
            }
        }
        if !matched.is_empty() {
            filtered_groups.push(GroupedModel {
                id: g.id.clone(),
                name: g.name.clone(),
                protocol: g.protocol.clone(),
                preset_id: g.preset_id.clone(),
                models: matched,
            });
        }
    }

    let viewport = window.viewport_size();
    let live_trigger_bounds = app
        .trigger_bounds
        .borrow()
        .get(&state.client_id)
        .copied()
        .unwrap_or(state.trigger_bounds);

    let geom = compute_popover_geometry(
        f32::from(viewport.width),
        f32::from(viewport.height),
        live_trigger_bounds,
        &groups,
    );

    let client_id = state.client_id.clone();
    let cur_val = state.current_value.clone();

    let popover_panel = div()
        .id("client_model_select_popover")
        .absolute()
        .left(px(geom.left))
        .when_some(geom.top, |el, t| el.top(px(t)))
        .when_some(geom.bottom, |el, b| el.bottom(px(b)))
        .w(px(geom.width))
        .rounded(px(8.0))
        .bg(palette.popover)
        .border_1()
        .border_color(palette.border)
        .shadow(palette.popover_shadows())
        .overflow_hidden()
        .flex()
        .flex_col()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, app_cx| {
            app_cx.stop_propagation();
        })
        .on_mouse_down(MouseButton::Right, |_, _, app_cx| {
            app_cx.stop_propagation();
        })
        .on_scroll(|_, _, app_cx| {
            app_cx.stop_propagation();
        })
        // 1. 顶部轻量单层搜索框（无粗糙外扩光环）
        .child(
            div()
                .flex_shrink_0()
                .p(px(6.0))
                .border_b_1()
                .border_color(palette.border_subtle)
                .child(NativeTextInput::search(&state.search_input, palette).render(window, cx)),
        )
        // 2. 分组模型无轨道滚动列表（扁平直接子元素以匹配 ScrollHandle::scroll_to_item 索引）
        .child({
            let mut list = div()
                .id("client_model_popover_scroll")
                .min_h_0()
                .max_h(px(geom.list_max_h))
                .overflow_y_scroll()
                .track_scroll(&state.scroll_handle)
                .p(px(4.0))
                .flex()
                .flex_col();

            if filtered_groups.is_empty() {
                list = list.child(
                    div()
                        .flex_shrink_0()
                        .py(px(18.0))
                        .px(px(8.0))
                        .text_size(px(11.5))
                        .text_color(palette.muted_foreground)
                        .text_center()
                        .child(locale.pick("无匹配的可用模型", "No matching models")),
                );
            } else {
                for (g_idx, grp) in filtered_groups.iter().enumerate() {
                    list = list.child(
                        div()
                            .flex_shrink_0()
                            .when(g_idx > 0, |el| {
                                el.mt(px(4.0))
                                    .pt(px(7.0))
                                    .border_t_1()
                                    .border_color(palette.border_subtle)
                            })
                            .when(g_idx == 0, |el| el.pt(px(3.0)))
                            .px(px(8.0))
                            .pb(px(2.5))
                            .text_size(px(10.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.muted_foreground)
                            .truncate()
                            .child(grp.name.clone()),
                    );

                    for (m_idx, opt) in grp.models.iter().enumerate() {
                        let is_selected = is_model_option_selected(&cur_val, opt);
                        let cid = client_id.clone();
                        let next_val = opt.value.clone();

                        list = list.child(
                            div()
                                .id(SharedString::from(format!(
                                    "cmodel_opt_{}_{}",
                                    g_idx, m_idx
                                )))
                                .flex_shrink_0()
                                .mt(px(1.0))
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .h(px(27.0))
                                .px(px(7.0))
                                .rounded(px(5.0))
                                .cursor_pointer()
                                .bg(if is_selected {
                                    palette.accent
                                } else {
                                    rgba(0x00000000)
                                })
                                .hover(move |s| s.bg(palette.accent))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.select_client_model(
                                            cid.clone(),
                                            next_val.clone(),
                                            cx,
                                        );
                                    }),
                                )
                                .child(
                                    div()
                                        .w(px(13.0))
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .when(is_selected, |el| {
                                            el.child(lucide_icon(
                                                IconName::Check,
                                                11.5,
                                                palette.foreground,
                                            ))
                                        }),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_size(px(11.5))
                                        .font_weight(if is_selected {
                                            FontWeight::MEDIUM
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(palette.foreground)
                                        .truncate()
                                        .child(opt.label.clone()),
                                ),
                        );
                    }
                }
            }

            list
        });

    div()
        .id("client_model_select_backdrop")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .occlude()
        .on_scroll(|_, _, app_cx| {
            app_cx.stop_propagation();
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.client_model_select = None;
                this.root_focus.focus(window, cx);
                cx.notify();
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.client_model_select = None;
                this.root_focus.focus(window, cx);
                cx.notify();
            }),
        )
        .child(popover_panel)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_groups(counts: &[usize]) -> Vec<GroupedModel> {
        counts
            .iter()
            .enumerate()
            .map(|(gi, &count)| GroupedModel {
                id: format!("p{}", gi),
                name: format!("Provider {}", gi),
                protocol: "openai".to_string(),
                preset_id: None,
                models: (0..count)
                    .map(|mi| GroupedModelOption {
                        value: format!("m-{}-{}", gi, mi),
                        label: format!("m-{}-{}", gi, mi),
                    })
                    .collect(),
            })
            .collect()
    }

    #[::core::prelude::v1::test]
    fn mid_trigger_places_below_and_fits_viewport() {
        let trigger = Bounds::new(point(px(218.0), px(160.5)), size(px(120.0), px(28.0)));
        let groups = make_groups(&[4, 4]);
        let geom = compute_popover_geometry(416.0, 460.0, trigger, &groups);

        assert_eq!(geom.top, Some(192.5));
        assert_eq!(geom.bottom, None);
        let panel_bottom = geom.top.unwrap() + POPOVER_CHROME_H + geom.list_max_h;
        assert!(panel_bottom <= 450.0, "panel bottom {} exceeds 450", panel_bottom);
    }

    #[::core::prelude::v1::test]
    fn bottom_trigger_with_many_models_flips_above_without_covering_header_or_trigger() {
        let trigger = Bounds::new(point(px(218.0), px(249.5)), size(px(120.0), px(28.0)));
        let groups = make_groups(&[4, 4]);
        let win_h = 460.0;
        let geom = compute_popover_geometry(416.0, win_h, trigger, &groups);

        assert_eq!(geom.top, None);
        let anchor_bottom = geom.bottom.expect("should anchor bottom");
        let panel_bottom = win_h - anchor_bottom;
        assert!((panel_bottom - 245.5).abs() < 1e-3);
        let panel_top = panel_bottom - (POPOVER_CHROME_H + geom.list_max_h);
        assert!(
            panel_top >= HEADER_SAFE_TOP,
            "panel top {} overlaps header",
            panel_top
        );
    }

    #[::core::prelude::v1::test]
    fn bottom_trigger_with_two_models_stays_below() {
        let trigger = Bounds::new(point(px(218.0), px(249.5)), size(px(120.0), px(28.0)));
        let groups = make_groups(&[2]);
        let geom = compute_popover_geometry(416.0, 460.0, trigger, &groups);

        assert_eq!(geom.top, Some(281.5));
        assert_eq!(geom.bottom, None);
    }

    #[::core::prelude::v1::test]
    fn find_selected_flat_index_handles_groups_and_prefixed_duplicates() {
        let groups = vec![
            GroupedModel {
                id: "p1".to_string(),
                name: "Antigravity".to_string(),
                protocol: "antigravity".to_string(),
                preset_id: None,
                models: vec![
                    GroupedModelOption {
                        value: "antigravity/gpt-4o".to_string(),
                        label: "gpt-4o".to_string(),
                    },
                    GroupedModelOption {
                        value: "claude-opus".to_string(),
                        label: "claude-opus".to_string(),
                    },
                ],
            },
            GroupedModel {
                id: "p2".to_string(),
                name: "ChatGPT".to_string(),
                protocol: "chatgpt".to_string(),
                preset_id: None,
                models: vec![GroupedModelOption {
                    value: "chatgpt/gpt-4o".to_string(),
                    label: "gpt-4o".to_string(),
                }],
            },
        ];

        assert_eq!(find_selected_flat_index("antigravity/gpt-4o", &groups), Some(1));
        assert_eq!(find_selected_flat_index("claude-opus", &groups), Some(2));
        assert_eq!(find_selected_flat_index("chatgpt/gpt-4o", &groups), Some(4));
        assert_eq!(find_selected_flat_index("gpt-4o", &groups), None);
    }
}
