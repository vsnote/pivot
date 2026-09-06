use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let show_i = MenuItem::with_id(app, "show", "Open OmniSwitch", true, None::<&str>)?;
    let status_i = MenuItem::with_id(
        app,
        "status",
        format!("Gateway: Running (127.0.0.1:{})", port),
        false,
        None::<&str>,
    )?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit OmniSwitch", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&status_i, &show_i, &quit_i])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip(format!("OmniSwitch - Port {}", port))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;

    Ok(())
}
