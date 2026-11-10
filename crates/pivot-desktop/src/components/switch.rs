use gpui_kit::*;

pub struct NativeSwitch {
    checked: bool,
    disabled: bool,
}

impl NativeSwitch {
    pub fn new(checked: bool) -> Self {
        Self {
            checked,
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_toggle(
        self,
        on_toggle: impl Fn(&gpui_kit::MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        let is_checked = self.checked;
        let is_disabled = self.disabled;

        // 原生 iOS / macOS 紧凑标准开关 (36px × 20px, 纯白 14px 圆形滑块)
        div()
            .flex()
            .items_center()
            .w(px(36.0))
            .h(px(20.0))
            .rounded_full()
            .p(px(2.0))
            .cursor_pointer()
            .border_1()
            .border_color(if is_checked {
                if is_disabled {
                    rgba(0x10b98140)
                } else {
                    rgb(0x10b981)
                }
            } else {
                rgb(0x3f3f46)
            })
            .bg(if is_checked {
                if is_disabled {
                    rgba(0x10b98160)
                } else {
                    rgb(0x10b981)
                }
            } else {
                if is_disabled {
                    rgb(0x222226)
                } else {
                    rgb(0x27272a)
                }
            })
            .hover(|style| {
                if !is_disabled {
                    if is_checked {
                        style.bg(rgb(0x059669))
                    } else {
                        style.bg(rgb(0x323238))
                    }
                } else {
                    style
                }
            })
            .on_mouse_down(MouseButton::Left, move |e, window, app| {
                if !is_disabled {
                    on_toggle(e, window, app);
                }
            })
            .child(
                // 纯白圆滑块，利用 margin-left 瞬时或极简平移，响应零延迟，绝不卡死
                div()
                    .size(px(14.0))
                    .rounded_full()
                    .bg(if is_disabled {
                        rgb(0xa1a1aa)
                    } else {
                        rgb(0xffffff)
                    })
                    .ml(if is_checked { px(16.0) } else { px(0.0) }),
            )
    }
}
