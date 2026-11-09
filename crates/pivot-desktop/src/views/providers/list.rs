use crate::PivotApp;
use gpui_kit::component::switch::Switch;
use gpui_kit::*;

pub fn render_provider_list(
    app: &PivotApp,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let providers = app.providers.clone();
    let latencies = app.latencies.clone();
    let testing = app.testing_latencies.clone();

    if providers.is_empty() {
        return div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .h(px(400.0))
            .w_full()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xd4d4d8))
                    .child("暂无配置的模型服务商"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(0x71717a))
                    .child("添加 OpenAI、Anthropic、DeepSeek 或兼容服务商以开始使用"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .bg(rgb(0x2563eb))
                    .text_color(rgb(0xffffff))
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x1d4ed8)))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                        this.open_editor(None, window, cx);
                    }))
                    .child("+ 创建第一个服务商"),
            );
    }

    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(12.0))
        .children(providers.into_iter().map(|p| {
            let p_id = p.id.clone();
            let is_enabled = p.enabled;
            let latency = latencies.get(&p_id).copied();
            let is_testing = testing.get(&p_id).copied().unwrap_or(false);
            let provider_clone = p.clone();

            div()
                .flex()
                .flex_col()
                .w_full()
                .p(px(16.0))
                .rounded(px(10.0))
                .bg(rgb(0x202023))
                .border_1()
                .border_color(rgba(0xffffff10))
                .gap(px(12.0))
                .child(
                    // 卡片头部：名称、协议、开关、测试延迟
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .px(px(6.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(rgba(0x3b82f61a))
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(0x60a5fa))
                                        .child(p.protocol.to_uppercase()),
                                )
                                .child(
                                    div()
                                        .text_size(px(15.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(0xffffff))
                                        .child(p.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(rgb(0x71717a))
                                        .child(p.base_url.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.0))
                                .child(
                                    // 延迟指示器与测速按钮
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.0))
                                        .px(px(8.0))
                                        .py(px(3.0))
                                        .rounded(px(4.0))
                                        .bg(rgba(0xffffff0a))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgba(0xffffff15)))
                                        .on_mouse_down(MouseButton::Left, {
                                            let id = p_id.clone();
                                            cx.listener(move |this, _, _, cx| {
                                                this.test_provider_latency(id.clone(), cx);
                                            })
                                        })
                                        .child(if is_testing {
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgb(0xeab308))
                                                .child("测速中...")
                                        } else if let Some(ms) = latency {
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(if ms < 200 {
                                                    rgb(0x4ade80)
                                                } else if ms < 600 {
                                                    rgb(0xfacc15)
                                                } else {
                                                    rgb(0xf87171)
                                                })
                                                .child(format!("{}ms", ms))
                                        } else {
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgb(0xa1a1aa))
                                                .child("测速")
                                        }),
                                )
                                .child(
                                    // 启用/停用开关
                                    Switch::new(format!("switch-{}", p_id))
                                        .checked(is_enabled)
                                        .on_click({
                                            let id = p_id.clone();
                                            cx.listener(move |this, checked: &bool, _, cx| {
                                                this.toggle_provider(id.clone(), *checked, cx);
                                            })
                                        }),
                                )
                                .child(
                                    // 编辑按钮
                                    div()
                                        .px(px(8.0))
                                        .py(px(3.0))
                                        .rounded(px(4.0))
                                        .cursor_pointer()
                                        .bg(rgba(0xffffff0a))
                                        .hover(|s| s.bg(rgba(0xffffff1a)))
                                        .text_size(px(11.0))
                                        .text_color(rgb(0xd4d4d8))
                                        .on_mouse_down(MouseButton::Left, {
                                            let provider = provider_clone.clone();
                                            cx.listener(move |this, _, window, cx| {
                                                this.open_editor(Some(provider.clone()), window, cx);
                                            })
                                        })
                                        .child("编辑"),
                                )
                                .child(
                                    // 删除按钮
                                    div()
                                        .px(px(8.0))
                                        .py(px(3.0))
                                        .rounded(px(4.0))
                                        .cursor_pointer()
                                        .bg(rgba(0xef444415))
                                        .hover(|s| s.bg(rgba(0xef44442a)))
                                        .text_size(px(11.0))
                                        .text_color(rgb(0xf87171))
                                        .on_mouse_down(MouseButton::Left, {
                                            let id = p_id.clone();
                                            cx.listener(move |this, _, _, cx| {
                                                this.delete_provider(id.clone(), cx);
                                            })
                                        })
                                        .child("删除"),
                                ),
                        ),
                )
                .child(
                    // 卡片下部：模型池标签列表
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.0))
                        .items_center()
                        .children(p.models.iter().map(|m| {
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(rgba(0xffffff08))
                                .border_1()
                                .border_color(rgba(0xffffff0a))
                                .text_size(px(11.0))
                                .text_color(rgb(0xa1a1aa))
                                .child(m.clone())
                        })),
                )
        }))
}
