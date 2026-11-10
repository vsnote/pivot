use gpui_kit::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NativeButtonVariant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

pub struct NativeButton {
    label: String,
    variant: NativeButtonVariant,
    disabled: bool,
}

impl NativeButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: NativeButtonVariant::Secondary,
            disabled: false,
        }
    }

    pub fn primary(mut self) -> Self {
        self.variant = NativeButtonVariant::Primary;
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

    #[allow(dead_code)]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        self,
        on_click: impl Fn(&gpui_kit::MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        let is_disabled = self.disabled;
        let variant = self.variant;
        let label = self.label;

        let (bg, hover_bg, border, text_color) = match variant {
            NativeButtonVariant::Primary => (
                rgb(0x2563eb),
                rgb(0x1d4ed8),
                rgb(0x3b82f6),
                rgb(0xffffff),
            ),
            NativeButtonVariant::Secondary => (
                rgb(0x27272a),
                rgb(0x323238),
                rgb(0x3f3f46),
                rgb(0xededee),
            ),
            NativeButtonVariant::Ghost => (
                rgba(0x00000000),
                rgb(0x27272a),
                rgba(0x00000000),
                rgb(0xa1a1aa),
            ),
            NativeButtonVariant::Danger => (
                rgba(0xef444415),
                rgba(0xef444425),
                rgba(0xef444440),
                rgb(0xf87171),
            ),
        };

        div()
            .flex()
            .items_center()
            .justify_center()
            .h(px(30.0))
            .px(px(14.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .border_1()
            .border_color(border)
            .bg(if is_disabled { rgb(0x222226) } else { bg })
            .text_size(px(12.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if is_disabled { rgb(0x71717a) } else { text_color })
            .hover(move |style| {
                if !is_disabled {
                    style.bg(hover_bg)
                } else {
                    style
                }
            })
            .on_mouse_down(MouseButton::Left, move |e, window, app| {
                if !is_disabled {
                    on_click(e, window, app);
                }
            })
            .child(label)
    }
}
