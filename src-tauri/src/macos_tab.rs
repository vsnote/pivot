#![allow(deprecated)]

#[cfg(target_os = "macos")]
pub mod imp {
    use cocoa::base::{id, nil, BOOL, NO, YES};
    use cocoa::foundation::{NSArray, NSSize, NSString};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::sync::Once;
    use tauri::{Emitter, Manager, WebviewWindow};

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {
        static NSToolbarFlexibleSpaceItemIdentifier: id;
    }

    static APP_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
    static SEGMENT_CONTROL: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
    static ADD_BUTTON: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
    static THEME_BUTTON: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
    static INIT: Once = Once::new();

    const TAB_IDS: &[&str] = &["providers", "clients", "routes", "gateway", "logs", "about"];
    const NAVIGATION_ID: &str = "com.omniswitch.navigation";
    const ADD_PROVIDER_ID: &str = "com.omniswitch.add-provider";
    const THEME_ID: &str = "com.omniswitch.theme";

    extern "C" fn on_segment_changed(_this: &Object, _cmd: Sel, sender: id) {
        unsafe {
            let index: isize = msg_send![sender, selectedSegment];
            if index >= 0 && (index as usize) < TAB_IDS.len() {
                let tab_id = TAB_IDS[index as usize];
                let handle_ptr = APP_HANDLE.load(Ordering::SeqCst);
                if !handle_ptr.is_null() {
                    let app: &tauri::AppHandle = &*(handle_ptr as *const tauri::AppHandle);
                    let _ = app.emit("native-tab-selected", tab_id);
                }
            }
        }
    }

    extern "C" fn on_add_provider(_this: &Object, _cmd: Sel, _sender: id) {
        emit_native_event("native-add-provider");
    }

    extern "C" fn on_theme_toggle(_this: &Object, _cmd: Sel, _sender: id) {
        emit_native_event("native-theme-toggle");
    }

    fn emit_native_event(event: &str) {
        let handle_ptr = APP_HANDLE.load(Ordering::SeqCst);
        if !handle_ptr.is_null() {
            let app: &tauri::AppHandle = unsafe { &*(handle_ptr as *const tauri::AppHandle) };
            let _ = app.emit(event, ());
        }
    }

    unsafe fn identifier_equals(identifier: id, expected: &str) -> bool {
        let expected = NSString::alloc(nil).init_str(expected);
        let equal: BOOL = msg_send![identifier, isEqualToString:expected];
        let _: () = msg_send![expected, release];
        equal == YES
    }

    unsafe fn action_item(
        identifier: id,
        symbol: &str,
        label: &str,
        target: id,
        action: Sel,
        control: &AtomicPtr<std::ffi::c_void>,
    ) -> id {
        let symbol_name = NSString::alloc(nil).init_str(symbol);
        let accessibility_label = NSString::alloc(nil).init_str(label);
        let image: id = msg_send![class!(NSImage),
            imageWithSystemSymbolName:symbol_name
            accessibilityDescription:accessibility_label
        ];
        let button: id = msg_send![class!(NSButton),
            buttonWithImage:image
            target:target
            action:action
        ];
        let _: () = msg_send![button, setBezelStyle:7usize];
        let _: () = msg_send![button, setControlSize:3usize];
        let control_size = NSSize::new(32.0, 32.0);
        let _: () = msg_send![button, setFrameSize:control_size];
        let _: () = msg_send![button, setToolTip:accessibility_label];
        control.store(button as *mut std::ffi::c_void, Ordering::SeqCst);

        let item: id = msg_send![class!(NSToolbarItem), alloc];
        let item: id = msg_send![item, initWithItemIdentifier:identifier];
        let _: () = msg_send![item, setView:button];
        let _: () = msg_send![item, setMinSize:control_size];
        let _: () = msg_send![item, setMaxSize:control_size];

        let _: () = msg_send![symbol_name, release];
        let _: () = msg_send![accessibility_label, release];
        item
    }

    extern "C" fn toolbar_item(
        this: &Object,
        _cmd: Sel,
        _toolbar: id,
        item_identifier: id,
        _will_insert: BOOL,
    ) -> id {
        unsafe {
            let target = this as *const Object as id;
            if identifier_equals(item_identifier, ADD_PROVIDER_ID) {
                return action_item(
                    item_identifier,
                    "plus",
                    "添加服务商",
                    target,
                    sel!(onAddProvider:),
                    &ADD_BUTTON,
                );
            }
            if identifier_equals(item_identifier, THEME_ID) {
                return action_item(
                    item_identifier,
                    "circle.lefthalf.filled",
                    "切换外观",
                    target,
                    sel!(onThemeToggle:),
                    &THEME_BUTTON,
                );
            }
            if !identifier_equals(item_identifier, NAVIGATION_ID) {
                return nil;
            }

            let titles: Vec<id> = ["服务商", "客户端", "路由", "监控", "日志", "关于"]
                .iter()
                .map(|&title| NSString::alloc(nil).init_str(title))
                .collect();
            let titles_array: id = NSArray::arrayWithObjects(nil, &titles);

            let segmented: id = msg_send![class!(NSSegmentedControl),
                segmentedControlWithLabels:titles_array
                trackingMode:0isize
                target:target
                action:sel!(onSegmentChanged:)
            ];
            let _: () = msg_send![segmented, setSegmentStyle:5isize];
            let _: () = msg_send![segmented, setControlSize:3usize];
            let _: () = msg_send![segmented, setSelectedSegment:0isize];
            let control_size = NSSize::new(360.0, 32.0);
            let _: () = msg_send![segmented, setFrameSize:control_size];

            let item: id = msg_send![class!(NSToolbarItem), alloc];
            let item: id = msg_send![item, initWithItemIdentifier:item_identifier];
            let _: () = msg_send![item, setView:segmented];
            let _: () = msg_send![item, setMinSize:control_size];
            let _: () = msg_send![item, setMaxSize:control_size];

            SEGMENT_CONTROL.store(segmented as *mut std::ffi::c_void, Ordering::SeqCst);
            item
        }
    }

