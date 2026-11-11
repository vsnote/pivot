use crate::components::icons::lucide_icon;
use crate::components::text_input::NativeTextInput;
use crate::theme::ThemePalette;
use crate::PivotApp;
use gpui_kit::assets::IconName;
use gpui_kit::base::{Align, Placement, Positioner};
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
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.trim().to_lowercase().chars() {
        if ch.is_whitespace() {
            if !prev_dash {
                out.push('-');
                prev_dash = true;
            }
        } else {
            out.push(ch);
            prev_dash = false;
        }
    }
    out
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
    let active: Vec<&Provider> = providers
        .iter()
        .filter(|p| p.enabled && !p.models.is_empty())
        .collect();

    if active.is_empty() {
        return None;
    }

    match client_id {
        "claude_code" => {
            for p in &active {
                if p.protocol == "anthropic" || p.name.to_lowercase().contains("anthropic") {
                    if let Some(first) = p.models.first() {
                        return Some(first.clone());
                    }
                }
                if let Some(m) = p
                    .models
                    .iter()
                    .find(|name| name.to_lowercase().contains("claude"))
                {
                    return Some(m.clone());
                }
            }
        }
        "codex_cli" => {
            for p in &active {
                let lower_name = p.name.to_lowercase();
                if p.protocol == "openai"
                    || p.protocol == "chatgpt"
                    || lower_name.contains("openai")
                    || lower_name.contains("chatgpt")
                {
                    if let Some(first) = p.models.first() {
                        return Some(first.clone());
                    }
                }
                if let Some(m) = p.models.iter().find(|name| {
                    let lower = name.to_lowercase();
                    lower.contains("gpt") || lower.contains("codex")
                }) {
                    return Some(m.clone());
                }
            }
        }
        "gemini_cli" => {
            for p in &active {
                if p.protocol == "gemini" || p.name.to_lowercase().contains("gemini") {
                    if let Some(first) = p.models.first() {
                        return Some(first.clone());
                    }
                }
                if let Some(m) = p
                    .models
                    .iter()
                    .find(|name| name.to_lowercase().contains("gemini"))
                {
                    return Some(m.clone());
                }
            }
        }
        _ => {}
    }

    active.first().and_then(|p| p.models.first().cloned())
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
    let win_w = f32::from(viewport.width).max(320.0);
    let pop_w = 244.0f32.min(win_w - 20.0);

    let live_trigger_bounds = app
        .trigger_bounds
        .borrow()
        .get(&state.client_id)
        .copied()
        .unwrap_or(state.trigger_bounds);

    let client_id = state.client_id.clone();
    let cur_val = state.current_value.clone();

    let popover_panel = div()
        .id("client_model_select_popover")
        .w(px(pop_w))
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
        .on_scroll(|_, _, app_cx| {
            app_cx.stop_propagation();
        })
        // 1. 顶部轻量单层搜索框（无粗糙外扩光环）
        .child(
            div()
                .p(px(6.0))
                .border_b_1()
                .border_color(palette.border_subtle)
                .child(NativeTextInput::search(&state.search_input, palette).render(window, cx)),
        )
        // 2. 分组模型无轨道滚动列表
        .child({
            let mut list = div()
                .id("client_model_popover_scroll")
                .max_h(px(224.0))
                .overflow_y_scroll()
                .track_scroll(&state.scroll_handle)
                .p(px(4.0))
                .flex()
                .flex_col();

            if filtered_groups.is_empty() {
                list = list.child(
                    div()
                        .py(px(18.0))
                        .px(px(8.0))
                        .text_size(px(11.5))
                        .text_color(palette.muted_foreground)
                        .text_center()
                        .child(locale.pick("无匹配的可用模型", "No matching models")),
                );
            } else {
                for (g_idx, grp) in filtered_groups.iter().enumerate() {
                    let mut grp_block = div()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .when(g_idx > 0, |el| {
                            el.mt(px(4.0))
                                .pt(px(4.0))
                                .border_t_1()
                                .border_color(palette.border_subtle)
                        })
                        .child(
                            div()
                                .px(px(8.0))
                                .pt(px(3.0))
                                .pb(px(2.5))
                                .text_size(px(10.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.muted_foreground)
                                .truncate()
                                .child(grp.name.clone()),
                        );

                    for (m_idx, opt) in grp.models.iter().enumerate() {
                        let is_selected = cur_val == opt.value
                            || (cur_val == opt.label && !cur_val.contains('/'));
                        let cid = client_id.clone();
                        let next_val = opt.value.clone();

                        grp_block = grp_block.child(
                            div()
                                .id(SharedString::from(format!(
                                    "cmodel_opt_{}_{}",
                                    g_idx, m_idx
                                )))
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

                    list = list.child(grp_block);
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
        .on_scroll(|_, _, app_cx| {
            app_cx.stop_propagation();
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.client_model_select = None;
                cx.notify();
            }),
        )
        .child(
            Positioner::side(live_trigger_bounds)
                .placement(Placement::Bottom)
                .align(Align::End)
                .offset(px(4.0))
                .margin(px(10.0))
                .occlude()
                .child(popover_panel),
        )
        .into_any_element()
}
