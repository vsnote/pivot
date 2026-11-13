use crate::components::icons::lucide_icon;
use crate::theme::ThemePalette;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::Sizable;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputVariant {
    Form,
    Search,
}

/// 轻量级原生输入框封装：
/// 关闭 `gpui-component` 默认粗糙的半透明外扩 Focus Ring，
/// 统一使用干净的 1px 细边框与平滑焦点边框变色，对齐原版 Tailwind/Shadcn 精致视觉。
pub struct NativeTextInput {
    state: Entity<InputState>,
    palette: ThemePalette,
    variant: TextInputVariant,
    height_px: f32,
    text_px: f32,
    cleanable: bool,
    disabled: bool,
    suffix_el: Option<AnyElement>,
}

impl NativeTextInput {
    pub fn new(state: &Entity<InputState>, palette: ThemePalette) -> Self {
        Self {
            state: state.clone(),
            palette,
            variant: TextInputVariant::Form,
            height_px: 30.0,
            text_px: 12.0,
            cleanable: false,
            disabled: false,
            suffix_el: None,
        }
    }

    pub fn search(state: &Entity<InputState>, palette: ThemePalette) -> Self {
        Self {
            state: state.clone(),
            palette,
            variant: TextInputVariant::Search,
            height_px: 28.0,
            text_px: 11.5,
            cleanable: true,
            disabled: false,
            suffix_el: None,
        }
    }

    pub fn compact(mut self) -> Self {
        self.height_px = 28.0;
        self.text_px = 11.5;
        self
    }

    pub fn xs(mut self) -> Self {
        self.height_px = 26.0;
        self.text_px = 11.0;
        self
    }

    pub fn cleanable(mut self, cleanable: bool) -> Self {
        self.cleanable = cleanable;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn suffix(mut self, el: impl IntoElement) -> Self {
        self.suffix_el = Some(el.into_any_element());
        self
    }

    pub fn render(self, window: &Window, cx: &App) -> Div {
        let palette = self.palette;
        let is_focused = !self.disabled
            && self
                .state
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx);

        let (bg_color, normal_border, focus_border) = match self.variant {
            TextInputVariant::Form => (
                if self.disabled {
                    palette.muted_subtle
                } else {
                    palette.background
                },
                palette.input,
                palette.ring_border,
            ),
            TextInputVariant::Search => (
                palette.search_bg,
                palette.border_subtle,
                palette.ring_border,
            ),
        };

        let mut inner = Input::new(&self.state)
            .appearance(false)
            .xsmall()
            .h_full()
            .text_size(px(self.text_px))
            .disabled(self.disabled)
            .cleanable(self.cleanable);

        if self.variant == TextInputVariant::Search {
            inner = inner.prefix(lucide_icon(
                IconName::Search,
                12.0,
                palette.muted_foreground,
            ));
        }

        if let Some(suf) = self.suffix_el {
            inner = inner.suffix(suf);
        }

        let state_for_click = self.state.clone();
        let disabled = self.disabled;

        div()
            .w_full()
            .h(px(self.height_px))
            .rounded(px(6.0))
            .bg(bg_color)
            .border_1()
            .border_color(if is_focused {
                focus_border
            } else {
                normal_border
            })
            .when(disabled, |el| el.opacity(0.65))
            .when(!disabled, |el| {
                el.cursor_text().on_mouse_down(
                    MouseButton::Left,
                    move |_, window, cx| {
                        let fh = state_for_click.read(cx).focus_handle(cx);
                        if !fh.contains_focused(window, cx) {
                            fh.focus(window, cx);
                        }
                    },
                )
            })
            .flex()
            .items_center()
            .overflow_hidden()
            .child(inner)
    }
}