    extern "C" fn toolbar_item_identifiers(_this: &Object, _cmd: Sel, _toolbar: id) -> id {
        unsafe {
            let identifiers = [
                NSString::alloc(nil).init_str(NAVIGATION_ID),
                NSToolbarFlexibleSpaceItemIdentifier,
                NSString::alloc(nil).init_str(ADD_PROVIDER_ID),
                NSString::alloc(nil).init_str(THEME_ID),
            ];
            NSArray::arrayWithObjects(nil, &identifiers)
        }
    }

    fn register_delegate_class() -> &'static Class {
        static mut CLS: Option<&'static Class> = None;
        INIT.call_once(|| {
            let superclass = class!(NSObject);
            let mut decl = ClassDecl::new("OmniSwitchTabDelegate", superclass).unwrap();
            unsafe {
                decl.add_method(
                    sel!(onSegmentChanged:),
                    on_segment_changed as extern "C" fn(&Object, Sel, id),
                );
                decl.add_method(
                    sel!(onAddProvider:),
                    on_add_provider as extern "C" fn(&Object, Sel, id),
                );
                decl.add_method(
                    sel!(onThemeToggle:),
                    on_theme_toggle as extern "C" fn(&Object, Sel, id),
                );
                decl.add_method(
                    sel!(toolbar:itemForItemIdentifier:willBeInsertedIntoToolbar:),
                    toolbar_item as extern "C" fn(&Object, Sel, id, id, BOOL) -> id,
                );
                decl.add_method(
                    sel!(toolbarAllowedItemIdentifiers:),
                    toolbar_item_identifiers as extern "C" fn(&Object, Sel, id) -> id,
                );
                decl.add_method(
                    sel!(toolbarDefaultItemIdentifiers:),
                    toolbar_item_identifiers as extern "C" fn(&Object, Sel, id) -> id,
                );
            }
            let cls = decl.register();
            unsafe {
                CLS = Some(cls);
            }
        });
        unsafe { CLS.unwrap() }
    }

    pub fn setup_native_tabs(window: &WebviewWindow) {
        let app_handle_box = Box::new(window.app_handle().clone());
        let raw_ptr = Box::into_raw(app_handle_box) as *mut std::ffi::c_void;
        APP_HANDLE.store(raw_ptr, Ordering::SeqCst);

        unsafe {
            let ns_window = match window.ns_window() {
                Ok(w) => w as id,
                Err(_) => return,
            };

            let delegate_cls = register_delegate_class();
            let delegate: id = msg_send![delegate_cls, alloc];
            let delegate: id = msg_send![delegate, init];

            let toolbar_identifier = NSString::alloc(nil).init_str("com.omniswitch.toolbar");
            let toolbar: id = msg_send![class!(NSToolbar), alloc];
            let toolbar: id = msg_send![toolbar, initWithIdentifier:toolbar_identifier];
            let _: () = msg_send![toolbar, setDelegate:delegate];
            let _: () = msg_send![toolbar, setAllowsUserCustomization:NO];
            let _: () = msg_send![toolbar, setAutosavesConfiguration:NO];
            let _: () = msg_send![toolbar, setDisplayMode:2usize];

            let item_identifier = NSString::alloc(nil).init_str(NAVIGATION_ID);
            let centered_identifiers: id = msg_send![class!(NSSet), setWithObject:item_identifier];
            let supports_centered_items: BOOL =
                msg_send![toolbar, respondsToSelector:sel!(setCenteredItemIdentifiers:)];
            if supports_centered_items == YES {
                let _: () = msg_send![toolbar, setCenteredItemIdentifiers:centered_identifiers];
            } else {
                let _: () = msg_send![toolbar, setCenteredItemIdentifier:item_identifier];
            }

            // Unified lets AppKit provide the native Liquid Glass material while leaving
            // enough vertical room for the large capsule control.
            let _: () = msg_send![ns_window, setToolbarStyle:3isize];
            let _: () = msg_send![ns_window, setToolbar:toolbar];
        }
    }

    pub fn set_selected_tab(tab: &str) {
        if let Some(pos) = TAB_IDS.iter().position(|&t| t == tab) {
            let ptr = SEGMENT_CONTROL.load(Ordering::SeqCst);
            if !ptr.is_null() {
                unsafe {
                    let segmented = ptr as id;
                    let _: () = msg_send![segmented, setSelectedSegment: pos as isize];
                }
            }
        }
    }

    pub fn set_toolbar_enabled(enabled: bool) {
        let enabled = if enabled { YES } else { NO };
        for ptr in [&SEGMENT_CONTROL, &ADD_BUTTON, &THEME_BUTTON] {
            let ptr = ptr.load(Ordering::SeqCst);
            if !ptr.is_null() {
                unsafe {
                    let control = ptr as id;
                    let _: () = msg_send![control, setEnabled:enabled];
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub mod imp {
    use tauri::WebviewWindow;
    pub fn setup_native_tabs(_window: &WebviewWindow) {}
    pub fn set_selected_tab(_tab: &str) {}
    pub fn set_toolbar_enabled(_enabled: bool) {}
}

pub use imp::*;
