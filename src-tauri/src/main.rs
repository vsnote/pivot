// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(deprecated)]

fn main() {
    #[cfg(target_os = "macos")]
    unsafe {
        use cocoa::base::nil;
        use cocoa::foundation::NSString;
        use objc::{class, msg_send, sel, sel_impl};
        let process_info: cocoa::base::id = msg_send![class!(NSProcessInfo), processInfo];
        let name = NSString::alloc(nil).init_str("AnyRelay");
        let _: () = msg_send![process_info, setProcessName: name];
        let _: () = msg_send![name, release];
    }

    anyrelay_lib::run()
}
