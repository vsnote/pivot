#![allow(unexpected_cfgs, deprecated)]

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::OnceLock;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    ShowWindow,
    QuitApp,
}

static SHOULD_QUIT: AtomicBool = AtomicBool::new(false);
static LAST_DOCK_THEME: AtomicU8 = AtomicU8::new(255);
static TRAY_TX: OnceLock<mpsc::UnboundedSender<TrayCommand>> = OnceLock::new();

pub fn mark_quitting() {
    SHOULD_QUIT.store(true, Ordering::SeqCst);
}

pub fn should_quit() -> bool {
    SHOULD_QUIT.load(Ordering::SeqCst)
}

/// 同步 macOS 应用级图标（Dock 与 Cmd+Tab 切换器），仅在首次或深浅色变化时更新
pub fn sync_macos_app_icon(is_dark: bool) {
    let next = u8::from(is_dark);
    if LAST_DOCK_THEME.swap(next, Ordering::SeqCst) == next {
        return;
    }
    #[cfg(target_os = "macos")]
    macos::set_app_dock_icon(is_dark);
}

pub fn init_system_tray(port: u16) -> mpsc::UnboundedReceiver<TrayCommand> {
    let (tx, rx) = mpsc::unbounded_channel();
    let _ = TRAY_TX.set(tx);

    #[cfg(target_os = "macos")]
    macos::setup_status_item(port);

    #[cfg(not(target_os = "macos"))]
    let _ = port;

    rx
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{mark_quitting, TrayCommand, TRAY_TX};
    use cocoa::appkit::{NSStatusBar, NSVariableStatusItemLength};
    use cocoa::base::{id, nil, NO, YES};
    use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};
    use std::os::raw::c_void;
    use std::sync::Once;

    static REGISTER_TARGET_CLASS: Once = Once::new();
    const TRAY_PNG: &[u8] = include_bytes!("../assets/tray-icon.png");
    const APP_ICON_LIGHT_PNG: &[u8] = include_bytes!("../assets/app-icon-light.png");
    const APP_ICON_DARK_PNG: &[u8] = include_bytes!("../assets/app-icon-dark.png");

    pub fn set_app_dock_icon(is_dark: bool) {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            let bytes = if is_dark {
                APP_ICON_DARK_PNG
            } else {
                APP_ICON_LIGHT_PNG
            };
            let ns_data: id = msg_send![
                class!(NSData),
                dataWithBytes: bytes.as_ptr() as *const c_void
                length: bytes.len()
            ];
            if ns_data != nil {
                let ns_image: id = msg_send![class!(NSImage), alloc];
                let ns_image: id = msg_send![ns_image, initWithData: ns_data];
                if ns_image != nil {
                    let ns_app: id = msg_send![class!(NSApplication), sharedApplication];
                    if ns_app != nil {
                        let _: () = msg_send![ns_app, setApplicationIconImage: ns_image];
                    }
                    let _: () = msg_send![ns_image, release];
                }
            }
        }
    }

    extern "C" fn handle_open_pivot(_this: &Object, _cmd: Sel, _sender: id) {
        unsafe {
            let ns_app: id = msg_send![class!(NSApplication), sharedApplication];
            if ns_app != nil {
                let _: () = msg_send![ns_app, unhide: nil];
                let _: () = msg_send![ns_app, activateIgnoringOtherApps: YES];
            }
        }
        if let Some(tx) = TRAY_TX.get() {
            let _ = tx.send(TrayCommand::ShowWindow);
        }
    }

    extern "C" fn handle_quit_pivot(_this: &Object, _cmd: Sel, _sender: id) {
        mark_quitting();
        if let Some(tx) = TRAY_TX.get() {
            let _ = tx.send(TrayCommand::QuitApp);
        }
    }

    fn ensure_target_class() -> *const Class {
        REGISTER_TARGET_CLASS.call_once(|| unsafe {
            if Class::get("PivotGpuiTrayTarget").is_none() {
                if let Some(mut decl) = ClassDecl::new("PivotGpuiTrayTarget", class!(NSObject)) {
                    decl.add_method(
                        sel!(onOpenPivot:),
                        handle_open_pivot as extern "C" fn(&Object, Sel, id),
                    );
                    decl.add_method(
                        sel!(onQuitPivot:),
                        handle_quit_pivot as extern "C" fn(&Object, Sel, id),
                    );
                    decl.register();
                }
            }
        });
        Class::get("PivotGpuiTrayTarget").unwrap()
    }

    pub fn setup_status_item(port: u16) {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            let target_cls = ensure_target_class();
            let target: id = msg_send![target_cls, new];
            let _: id = msg_send![target, retain];

            let status_bar = NSStatusBar::systemStatusBar(nil);
            let status_item: id = status_bar.statusItemWithLength_(NSVariableStatusItemLength);
            if status_item == nil {
                return;
            }
            // 常驻 retain 防止 statusItem 离开作用域后从菜单栏消失
            let _: id = msg_send![status_item, retain];

            // 加载内嵌模板图标
            let ns_data: id = msg_send![
                class!(NSData),
                dataWithBytes: TRAY_PNG.as_ptr() as *const c_void
                length: TRAY_PNG.len()
            ];
            if ns_data != nil {
                let ns_image: id = msg_send![class!(NSImage), alloc];
                let ns_image: id = msg_send![ns_image, initWithData: ns_data];
                if ns_image != nil {
                    let icon_size = NSSize::new(18.0, 18.0);
                    let _: () = msg_send![ns_image, setSize: icon_size];
                    let _: () = msg_send![ns_image, setTemplate: YES];

                    let button: id = msg_send![status_item, button];
                    if button != nil {
                        let _: () = msg_send![button, setImage: ns_image];
                    } else {
                        let _: () = msg_send![status_item, setImage: ns_image];
                    }
                }
            }

            // 构建状态栏下拉菜单
            let menu: id = msg_send![class!(NSMenu), new];
            let _: () = msg_send![menu, setAutoenablesItems: NO];

            // 1. 网关状态只读信息项
            let status_label = format!("Pivot · 127.0.0.1:{}", port);
            let status_title = NSString::alloc(nil).init_str(&status_label);
            let empty_key = NSString::alloc(nil).init_str("");
            let status_menu_item: id = msg_send![class!(NSMenuItem), alloc];
            let status_menu_item: id = msg_send![
                status_menu_item,
                initWithTitle: status_title
                action: nil
                keyEquivalent: empty_key
            ];
            let _: () = msg_send![status_menu_item, setEnabled: NO];
            let _: () = msg_send![menu, addItem: status_menu_item];

            // 2. 分隔线
            let sep: id = msg_send![class!(NSMenuItem), separatorItem];
            let _: () = msg_send![menu, addItem: sep];

            // 3. 打开主界面
            let open_title = NSString::alloc(nil).init_str("打开 Pivot");
            let open_item: id = msg_send![class!(NSMenuItem), alloc];
            let open_item: id = msg_send![
                open_item,
                initWithTitle: open_title
                action: sel!(onOpenPivot:)
                keyEquivalent: empty_key
            ];
            let _: () = msg_send![open_item, setTarget: target];
            let _: () = msg_send![open_item, setEnabled: YES];
            let _: () = msg_send![menu, addItem: open_item];

            // 4. 退出 Pivot
            let quit_title = NSString::alloc(nil).init_str("退出 Pivot");
            let quit_key = NSString::alloc(nil).init_str("q");
            let quit_item: id = msg_send![class!(NSMenuItem), alloc];
            let quit_item: id = msg_send![
                quit_item,
                initWithTitle: quit_title
                action: sel!(onQuitPivot:)
                keyEquivalent: quit_key
            ];
            let _: () = msg_send![quit_item, setTarget: target];
            let _: () = msg_send![quit_item, setEnabled: YES];
            let _: () = msg_send![menu, addItem: quit_item];

            let _: () = msg_send![status_item, setMenu: menu];
            let _ = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
        }
    }
}
