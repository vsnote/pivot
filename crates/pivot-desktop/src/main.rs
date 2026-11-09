mod components;
mod views;

use components::header::render_header;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::*;
use pivot_core::db::{Provider, RequestLog};
use pivot_core::interceptor::InterceptorInfo;
use pivot_core::service;
use pivot_core::state::AppState;
use std::collections::HashMap;
use views::clients::render_clients_view;
use views::dashboard::render_dashboard_view;
use views::providers::{render_provider_editor, render_provider_list};
use views::settings::render_settings_view;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActiveTab {
    Providers,
    Clients,
    Logs,
    Settings,
}

pub struct PivotApp {
    pub core_state: AppState,
    pub active_tab: ActiveTab,
    pub port: u16,
    pub providers: Vec<Provider>,
    pub interceptors: Vec<InterceptorInfo>,
    pub logs: Vec<RequestLog>,
    pub latencies: HashMap<String, i64>,
    pub testing_latencies: HashMap<String, bool>,
    pub is_provider_editor_open: bool,
    pub error_message: Option<String>,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_requests: u64,

    // 编辑器内部状态
    pub editor_provider_id: Option<String>,
    pub editor_protocol: String,
    pub editor_name: Option<Entity<InputState>>,
    pub editor_url: Option<Entity<InputState>>,
    pub editor_key: Option<Entity<InputState>>,
    pub editor_models: Option<Entity<InputState>>,
}

impl PivotApp {
    pub fn new(core_state: AppState, cx: &mut Context<Self>) -> Self {
        let port = core_state.port;
        let mut app = Self {
            core_state,
            active_tab: ActiveTab::Providers,
            port,
            providers: Vec::new(),
            interceptors: Vec::new(),
            logs: Vec::new(),
            latencies: HashMap::new(),
            testing_latencies: HashMap::new(),
            is_provider_editor_open: false,
            error_message: None,
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            total_requests: 0,
            editor_provider_id: None,
            editor_protocol: "openai".to_string(),
            editor_name: None,
            editor_url: None,
            editor_key: None,
            editor_models: None,
        };

        app.refresh_all(cx);
        app
    }

