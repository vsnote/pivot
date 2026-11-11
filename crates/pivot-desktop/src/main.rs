#![allow(unexpected_cfgs)]

mod app;
mod components;
mod i18n;
mod pricing;
mod theme;
mod tray;
mod views;

pub use app::{ActiveTab, PivotApp};

use gpui_kit::*;
use std::sync::Arc;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // 1. 初始化 Tokio 多线程运行时
    let tokio_rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(4)
            .build()
            .expect("Failed to build Tokio runtime"),
    );
    let tokio_handle = tokio_rt.handle().clone();

    // 2. 自动避让正式版 16888 端口，绝不冲突
    let custom_port = std::env::var("PIVOT_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .or_else(|| {
            if std::net::TcpListener::bind(("127.0.0.1", pivot_core::DEFAULT_PORT)).is_err() {
                (16889..=16899)
                    .find(|&p| std::net::TcpListener::bind(("127.0.0.1", p)).is_ok())
            } else {
                None
            }
        });

    // 3. 初始化核心状态并启动后台网关与实时日志广播通道
    let app_state = pivot_core::init_core(custom_port).expect("Failed to initialize pivot core");
    let (log_tx, log_rx) = tokio::sync::broadcast::channel(256);
    {
        let _rt_guard = tokio_rt.enter();
        pivot_core::start_gateway(app_state.clone(), Some(log_tx));
    }

    // 4. 启动 GPUI 原生桌面应用并按需注入轻量矢量图标资产
    gpui_kit::application()
        .with_assets(components::icons::PivotAssets)
        .run(move |cx| {
            gpui_kit::init(cx);

            let tray_rx = tray::init_system_tray(app_state.port);
            let bounds = Bounds::centered(None, size(px(415.0), px(620.0)), cx);

            let handle_clone = tokio_handle.clone();
            let state_clone = app_state.clone();

            gpui_kit::open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("Pivot".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(14.0), px(17.0))),
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(415.0), px(460.0))),
                    ..Default::default()
                },
                cx,
                move |window, cx| {
                    window.on_window_should_close(cx, |_window, cx| {
                        if tray::should_quit() {
                            true
                        } else {
                            cx.hide();
                            false
                        }
                    });
                    cx.new(|cx| {
                        let app = PivotApp::new(state_clone, handle_clone, log_rx, tray_rx, window, cx);
                        let is_dark = theme::is_dark_appearance(app.prefs.theme, window);
                        tray::sync_macos_app_icon(is_dark);
                        app
                    })
                },
            )
            .expect("Failed to open Pivot GPUI window");

            cx.activate(true);
        });
}
