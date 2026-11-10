use crate::components::NativeSwitch;
use crate::PivotApp;
use gpui_kit::*;
use pivot_core::db::Provider;

pub fn render_provider_list(
    app: &PivotApp,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let providers = app.providers.clone();
    let latencies = app.latencies.clone();
    let testing = app.testing_latencies.clone();

    // 统一定宽居中容器 (原版 max-w-5xl)
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(960.0))
        .mx_auto()
        .gap(px(14.0))
        .child(
            // 原版统一面板 PanelClass
            div()
                .flex()
                .flex_col()
                .w_full()
                .rounded(px(8.0))
                .border_1()
                .border_color(rgb(0x2e2e32))
                .bg(rgb(0x1c1c1f))
                .overflow_hidden()
                .child(if providers.is_empty() {
                    // 空状态
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .py(px(40.0))
                        .px(px(16.0))
                        .gap(px(12.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(0xa1a1aa))
                                .child("尚未添加任何服务商"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .px(px(12.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(rgb(0x3f3f46))
                                .bg(rgb(0x27272a))
                                .text_size(px(12.0))
                                .text_color(rgb(0xededee))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x323238)))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                    this.open_editor(None, window, cx);
                                }))
                                .child("+ 添加服务商"),
                        )
                } else {
                    // 服务商列表项集合
                    div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .children(providers.iter().enumerate().map(|(idx, p)| {
                            render_provider_row(
                                p,
                                idx == 0,
                                latencies.get(&p.id).copied(),
                                testing.get(&p.id).copied().unwrap_or(false),
                                cx,
                            )
                        }))
                })
                .child(
                    // 原版面板最底部：全宽内嵌「+ 添加服务商」按钮
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w_full()
                        .h(px(38.0))
                        .border_t_1()
                        .border_color(rgb(0x2e2e32))
                        .bg(rgb(0x1c1c1f))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x232327)))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                            this.open_editor(None, window, cx);
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xa1a1aa))
                                .hover(|s| s.text_color(rgb(0xffffff)))
                                .child("+ 添加服务商"),
                        ),
                ),
        )
}

fn render_provider_row(
    provider: &Provider,
    _is_first: bool,
    latency: Option<i64>,
    is_testing: bool,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let p_clone = provider.clone();
    let p_id = provider.id.clone();
    let is_enabled = provider.enabled;
    let name = provider.name.clone();
    let protocol = provider.protocol.clone();
    let base_url = provider.base_url.clone();

    // 格式化简短端点 (去除 http://, https://, /v1)
    let display_endpoint = base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches("/v1")
        .trim_end_matches('/')
        .to_string();

    let is_subscription = provider.auth_type.as_deref() == Some("subscription_pool")
        || provider.preset_id.as_deref() == Some("antigravity")
        || provider.preset_id.as_deref() == Some("chatgpt_subscription")
        || protocol == "chatgpt";

    // 品牌首字母或标识
    let initial = name.chars().next().unwrap_or('P').to_uppercase().to_string();

    div()
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .h(px(46.0))
        .px(px(16.0))
        .border_b_1()
        .border_color(rgb(0x2e2e32))
        .bg(if is_enabled {
            rgb(0x1c1c1f)
        } else {
            rgba(0x1c1c1f80)
        })
        .hover(|s| s.bg(rgb(0x242428)))
        .child(
            // 左侧：24×24 Avatar + 名称 + 订阅/端点标签 (点击打开编辑)
            div()
                .flex()
                .flex_1()
                .h_full()
                .items_center()
                .cursor_pointer()
                .gap(px(10.0))
                .on_mouse_down(MouseButton::Left, cx.listener({
                    let p = p_clone.clone();
                    move |this, _, window, cx| {
                        this.open_editor(Some(p.clone()), window, cx);
                    }
                }))
                .child(
                    // 统一 24×24 品牌图标槽
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(24.0))
                        .rounded(px(5.0))
                        .bg(rgb(0x2e2e34))
                        .border_1()
                        .border_color(rgb(0x3f3f46))
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xd4d4d8))
                        .child(initial),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if is_enabled {
                                    rgb(0xededee)
                                } else {
                                    rgb(0x71717a)
                                })
                                .child(name),
                        )
                        .child(if is_subscription {
                            // 订阅池徽标
                            div()
                                .flex()
                                .items_center()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded(px(4.0))
                                .bg(rgba(0x818cf820))
                                .border_1()
                                .border_color(rgba(0x818cf840))
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xa5b4fc))
                                .child("官方订阅 · 1")
                        } else if !display_endpoint.is_empty() {
                            // 极简端点
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(0x71717a))
                                .child(display_endpoint)
                        } else {
                            div()
                        }),
                ),
        )
        .child(
            // 右侧：测速药丸 + 启停开关
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(if !is_enabled {
                    div()
                } else if is_testing {
                    div()
                        .flex()
                        .items_center()
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .bg(rgb(0x27272a))
                        .text_size(px(11.0))
                        .text_color(rgb(0xa1a1aa))
                        .child("测速中...")
                } else if let Some(ms) = latency {
                    if ms > 0 {
                        let (bg_color, text_color, dot_color) = if ms < 300 {
                            (rgba(0x10b98120), rgb(0x34d399), rgb(0x10b981))
                        } else if ms < 1000 {
                            (rgba(0xf59e0b20), rgb(0xfbbf24), rgb(0xf59e0b))
                        } else {
                            (rgba(0xf9731620), rgb(0xfb923c), rgb(0xf97316))
                        };

                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .px(px(7.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .bg(bg_color)
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let id = p_id.clone();
                                move |this, _, _, cx| {
                                    this.test_provider_latency(id.clone(), cx);
                                }
                            }))
                            .child(div().size(px(5.0)).rounded_full().bg(dot_color))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(text_color)
                                    .child(format!("{}ms", ms)),
                            )
                    } else {
                        // 测速失败
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .px(px(7.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .bg(rgba(0xf43f5e20))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let id = p_id.clone();
                                move |this, _, _, cx| {
                                    this.test_provider_latency(id.clone(), cx);
                                }
                            }))
                            .child(div().size(px(5.0)).rounded_full().bg(rgb(0xf43f5e)))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xfb7185))
                                    .child("连接失败"),
                            )
                    }
                } else {
                    // 未测速状态：轻量 ⚡ 按钮
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .p(px(4.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .text_color(rgb(0x71717a))
                        .hover(|s| s.bg(rgb(0x27272a)).text_color(rgb(0xd4d4d8)))
                        .on_mouse_down(MouseButton::Left, cx.listener({
                            let id = p_id.clone();
                            move |this, _, _, cx| {
                                this.test_provider_latency(id.clone(), cx);
                            }
                        }))
                        .child("⚡")
                })
                .child(
                    NativeSwitch::new(is_enabled).on_toggle(cx.listener({
                        let id = p_id.clone();
                        move |this, _, _, cx| {
                            this.toggle_provider(id.clone(), !is_enabled, cx);
                        }
                    })),
                ),
        )
}