    pub fn refresh_all(&mut self, cx: &mut Context<Self>) {
        let state = self.core_state.clone();
        cx.spawn(async move |this, cx| {
            let providers_res = service::get_providers(&state).await;
            let interceptors_res = service::get_interceptor_status(&state).await;
            let logs_res = service::get_logs(&state, Some(100), None).await;

            let _ = this.update(cx, |app: &mut PivotApp, cx: &mut Context<PivotApp>| {
                if let Ok(providers) = providers_res {
                    app.providers = providers;
                }
                if let Ok(interceptors) = interceptors_res {
                    app.interceptors = interceptors;
                }
                if let Ok(logs) = logs_res {
                    let mut p_tokens = 0u64;
                    let mut c_tokens = 0u64;
                    for log in &logs {
                        p_tokens += log.prompt_tokens.max(0) as u64;
                        c_tokens += log.completion_tokens.max(0) as u64;
                    }
                    app.total_prompt_tokens = p_tokens;
                    app.total_completion_tokens = c_tokens;
                    app.total_requests = logs.len() as u64;
                    app.logs = logs;
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn open_editor(
        &mut self,
        provider: Option<Provider>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (id, protocol, name, base_url, key, models) = match provider {
            Some(p) => (
                Some(p.id),
                p.protocol,
                p.name,
                p.base_url,
                p.api_keys.first().cloned().unwrap_or_default(),
                p.models.join(", "),
            ),
            None => (
                None,
                "openai".to_string(),
                "新服务商".to_string(),
                "https://api.openai.com/v1".to_string(),
                "".to_string(),
                "gpt-4o, gpt-4o-mini".to_string(),
            ),
        };

        self.editor_provider_id = id;
        self.editor_protocol = protocol;
        self.editor_name = Some(cx.new(|cx| InputState::new(window, cx).default_value(name)));
        self.editor_url = Some(cx.new(|cx| InputState::new(window, cx).default_value(base_url)));
        self.editor_key = Some(cx.new(|cx| InputState::new(window, cx).default_value(key)));
        self.editor_models = Some(cx.new(|cx| InputState::new(window, cx).default_value(models)));
        self.is_provider_editor_open = true;
        cx.notify();
    }

    pub fn close_editor(&mut self, cx: &mut Context<Self>) {
        self.is_provider_editor_open = false;
        self.editor_provider_id = None;
        self.editor_name = None;
        self.editor_url = None;
        self.editor_key = None;
        self.editor_models = None;
        cx.notify();
    }

    pub fn save_current_editor(&mut self, cx: &mut Context<Self>) {
        let id = self
            .editor_provider_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let protocol = self.editor_protocol.clone();

        let name = self
            .editor_name
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();
        let base_url = self
            .editor_url
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();
        let api_key = self
            .editor_key
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();
        let models_str = self
            .editor_models
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();

        let models: Vec<String> = models_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let provider = Provider {
            id,
            name,
            protocol,
            base_url,
            preset_id: None,
            auth_type: None,
            api_keys: if api_key.trim().is_empty() {
                vec![]
            } else {
                vec![api_key.trim().to_string()]
            },
            models,
            default_model: None,
            is_default: false,
            enabled: true,
            priority: 0,
            weight: 1,
            timeout_secs: 120,
            latency_ms: None,
            created_at: None,
            updated_at: None,
        };

        let state = self.core_state.clone();
        cx.spawn(async move |this, cx| {
            let res = service::save_provider(&state, provider).await;
            let _ = this.update(cx, |app: &mut PivotApp, cx: &mut Context<PivotApp>| {
                match res {
                    Ok(_) => {
                        app.close_editor(cx);
                        app.refresh_all(cx);
                    }
                    Err(e) => {
                        app.error_message = Some(e);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub fn delete_provider(&mut self, id: String, cx: &mut Context<Self>) {
        let state = self.core_state.clone();
        cx.spawn(async move |this, cx| {
            let res = service::delete_provider(&state, id).await;
            let _ = this.update(
                cx,
                |app: &mut PivotApp, cx: &mut Context<PivotApp>| match res {
                    Ok(_) => app.refresh_all(cx),
                    Err(e) => {
                        app.error_message = Some(e);
                        cx.notify();
                    }
                },
            );
        })
        .detach();
    }

    pub fn toggle_provider(&mut self, id: String, enabled: bool, cx: &mut Context<Self>) {
        let state = self.core_state.clone();
        cx.spawn(async move |this, cx| {
            let res = service::set_provider_enabled(&state, id, enabled).await;
            let _ = this.update(
                cx,
                |app: &mut PivotApp, cx: &mut Context<PivotApp>| match res {
                    Ok(_) => app.refresh_all(cx),
                    Err(e) => {
                        app.error_message = Some(e);
                        cx.notify();
                    }
                },
            );
        })
        .detach();
    }

    pub fn test_provider_latency(&mut self, id: String, cx: &mut Context<Self>) {
        self.testing_latencies.insert(id.clone(), true);
        cx.notify();

        let state = self.core_state.clone();
        let target_id = id.clone();
        cx.spawn(async move |this, cx| {
            let res = service::test_provider_latency(&state, target_id.clone()).await;
            let _ = this.update(cx, |app: &mut PivotApp, cx: &mut Context<PivotApp>| {
                app.testing_latencies.remove(&target_id);
                if let Ok(latency) = res {
                    app.latencies.insert(target_id, latency);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn toggle_interceptor(&mut self, id: String, enabled: bool, cx: &mut Context<Self>) {
        let state = self.core_state.clone();
        cx.spawn(async move |this, cx| {
            let res = service::toggle_interceptor(&state, id, enabled).await;
            let _ = this.update(
                cx,
                |app: &mut PivotApp, cx: &mut Context<PivotApp>| match res {
                    Ok(_) => app.refresh_all(cx),
                    Err(e) => {
                        app.error_message = Some(e);
                        cx.notify();
                    }
                },
            );
        })
        .detach();
    }

    pub fn set_interceptor_model(
        &mut self,
        id: String,
        model: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let state = self.core_state.clone();
        cx.spawn(async move |this, cx| {
            let res = service::set_interceptor_model(&state, id, model).await;
            let _ = this.update(
                cx,
                |app: &mut PivotApp, cx: &mut Context<PivotApp>| match res {
                    Ok(_) => app.refresh_all(cx),
                    Err(e) => {
                        app.error_message = Some(e);
                        cx.notify();
                    }
                },
            );
        })
        .detach();
    }

    pub fn clear_logs(&mut self, cx: &mut Context<Self>) {
        let state = self.core_state.clone();
        cx.spawn(async move |this, cx| {
            let res = service::clear_logs(&state).await;
            let _ = this.update(cx, |app: &mut PivotApp, cx: &mut Context<PivotApp>| {
                if res.is_ok() {
                    app.logs.clear();
                    app.total_requests = 0;
                    app.total_prompt_tokens = 0;
                    app.total_completion_tokens = 0;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

impl Render for PivotApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;
        let is_editor_open = self.is_provider_editor_open;
        let error_opt = self.error_message.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x18181b))
            .text_color(rgb(0xffffff))
            .child(render_header(active_tab, self.port, window, cx))
            // 错误提示条
            .when_some(error_opt, |el, err| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mx(px(16.0))
                        .mt(px(10.0))
                        .p(px(10.0))
                        .rounded(px(8.0))
                        .bg(rgba(0xef444415))
                        .border_1()
                        .border_color(rgba(0xef444430))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(0xf87171))
                                .child(err),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .px(px(6.0))
                                .py(px(2.0))
                                .text_size(px(11.0))
                                .text_color(rgb(0xfca5a5))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.error_message = None;
                                    cx.notify();
                                }))
                                .child("✕"),
                        ),
                )
            })
            // 主视图内容区域
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .p(px(20.0))
                    .overflow_y_scrollbar()
                    .child(match active_tab {
                        ActiveTab::Providers => {
                            if is_editor_open {
                                div().w_full().child(render_provider_editor(self, window, cx))
                            } else {
                                div().w_full().child(render_provider_list(self, window, cx))
                            }
                        }
                        ActiveTab::Clients => {
                            div().w_full().child(render_clients_view(self, window, cx))
                        }
                        ActiveTab::Logs => {
                            div().w_full().child(render_dashboard_view(self, window, cx))
                        }
                        ActiveTab::Settings => {
                            div().w_full().child(render_settings_view(self, window, cx))
                        }
                    }),
            )
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let custom_port = std::env::var("PIVOT_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok());

    // 1. 初始化核心业务状态
    let app_state = pivot_core::init_core(custom_port).expect("Failed to initialize pivot core");

    // 2. 启动后台 Tokio Axum Gateway 网关服务器
    pivot_core::start_gateway(app_state.clone(), None);

    // 3. 启动 GPUI 桌面渲染事件循环
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);

        gpui_kit::open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Pivot".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(14.0), px(14.0))),
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(120.0), px(100.0)),
                    size: size(px(1120.0), px(780.0)),
                })),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|cx| PivotApp::new(app_state, cx)),
        )
        .expect("Failed to open GPUI window");
    });
}
