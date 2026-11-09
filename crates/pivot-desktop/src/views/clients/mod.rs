use crate::PivotApp;
use gpui_kit::component::switch::Switch;
use gpui_kit::*;
use pivot_core::interceptor::InterceptorInfo;

pub fn render_clients_view(
    app: &PivotApp,
    _window: &mut Window,
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let interceptors = app.interceptors.clone();
    let providers = app.providers.clone();
    let port = app.port;

    // 收集所有已启用服务商声明的有效模型列表
    let mut available_models = Vec::new();
    for p in &providers {
        if p.enabled {
            for m in &p.models {
                let trimmed = m.trim().to_string();
                if !trimmed.is_empty() && !available_models.contains(&trimmed) {
                    available_models.push(trimmed);
                }
            }
        }
    }

    let has_enabled_providers = providers.iter().any(|p| p.enabled);

    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(960.0))
        .mx_auto()
        .gap(px(14.0))
        // 卡片 1：已接管客户端 (IngressCard)
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .rounded(px(8.0))
                .border_1()
                .border_color(rgb(0x2e2e32))
                .bg(rgb(0x1c1c1f))
                .overflow_hidden()
                .children(interceptors.iter().map(|client| {
                    render_client_row(client, &available_models, cx)
                })),
        )
        // 列表底部说明文字
        .child(
            div()
                .px(px(4.0))
                .text_size(px(11.0))
                .text_color(if !has_enabled_providers {
                    rgb(0xfbbf24)
                } else {
                    rgb(0x71717a)
                })
                .child(if !has_enabled_providers {
                    "⚠️ 当前没有已启用的模型服务商，接管客户端可能无法正常发起对话，请先在「服务商」中添加并启用服务商。"
                } else {
                    "开启接管后，对应 CLI/Agent 将自动通过本地 Pivot 网关分发请求，关闭时即刻恢复原生配置。"
                }),
        )
        // 卡片 2：通用客户端直连配置 (UniversalClientCard)
        .child(render_universal_client_card(port))
}

fn render_client_row(
    client: &InterceptorInfo,
    available_models: &[String],
    cx: &mut Context<PivotApp>,
) -> impl IntoElement {
    let client_id = client.id.clone();
    let is_enabled = client.is_intercepted;
    let is_installed = client.installed;
    let current_model = client.model.clone().unwrap_or_default();

    // 客户端品牌简写或首字
    let badge_mark = match client.id.as_str() {
        "claude_code" => "C",
        "codex_cli" => "O",
        "gemini_cli" => "G",
        "pi" => "P",
        "opencode" => "OC",
        _ => "CLI",
    };

    let models_clone = available_models.to_vec();

    div()
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .h(px(48.0))
        .px(px(16.0))
        .border_b_1()
        .border_color(rgb(0x2e2e32))
        .bg(rgb(0x1c1c1f))
        .child(
            // 左侧：24×24 品牌图标槽 + 客户端名称
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
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
                        .child(badge_mark),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if is_installed {
                            rgb(0xededee)
                        } else {
                            rgb(0x71717a)
                        })
                        .child(client.name.clone()),
                ),
        )
        .child(
            // 右侧：模型选择器（原版 ClientModelSelect）+ 接管开关
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(if is_installed {
                    div().flex().items_center().gap(px(8.0)).child(
                        // 独立模型选择按钮（点击循环切换）
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(5.0))
                            .border_1()
                            .border_color(rgb(0x3f3f46))
                            .bg(rgb(0x27272a))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x323238)))
                            .on_mouse_down(MouseButton::Left, cx.listener({
                                let cid = client_id.clone();
                                let cur = current_model.clone();
                                let m_list = models_clone.clone();
                                move |this, _, _, cx| {
                                    if m_list.is_empty() {
                                        return;
                                    }
                                    let next_model = if cur.is_empty() {
                                        m_list.first().cloned()
                                    } else if let Some(idx) = m_list.iter().position(|m| m == &cur) {
                                        if idx + 1 < m_list.len() {
                                            Some(m_list[idx + 1].clone())
                                        } else {
                                            Some(m_list[0].clone())
                                        }
                                    } else {
                                        m_list.first().cloned()
                                    };
                                    this.set_interceptor_model(cid.clone(), next_model, cx);
                                }
                            }))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(if current_model.is_empty() {
                                        rgb(0x71717a)
                                    } else {
                                        rgb(0x60a5fa)
                                    })
                                    .child(if current_model.is_empty() {
                                        "未选模型 (点击选择)".to_string()
                                    } else {
                                        current_model.clone()
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(0xa1a1aa))
                                    .child("▾"),
                            ),
                    )
                } else {
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0x71717a))
                        .child("未安装此客户端")
                })
                .child(if is_installed {
                    div().child(
                        Switch::new(format!("client-switch-{}", client_id))
                            .checked(is_enabled)
                            .on_click(cx.listener({
                                let cid = client_id.clone();
                                move |this, checked: &bool, _, cx| {
                                    this.toggle_interceptor(cid.clone(), *checked, cx);
                                }
                            })),
                    )
                } else {
                    div()
                }),
        )
}

fn render_universal_client_card(port: u16) -> impl IntoElement {
    let openai_url = format!("http://127.0.0.1:{}/v1", port);
    let anthropic_url = format!("http://127.0.0.1:{}", port);

    div()
        .flex()
        .flex_col()
        .w_full()
        .rounded(px(8.0))
        .border_1()
        .border_color(rgb(0x2e2e32))
        .bg(rgb(0x1c1c1f))
        .overflow_hidden()
        .child(
            // 卡片头部
            div()
                .flex()
                .items_center()
                .justify_between()
                .w_full()
                .px(px(16.0))
                .py(px(10.0))
                .bg(rgb(0x222226))
                .border_b_1()
                .border_color(rgb(0x2e2e32))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(0xededee))
                                .child("通用客户端连接 (Cursor / Continue / Roo Code)"),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .p(px(16.0))
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0xa1a1aa))
                        .child("任何支持自定义 API Base URL 的应用均可填入以下本地端点，API Key 任意填写即可："),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(12.0))
                        .child(
                            // OpenAI 兼容端点
                            div()
                                .flex_1()
                                .p(px(12.0))
                                .rounded(px(6.0))
                                .bg(rgb(0x232327))
                                .border_1()
                                .border_color(rgb(0x2e2e32))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .mb(px(6.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(0xededee))
                                                .child("OpenAI 兼容端点"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.0))
                                                .text_color(rgb(0x71717a))
                                                .child("/v1/chat/completions"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(rgb(0x60a5fa))
                                        .child(openai_url),
                                ),
                        )
                        .child(
                            // Anthropic 兼容端点
                            div()
                                .flex_1()
                                .p(px(12.0))
                                .rounded(px(6.0))
                                .bg(rgb(0x232327))
                                .border_1()
                                .border_color(rgb(0x2e2e32))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .mb(px(6.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(0xededee))
                                                .child("Anthropic 兼容端点"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.0))
                                                .text_color(rgb(0x71717a))
                                                .child("/v1/messages"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(rgb(0xa78bfa))
                                        .child(anthropic_url),
                                ),
                        ),
                ),
        )
}
