use crate::components::icons::lucide_icon;
use crate::theme::ThemePalette;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NativeButtonVariant {
    Primary,
    Outline,
    Secondary,
    Ghost,
    Danger,
    DangerGhost,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NativeButtonSize {
    Default,
    Sm,
    Xs,
}

pub struct NativeButton {
    label: String,
    icon: Option<IconName>,
    icon_color: Option<Rgba>,
    variant: NativeButtonVariant,
    size: NativeButtonSize,
    height_override: Option<f32>,
    disabled: bool,
    palette: ThemePalette,
}

impl NativeButton {
    pub fn new(label: impl Into<String>, palette: ThemePalette) -> Self {
        Self {
            label: label.into(),
            icon: None,
            icon_color: None,
            variant: NativeButtonVariant::Secondary,
            size: NativeButtonSize::Default,
            height_override: None,
            disabled: false,
            palette,
        }
    }

    pub fn primary(mut self) -> Self {
        self.variant = NativeButtonVariant::Primary;
        self
    }

    pub fn outline(mut self) -> Self {
        self.variant = NativeButtonVariant::Outline;
        self
    }

    pub fn secondary(mut self) -> Self {
        self.variant = NativeButtonVariant::Secondary;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.variant = NativeButtonVariant::Ghost;
        self
    }

    pub fn danger(mut self) -> Self {
        self.variant = NativeButtonVariant::Danger;
        self
    }

    pub fn danger_ghost(mut self) -> Self {
        self.variant = NativeButtonVariant::DangerGhost;
        self
    }

    pub fn sm(mut self) -> Self {
        self.size = NativeButtonSize::Sm;
        self
    }

    pub fn xs(mut self) -> Self {
        self.size = NativeButtonSize::Xs;
        self
    }

    pub fn h_px(mut self, h: f32) -> Self {
        self.height_override = Some(h);
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn icon_colored(mut self, icon: IconName, color: Rgba) -> Self {
        self.icon = Some(icon);
        self.icon_color = Some(color);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        self,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        let is_disabled = self.disabled;
        let p = self.palette;
        let label = self.label;
        let icon = self.icon;
        let custom_icon_color = self.icon_color;
        let h_override = self.height_override;

        let (base_height, pad_x, font_sz, radius, icon_sz) = match self.size {
            NativeButtonSize::Default => (32.0, 12.0, 12.0, 6.0, 13.5),
            NativeButtonSize::Sm => (28.0, 10.0, 12.0, 6.0, 13.0),
            NativeButtonSize::Xs => (24.0, 8.0, 11.0, 4.5, 12.0),
        };
        let height = h_override.unwrap_or(base_height);

        let (bg, hover_bg, border, text_color, hover_text) = match self.variant {
            NativeButtonVariant::Primary => (
                p.primary,
                p.primary_hover,
                rgba(0x00000000),
                p.primary_foreground,
                p.primary_foreground,
            ),
            NativeButtonVariant::Outline => (
                p.background,
                p.accent,
                p.input,
                p.foreground,
                p.accent_foreground,
            ),
            NativeButtonVariant::Secondary => (
                p.secondary,
                p.secondary_hover,
                rgba(0x00000000),
                p.secondary_foreground,
                p.secondary_foreground,
            ),
            NativeButtonVariant::Ghost => (
                rgba(0x00000000),
                p.accent,
                rgba(0x00000000),
                p.muted_foreground,
                p.accent_foreground,
            ),
            NativeButtonVariant::Danger => (
                p.destructive,
                p.destructive_hover,
                rgba(0x00000000),
                p.destructive_foreground,
                p.destructive_foreground,
            ),
            NativeButtonVariant::DangerGhost => (
                rgba(0x00000000),
                p.destructive_soft_bg,
                rgba(0x00000000),
                p.muted_foreground,
                p.destructive_text,
            ),
        };

        let effective_icon_color = custom_icon_color.unwrap_or(text_color);

        div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(5.0))
            .flex_shrink_0()
            .h(px(height))
            .px(px(pad_x))
            .rounded(px(radius))
            .border_1()
            .border_color(border)
            .bg(bg)
            .text_size(px(font_sz))
            .font_weight(FontWeight::MEDIUM)
            .text_color(text_color)
            .when(is_disabled, |el| el.opacity(0.4))
            .cursor(if is_disabled {
                CursorStyle::Arrow
            } else {
                CursorStyle::PointingHand
            })
            .hover(move |style| {
                if !is_disabled {
                    style.bg(hover_bg).text_color(hover_text)
                } else {
                    style
                }
            })
            .on_mouse_down(MouseButton::Left, move |e, window, app| {
                app.stop_propagation();
                if !is_disabled {
                    on_click(e, window, app);
                }
            })
            .when_some(icon, |el, ic| {
                el.child(lucide_icon(ic, icon_sz, effective_icon_color))
            })
            .when(!label.is_empty(), |el| el.child(label))
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SegmentedTabsSize {
    Default,
    Sm,
    Xs,
}

/// 1:1 复刻 `Tabs.tsx` (`variant="default"`) 的内嵌分段切换器
pub fn render_segmented_tabs<T: Clone + PartialEq + 'static>(
    items: Vec<(T, String)>,
    current: T,
    size: SegmentedTabsSize,
    palette: ThemePalette,
    on_select: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
) -> Div {
    let (list_h, item_h, pad_x, font_sz, radius) = match size {
        SegmentedTabsSize::Default => (30.0, 26.0, 12.0, 12.0, 5.0),
        SegmentedTabsSize::Sm => (26.0, 22.0, 8.0, 11.0, 4.0),
        SegmentedTabsSize::Xs => (22.0, 18.0, 6.0, 10.0, 4.0),
    };

    let mut row = div()
        .flex()
        .items_center()
        .flex_shrink_0()
        .h(px(list_h))
        .p(px(2.0))
        .rounded(px(6.0))
        .bg(palette.segmented_bg)
        .border_1()
        .border_color(palette.border_subtle);

    for (val, label) in items {
        let is_active = val == current;
        let on_select_cb = on_select.clone();
        let val_clone = val.clone();

        row = row.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(item_h))
                .px(px(pad_x))
                .rounded(px(radius))
                .cursor_pointer()
                .text_size(px(font_sz))
                .font_weight(FontWeight::MEDIUM)
                .border_1()
                .border_color(if is_active {
                    palette.segmented_active_border
                } else {
                    rgba(0x00000000)
                })
                .bg(if is_active {
                    palette.segmented_active_bg
                } else {
                    rgba(0x00000000)
                })
                .text_color(if is_active {
                    palette.foreground
                } else {
                    palette.muted_foreground
                })
                .hover(move |style| {
                    if !is_active {
                        style.text_color(palette.foreground)
                    } else {
                        style
                    }
                })
                .on_mouse_down(MouseButton::Left, move |_, window, app| {
                    app.stop_propagation();
                    on_select_cb(val_clone.clone(), window, app);
                })
                .child(label),
        );
    }

    row
}
