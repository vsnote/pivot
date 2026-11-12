use crate::components::button::NativeButton;
use crate::components::icons::{lucide_icon, render_client_avatar};
use crate::i18n::client_type_label;
use crate::theme::ThemePalette;
use crate::PivotApp;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pivot_core::interceptor::InterceptorInfo;

fn build_env_export_cmd(client_id: &str, port: u16) -> String {
    match client_id {
        "claude_code" => format!(
            "export ANTHROPIC_BASE_URL=\"http://127.0.0.1:{}\" ANTHROPIC_API_KEY=\"pivot\"",
            port
        ),
        "gemini_cli" => format!(
            "export GOOGLE_GEMINI_BASE_URL=\"http://127.0.0.1:{}\" GEMINI_API_KEY=\"pivot\"",
            port
        ),
        _ => format!(
            "export OPENAI_BASE_URL=\"http://127.0.0.1:{}/v1\" OPENAI_API_KEY=\"pivot\"",
            port
        ),
    }
}

fn target_gateway_endpoint(client_id: &str, port: u16) -> String {
    match client_id {
        "claude_code" | "gemini_cli" => format!("http://127.0.0.1:{}", port),
        _ => format!("http://127.0.0.1:{}/v1", port),
    }
}

pub fn render_conflict_dialog(
    app: &PivotApp,
    target: &InterceptorInfo,
    busy: bool,
    palette: ThemePalette,
    cx: &mut Context<PivotApp>,
) -> AnyElement {
    let locale = app.prefs.locale;
    let port = app.gateway_status.port;
    let has_enabled = app.providers.iter().any(|p| p.enabled);
    let label = client_type_label(&target.id, locale);
    let target_id = target.id.clone();

    let current_ep = target
        .third_party
        .as_ref()
        .map(|tp| tp.endpoint.clone())
        .or_else(|| target.conflict.clone())
        .or_else(|| target.original_endpoint.clone())
        .unwrap_or_else(|| locale.pick("第三方自定义配置", "Custom configuration").to_string());

    let gateway_ep = target_gateway_endpoint(&target.id, port);
    let env_cmd = build_env_export_cmd(&target.id, port);
    let copy_key = format!("conflict_env_{}", target.id);
    let is_copied = app.copied_id.as_deref() == Some(copy_key.as_str());
    let copy_cmd = env_cmd.clone();

    div()
        .id("agent_conflict_backdrop")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .px(px(16.0))
        .bg(palette.overlay_backdrop)
        .on_scroll(|_, _, app_cx| {
            app_cx.stop_propagation();
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                if !busy {
                    this.conflict_target = None;
                    cx.notify();
                }
            }),
        )
        .child(
            div()
                .w_full()
                .max_w(px(376.0))
                .rounded(px(8.0))
                .bg(palette.popover)
                .border_1()
                .border_color(palette.border)
                .shadow(palette.modal_shadows())
                .overflow_hidden()
                .occlude()
                .flex()
                .flex_col()
                .on_mouse_down(MouseButton::Left, |_, _, app_cx| {
                    app_cx.stop_propagation();
                })
                // 头部
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .px(px(16.0))
                        .py(px(12.0))
                        .border_b_1()
                        .border_color(palette.border)
                        .child(render_client_avatar(&target.id, 24.0, true, palette))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(palette.foreground)
                                .truncate()
                                .child(if target.is_overwritten {
                                    format!(
                                        "{} {}",
                                        label,
                                        locale.pick("代理配置已被外部修改", "Config Overwritten")
                                    )
                                } else {
                                    format!(
                                        "{} {}",
                                        label,
                                        locale.pick("检测到现有代理配置", "Existing Config Detected")
                                    )
                                }),
                        ),
                )
                // 主体说明
                .child(
                    div()
                        .p(px(16.0))
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        // 端点对比卡
                        .child(
                            div()
                                .p(px(10.0))
                                .rounded(px(6.0))
                                .bg(palette.muted_subtle)
                                .border_1()
                                .border_color(palette.border_subtle)
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(palette.muted_foreground)
                                                .flex_shrink_0()
                                                .child(locale.pick("当前配置", "Current")),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(palette.amber_text)
                                                .truncate()
                                                .child(current_ep),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(palette.muted_foreground)
                                                .flex_shrink_0()
                                                .child(locale.pick("接管指向", "Pivot Gateway")),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(palette.emerald_text)
                                                .truncate()
                                                .child(gateway_ep),
                                        ),
                                ),
                        )
                        // 自动备份说明
                        .child(
                            div()
                                .text_size(px(11.5))
                                .line_height(rems(1.4))
                                .text_color(palette.muted_foreground)
                                .child(format!(
                                    "{} ({})",
                                    locale.pick(
                                        "确认接管前将自动备份原配置文件，关闭开关即可一键无损还原",
                                        "Original config will be backed up automatically and restored when turned off",
                                    ),
                                    target.config_path
                                )),
                        )
                        // 无已启用服务商提醒
                        .when(!has_enabled, |el| {
                            el.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .px(px(8.0))
                                    .py(px(6.0))
                                    .rounded(px(5.0))
                                    .bg(palette.amber_soft_bg)
                                    .border_1()
                                    .border_color(palette.amber_soft_border)
                                    .child(lucide_icon(
                                        IconName::TriangleAlert,
                                        12.5,
                                        palette.amber_text,
                                    ))
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(11.0))
                                            .text_color(palette.amber_text)
                                            .child(locale.pick(
                                                "当前暂无已启用的服务商，接管后请记得启用上游渠道",
                                                "No enabled providers yet — enable one after takeover",
                                            )),
                                    ),
                            )
                        })
                        // 临时终端命令替代方案
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(palette.muted_foreground)
                                                .child(locale.pick(
                                                    "或仅在当前终端临时生效：",
                                                    "Or export for a single terminal session:",
                                                )),
                                        )
                                        .child(
                                            div()
                                                .id("copy_conflict_env_btn")
                                                .flex()
                                                .items_center()
                                                .gap(px(3.5))
                                                .cursor_pointer()
                                                .text_size(px(10.5))
                                                .text_color(if is_copied {
                                                    palette.emerald_text
                                                } else {
                                                    palette.muted_foreground
                                                })
                                                .hover(move |s| s.text_color(palette.foreground))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.copy_to_clipboard(
                                                            copy_key.clone(),
                                                            copy_cmd.clone(),
                                                            cx,
                                                        );
                                                    }),
                                                )
                                                .child(lucide_icon(
                                                    if is_copied {
                                                        IconName::Check
                                                    } else {
                                                        IconName::Copy
                                                    },
                                                    11.0,
                                                    if is_copied {
                                                        palette.emerald_text
                                                    } else {
                                                        palette.muted_foreground
                                                    },
                                                ))
                                                .child(if is_copied {
                                                    locale.pick("已复制", "Copied")
                                                } else {
                                                    locale.pick("复制", "Copy")
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .px(px(8.0))
                                        .py(px(6.0))
                                        .rounded(px(5.0))
                                        .bg(palette.background)
                                        .border_1()
                                        .border_color(palette.border_subtle)
                                        .text_size(px(10.5))
                                        .text_color(palette.muted_foreground)
                                        .truncate()
                                        .child(env_cmd),
                                ),
                        ),
                )
                // 底部按钮栏
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(8.0))
                        .px(px(16.0))
                        .py(px(11.0))
                        .border_t_1()
                        .border_color(palette.border)
                        .child(
                            NativeButton::new(locale.pick("取消", "Cancel"), palette)
                                .secondary()
                                .sm()
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.conflict_target = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            NativeButton::new(
                                if busy {
                                    locale.pick("处理中…", "Applying…")
                                } else {
                                    locale.pick("确认接管", "Confirm Takeover")
                                },
                                palette,
                            )
                            .primary()
                            .sm()
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.confirm_conflict_takeover(target_id.clone(), cx);
                            })),
                        ),
                ),
        )
        .into_any_element()
}
