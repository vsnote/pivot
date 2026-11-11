use crate::components::button::NativeButton;
use crate::components::icons::lucide_icon;
use crate::theme::ThemePalette;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConfirmVariant {
    Danger,
    Primary,
}

#[derive(Clone)]
pub struct ConfirmDialogSpec {
    pub title: String,
    pub details: Vec<String>,
    pub confirm_label: String,
    pub cancel_label: String,
    pub variant: ConfirmVariant,
}

pub fn render_confirm_dialog(
    spec: &ConfirmDialogSpec,
    busy: bool,
    palette: ThemePalette,
    on_cancel: impl Fn(&MouseDownEvent, &mut Window, &mut App) + Clone + 'static,
    on_confirm: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let on_backdrop_cancel = on_cancel.clone();

    div()
        .id("confirm_dialog_backdrop")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .px(px(16.0))
        .bg(palette.overlay_backdrop)
        .on_scroll(|_, _, app| {
            app.stop_propagation();
        })
        .on_mouse_down(MouseButton::Left, move |e, window, app| {
            app.stop_propagation();
            if !busy {
                on_backdrop_cancel(e, window, app);
            }
        })
        .child(
            div()
                .w_full()
                .max_w(px(372.0))
                .rounded(px(8.0))
                .bg(palette.popover)
                .border_1()
                .border_color(palette.border)
                .shadow(palette.modal_shadows())
                .overflow_hidden()
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, app| {
                    app.stop_propagation();
                })
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(10.0))
                        .px(px(18.0))
                        .py(px(13.0))
                        .border_b_1()
                        .border_color(palette.border)
                        .child(
                            div()
                                .mt(px(1.5))
                                .child(lucide_icon(IconName::TriangleAlert, 15.0, palette.amber_text)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(palette.foreground)
                                .child(spec.title.clone()),
                        ),
                )
                .when(!spec.details.is_empty(), |el| {
                    let mut body = div().flex().flex_col().gap(px(6.0)).px(px(18.0)).py(px(13.0));
                    for line in &spec.details {
                        body = body.child(
                            div()
                                .text_size(px(11.5))
                                .line_height(rems(1.45))
                                .text_color(palette.muted_foreground)
                                .child(line.clone()),
                        );
                    }
                    el.child(body)
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(8.0))
                        .px(px(20.0))
                        .py(px(12.0))
                        .border_t_1()
                        .border_color(palette.border)
                        .child(
                            NativeButton::new(spec.cancel_label.clone(), palette)
                                .secondary()
                                .disabled(busy)
                                .on_click(on_cancel),
                        )
                        .child({
                            let btn = NativeButton::new(spec.confirm_label.clone(), palette)
                                .disabled(busy);
                            let btn = match spec.variant {
                                ConfirmVariant::Danger => btn.danger(),
                                ConfirmVariant::Primary => btn.primary(),
                            };
                            btn.on_click(on_confirm)
                        }),
                ),
        )
        .into_any_element()
}
