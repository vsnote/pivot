use crate::theme::ThemePalette;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SwitchSize {
    Sm,
    Xs,
}

type SwitchCallback = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct NativeSwitch {
    id: ElementId,
    checked: bool,
    disabled: bool,
    size: SwitchSize,
    palette: ThemePalette,
    on_toggle: Option<SwitchCallback>,
}

impl NativeSwitch {
    pub fn new(id: impl Into<ElementId>, checked: bool, palette: ThemePalette) -> Self {
        Self {
            id: id.into(),
            checked,
            disabled: false,
            size: SwitchSize::Sm,
            palette,
            on_toggle: None,
        }
    }

    pub fn xs(mut self) -> Self {
        self.size = SwitchSize::Xs;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_toggle(
        mut self,
        on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }
}

impl RenderOnce for NativeSwitch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_checked = self.checked;
        let is_disabled = self.disabled;
        let palette = self.palette;
        let on_toggle = self.on_toggle.clone();

        let (track_w, track_h, thumb_d, max_offset) = match self.size {
            SwitchSize::Sm => (36.0, 20.0, 16.0, 16.0),
            SwitchSize::Xs => (30.0, 16.0, 12.0, 14.0),
        };

        let spring_id = ElementId::Name(SharedString::from(format!("{}_thumb_x", self.id)));
        let thumb_x = gpui_kit::base::spring(
            spring_id,
            if is_checked { px(max_offset) } else { px(0.0) },
            gpui_kit::base::Spring::new(std::time::Duration::from_millis(150)),
            window,
            cx,
        );

        let track_bg = if is_checked {
            palette.primary
        } else {
            palette.switch_unchecked_bg
        };

        let track_hover_bg = if is_checked {
            palette.primary_hover
        } else {
            palette.switch_unchecked_hover
        };

        let thumb_bg = if is_checked {
            palette.switch_thumb_checked
        } else {
            palette.switch_thumb_unchecked
        };

        div()
            .id(self.id)
            .flex()
            .items_center()
            .flex_shrink_0()
            .w(px(track_w))
            .h(px(track_h))
            .rounded_full()
            .p(px(2.0))
            .bg(track_bg)
            .when(is_disabled, |el| el.opacity(0.4))
            .cursor(if is_disabled {
                CursorStyle::Arrow
            } else {
                CursorStyle::PointingHand
            })
            .hover(move |style| {
                if !is_disabled {
                    style.bg(track_hover_bg)
                } else {
                    style
                }
            })
            .on_mouse_down(MouseButton::Left, move |e, window, app| {
                app.stop_propagation();
                if !is_disabled {
                    if let Some(handler) = &on_toggle {
                        handler(e, window, app);
                    }
                }
            })
            .child(
                div()
                    .size(px(thumb_d))
                    .rounded_full()
                    .bg(thumb_bg)
                    .ml(thumb_x),
            )
    }
}
