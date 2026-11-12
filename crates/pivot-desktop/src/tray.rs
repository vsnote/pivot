#![allow(unexpected_cfgs, deprecated)]

use crate::app::ActiveTab;
use crate::i18n::Locale;
use crate::pricing::TodayUsageSummary;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayCommand {
    ShowWindow {
        tab: Option<ActiveTab>,
        open_add_provider: bool,
    },
    ToggleClient {
        client_id: String,
    },
    SelectClientModel {
        client_id: String,
        model: String,
    },
    ToggleProvider {
        provider_id: String,
        enabled: bool,
    },
    CopyText(String),
    QuitApp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayUpdateBanner {
    None,
    Available(String),
    ReadyToRestart(String),
}

#[derive(Debug, Clone)]
pub struct TrayModelOption {
    pub value: String,
    pub raw_id: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct TrayProviderModelGroup {
    pub provider_name: String,
    pub models: Vec<TrayModelOption>,
}

#[derive(Debug, Clone)]
pub struct TrayClientItem {
    pub id: String,
    pub name: String,
    pub is_intercepted: bool,
    pub is_overwritten: bool,
    pub active_model_value: String,
    pub active_model_display: String,
}

#[derive(Debug, Clone)]
pub struct TrayProviderItem {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    /// `(available, total, has_auth_error)` for subscription pool providers
    pub sub_status: Option<(usize, usize, bool)>,
}

#[derive(Debug, Clone)]
pub struct TrayMenuSnapshot {
    pub locale: Locale,
    pub is_running: bool,
    pub port: u16,
    pub today: TodayUsageSummary,
    pub clients: Vec<TrayClientItem>,
    pub model_groups: Vec<TrayProviderModelGroup>,
    pub providers: Vec<TrayProviderItem>,
    pub update_banner: TrayUpdateBanner,
}

impl TrayMenuSnapshot {
    pub fn default_with_port(port: u16) -> Self {
        Self {
            locale: Locale::default(),
            is_running: true,
            port,
            today: TodayUsageSummary::default(),
            clients: Vec::new(),
            model_groups: Vec::new(),
            providers: Vec::new(),
            update_banner: TrayUpdateBanner::None,
        }
    }
}

static SHOULD_QUIT: AtomicBool = AtomicBool::new(false);
static LAST_DOCK_THEME: AtomicU8 = AtomicU8::new(255);
static TRAY_TX: OnceLock<mpsc::UnboundedSender<TrayCommand>> = OnceLock::new();
static TRAY_SNAPSHOT: OnceLock<Mutex<TrayMenuSnapshot>> = OnceLock::new();

pub fn mark_quitting() {
    SHOULD_QUIT.store(true, Ordering::SeqCst);
}

pub fn should_quit() -> bool {
    SHOULD_QUIT.load(Ordering::SeqCst)
}

pub fn update_tray_snapshot(snapshot: TrayMenuSnapshot) {
    if let Some(lock) = TRAY_SNAPSHOT.get() {
        if let Ok(mut guard) = lock.lock() {
            *guard = snapshot;
        }
    } else {
        let _ = TRAY_SNAPSHOT.set(Mutex::new(snapshot));
    }
}

pub(crate) fn resolve_selected_model_index(
    active_val: &str,
    groups: &[TrayProviderModelGroup],
) -> Option<(usize, usize)> {
    let trimmed = active_val.trim();
    if trimmed.is_empty() {
        return None;
    }
    for (gi, grp) in groups.iter().enumerate() {
        for (mi, opt) in grp.models.iter().enumerate() {
            if opt.value == trimmed {
                return Some((gi, mi));
            }
        }
    }
    if !trimmed.contains('/') {
        for (gi, grp) in groups.iter().enumerate() {
            for (mi, opt) in grp.models.iter().enumerate() {
                if opt.raw_id == trimmed {
                    return Some((gi, mi));
                }
            }
        }
    }
    None
}

/// 唤起并置顶 macOS 主窗口（兼容已隐藏或已最小化至 Dock 的窗口）
pub fn reveal_main_window() {
    #[cfg(target_os = "macos")]
    macos::reveal_window();
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
    let _ = TRAY_SNAPSHOT.set(Mutex::new(TrayMenuSnapshot::default_with_port(port)));

    #[cfg(target_os = "macos")]
    macos::setup_status_item(port);

    #[cfg(not(target_os = "macos"))]
    let _ = port;

    rx
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{
        mark_quitting, resolve_selected_model_index, TrayCommand, TrayMenuSnapshot,
        TrayUpdateBanner, TRAY_SNAPSHOT, TRAY_TX,
    };
    use crate::i18n::Locale;
    use crate::pricing::{format_currency, format_tokens};
    use cocoa::appkit::{NSStatusBar, NSVariableStatusItemLength};
    use cocoa::base::{id, nil, BOOL, NO, YES};
    use cocoa::foundation::{NSAutoreleasePool, NSSize, NSString};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};
    use std::os::raw::c_void;
    use std::sync::{Mutex, Once, OnceLock};

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {
        static NSForegroundColorAttributeName: id;
        static NSFontAttributeName: id;
    }

    const STATE_MIXED: isize = -1;
    const STATE_OFF: isize = 0;
    const STATE_ON: isize = 1;
    const FLAT_MODEL_LIST_LIMIT: usize = 20;

    static REGISTER_TARGET_CLASS: Once = Once::new();
    static MENU_ACTIONS: OnceLock<Mutex<Vec<TrayCommand>>> = OnceLock::new();
    const TRAY_PNG: &[u8] = include_bytes!("../assets/tray-icon.png");

    fn action_table() -> &'static Mutex<Vec<TrayCommand>> {
        MENU_ACTIONS.get_or_init(|| Mutex::new(Vec::new()))
    }

    fn register_action(cmd: TrayCommand) -> isize {
        let mut guard = action_table().lock().unwrap_or_else(|e| e.into_inner());
        let idx = guard.len();
        guard.push(cmd);
        idx as isize
    }

    unsafe fn ns_string(s: &str) -> id {
        let raw = NSString::alloc(nil).init_str(s);
        msg_send![raw, autorelease]
    }

    unsafe fn make_attr_dict(color: id, font: id) -> id {
        let mut keys: [id; 2] = [NSForegroundColorAttributeName, nil];
        let mut vals: [id; 2] = [color, nil];
        let count = if font != nil {
            keys[1] = NSFontAttributeName;
            vals[1] = font;
            2usize
        } else {
            1usize
        };
        msg_send![
            class!(NSDictionary),
            dictionaryWithObjects: vals.as_ptr()
            forKeys: keys.as_ptr()
            count: count
        ]
    }

    unsafe fn make_status_attributed_title(dot_rgb: (f64, f64, f64), text: &str) -> id {
        let result: id = msg_send![class!(NSMutableAttributedString), alloc];
        let result: id = msg_send![result, init];
        let result: id = msg_send![result, autorelease];

        let font: id = msg_send![class!(NSFont), menuFontOfSize: 0.0f64];
        let dot_color: id = msg_send![
            class!(NSColor),
            colorWithSRGBRed: dot_rgb.0
            green: dot_rgb.1
            blue: dot_rgb.2
            alpha: 1.0f64
        ];
        let label_color: id = msg_send![class!(NSColor), labelColor];

        let dot_attrs = make_attr_dict(dot_color, font);
        let dot_str: id = msg_send![class!(NSAttributedString), alloc];
        let dot_str: id = msg_send![
            dot_str,
            initWithString: ns_string("●  ")
            attributes: dot_attrs
        ];
        let dot_str: id = msg_send![dot_str, autorelease];

        let text_attrs = make_attr_dict(label_color, font);
        let text_str: id = msg_send![class!(NSAttributedString), alloc];
        let text_str: id = msg_send![
            text_str,
            initWithString: ns_string(text)
            attributes: text_attrs
        ];
        let text_str: id = msg_send![text_str, autorelease];

        let _: () = msg_send![result, appendAttributedString: dot_str];
        let _: () = msg_send![result, appendAttributedString: text_str];
        result
    }

    unsafe fn new_submenu(title: &str) -> id {
        let m: id = msg_send![class!(NSMenu), alloc];
        let m: id = msg_send![m, initWithTitle: ns_string(title)];
        let m: id = msg_send![m, autorelease];
        let _: () = msg_send![m, setAutoenablesItems: NO];
        m
    }

    unsafe fn add_separator(menu: id) {
        let sep: id = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: sep];
    }

    unsafe fn add_disabled_item(menu: id, title: &str) -> id {
        let item: id = msg_send![class!(NSMenuItem), alloc];
        let item: id = msg_send![
            item,
            initWithTitle: ns_string(title)
            action: nil
            keyEquivalent: ns_string("")
        ];
        let item: id = msg_send![item, autorelease];
        let _: () = msg_send![item, setEnabled: NO];
        let _: () = msg_send![menu, addItem: item];
        item
    }

    unsafe fn add_section_header(menu: id, title: &str) {
        let supports_header: BOOL = msg_send![
            class!(NSMenuItem),
            respondsToSelector: sel!(sectionHeaderWithTitle:)
        ];
        if supports_header == YES {
            let header: id = msg_send![
                class!(NSMenuItem),
                sectionHeaderWithTitle: ns_string(title)
            ];
            let _: () = msg_send![menu, addItem: header];
        } else {
            add_disabled_item(menu, title);
        }
    }

    unsafe fn add_action_item(
        menu: id,
        target: id,
        title: &str,
        key_eq: &str,
        state: isize,
        cmd: TrayCommand,
    ) -> id {
        let tag = register_action(cmd);
        let item: id = msg_send![class!(NSMenuItem), alloc];
        let item: id = msg_send![
            item,
            initWithTitle: ns_string(title)
            action: sel!(onMenuAction:)
            keyEquivalent: ns_string(key_eq)
        ];
        let item: id = msg_send![item, autorelease];
        let _: () = msg_send![item, setTarget: target];
        let _: () = msg_send![item, setTag: tag];
        let _: () = msg_send![item, setState: state];
        let _: () = msg_send![item, setEnabled: YES];
        let _: () = msg_send![menu, addItem: item];
        item
    }

    unsafe fn add_submenu_parent_item(
        menu: id,
        title: &str,
        state: isize,
        submenu: id,
    ) -> id {
        let item: id = msg_send![class!(NSMenuItem), alloc];
        let item: id = msg_send![
            item,
            initWithTitle: ns_string(title)
            action: nil
            keyEquivalent: ns_string("")
        ];
        let item: id = msg_send![item, autorelease];
        let _: () = msg_send![item, setState: state];
        let _: () = msg_send![item, setEnabled: YES];
        let _: () = msg_send![item, setSubmenu: submenu];
        let _: () = msg_send![menu, addItem: item];
        item
    }

    unsafe fn is_gpui_main_window(win: id) -> bool {
        if win == nil {
            return false;
        }
        if let Some(gpui_cls) = Class::get("GPUIWindow") {
            let is_gpui: BOOL = msg_send![win, isKindOfClass: gpui_cls];
            if is_gpui == NO {
                return false;
            }
        } else {
            const NS_WINDOW_STYLE_MASK_TITLED: usize = 1 << 0;
            let style_mask: usize = msg_send![win, styleMask];
            let level: isize = msg_send![win, level];
            if (style_mask & NS_WINDOW_STYLE_MASK_TITLED) == 0 || level != 0 {
                return false;
            }
        }
        let can_main: BOOL = msg_send![win, canBecomeMainWindow];
        can_main == YES
    }

    unsafe fn unhide_and_focus_app_windows() {
        let ns_app: id = msg_send![class!(NSApplication), sharedApplication];
        if ns_app == nil {
            return;
        }
        let _: () = msg_send![ns_app, unhide: nil];
        let windows: id = msg_send![ns_app, windows];
        if windows != nil {
            let count: usize = msg_send![windows, count];
            for i in 0..count {
                let win: id = msg_send![windows, objectAtIndex: i];
                if is_gpui_main_window(win) {
                    let is_mini: BOOL = msg_send![win, isMiniaturized];
                    if is_mini == YES {
                        let _: () = msg_send![win, deminiaturize: nil];
                    }
                    let is_visible: BOOL = msg_send![win, isVisible];
                    if is_visible == NO {
                        let view: id = msg_send![win, contentView];
                        if view != nil {
                            let _: () = msg_send![view, setNeedsDisplay: YES];
                        }
                    }
                    let _: () = msg_send![win, makeKeyAndOrderFront: nil];
                }
            }
        }
        let _: () = msg_send![ns_app, activateIgnoringOtherApps: YES];
    }

    pub fn reveal_window() {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            unhide_and_focus_app_windows();
            let _: () = msg_send![pool, drain];
        }
    }

    pub fn set_app_dock_icon(is_dark: bool) {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let bytes = crate::components::icons::app_icon_bytes(is_dark);
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
            let _: () = msg_send![pool, drain];
        }
    }

    unsafe fn rebuild_menu_from_snapshot(menu: id, target: id, snap: &TrayMenuSnapshot) {
        let _: () = msg_send![menu, removeAllItems];
        if let Ok(mut guard) = action_table().lock() {
            guard.clear();
        }

        let locale = snap.locale;
        let enabled_providers = snap.providers.iter().filter(|p| p.enabled).count();
        let has_sub_auth_error = snap.providers.iter().any(|p| {
            p.enabled
                && p.sub_status
                    .map(|(_, _, auth_err)| auth_err)
                    .unwrap_or(false)
        });

        // 1. 顶部网关状态与健康告警
        let (dot_rgb, status_text, warning_line) = if !snap.is_running {
            (
                (0.957, 0.247, 0.369), // Rose #F43F5E
                format!(
                    "{} · 127.0.0.1:{}",
                    locale.pick("网关未监听", "Gateway Stopped"),
                    snap.port
                ),
                None,
            )
        } else if enabled_providers == 0 {
            (
                (0.961, 0.620, 0.043), // Amber #F59E0B
                format!(
                    "{} · 127.0.0.1:{}",
                    locale.pick("网关运行中", "Gateway Running"),
                    snap.port
                ),
                Some(locale.pick("当前无已启用的服务商", "No active providers enabled")),
            )
        } else if has_sub_auth_error {
            (
                (0.961, 0.620, 0.043), // Amber #F59E0B
                format!(
                    "{} · 127.0.0.1:{}",
                    locale.pick("网关运行中", "Gateway Running"),
                    snap.port
                ),
                Some(locale.pick(
                    "存在授权失效的订阅账号",
                    "Subscription account auth expired",
                )),
            )
        } else {
            (
                (0.063, 0.725, 0.506), // Emerald #10B981
                format!(
                    "{} · 127.0.0.1:{}",
                    locale.pick("网关运行中", "Gateway Running"),
                    snap.port
                ),
                None,
            )
        };

        let status_item = add_disabled_item(menu, &status_text);
        let attr_title = make_status_attributed_title(dot_rgb, &status_text);
        let _: () = msg_send![status_item, setAttributedTitle: attr_title];

        if let Some(warn) = warning_line {
            add_disabled_item(menu, warn);
        }

        // 今日用量摘要
        let today_line = if snap.today.requests == 0 {
            locale
                .pick("今日暂无调用", "Today: No requests")
                .to_string()
        } else {
            let tok_str = format_tokens(snap.today.tokens);
            let cost_str = format_currency(snap.today.cost_usd);
            match locale {
                Locale::ZhCn => format!(
                    "今日 {} 次调用 · {} Tokens · {}",
                    snap.today.requests, tok_str, cost_str
                ),
                Locale::En => format!(
                    "Today: {} reqs · {} Tokens · {}",
                    snap.today.requests, tok_str, cost_str
                ),
            }
        };
        add_disabled_item(menu, &today_line);

        add_separator(menu);

        // 2. 已安装客户端接管开关与按服务商分组模型选择
        let total_models: usize = snap.model_groups.iter().map(|g| g.models.len()).sum();

        if snap.clients.is_empty() {
            add_disabled_item(
                menu,
                locale.pick("未检测到已安装的客户端", "No supported agents installed"),
            );
        } else {
            for client in &snap.clients {
                let (parent_state, parent_title) = if client.is_overwritten {
                    (
                        STATE_MIXED,
                        format!(
                            "{} · {}",
                            client.name,
                            locale.pick("配置冲突", "Conflict")
                        ),
                    )
                } else if client.is_intercepted {
                    let title = if client.active_model_display.is_empty() {
                        client.name.clone()
                    } else {
                        format!("{} · {}", client.name, client.active_model_display)
                    };
                    (STATE_ON, title)
                } else {
                    (STATE_OFF, client.name.clone())
                };

                let client_sub = new_submenu(&client.name);

                // 子菜单首项：接管开关或解决冲突
                if client.is_overwritten {
                    add_action_item(
                        client_sub,
                        target,
                        locale.pick("解决配置冲突…", "Resolve Config Conflict…"),
                        "",
                        STATE_OFF,
                        TrayCommand::ToggleClient {
                            client_id: client.id.clone(),
                        },
                    );
                } else {
                    add_action_item(
                        client_sub,
                        target,
                        locale.pick("开启网关接管", "Intercept via Pivot"),
                        "",
                        if client.is_intercepted {
                            STATE_ON
                        } else {
                            STATE_OFF
                        },
                        TrayCommand::ToggleClient {
                            client_id: client.id.clone(),
                        },
                    );
                }

                add_separator(client_sub);

                // 子菜单模型池（严格按已启用服务商分组）
                if total_models == 0 {
                    add_disabled_item(
                        client_sub,
                        locale.pick("无可用模型", "No models available"),
                    );
                } else {
                    let selected_idx = resolve_selected_model_index(
                        &client.active_model_value,
                        &snap.model_groups,
                    );

                    if total_models <= FLAT_MODEL_LIST_LIMIT {
                        for (gi, grp) in snap.model_groups.iter().enumerate() {
                            if gi > 0 {
                                add_separator(client_sub);
                            }
                            add_section_header(client_sub, &grp.provider_name);
                            for (mi, opt) in grp.models.iter().enumerate() {
                                let is_sel = selected_idx == Some((gi, mi));
                                add_action_item(
                                    client_sub,
                                    target,
                                    &opt.label,
                                    "",
                                    if is_sel { STATE_ON } else { STATE_OFF },
                                    TrayCommand::SelectClientModel {
                                        client_id: client.id.clone(),
                                        model: opt.value.clone(),
                                    },
                                );
                            }
                        }
                    } else {
                        add_section_header(
                            client_sub,
                            locale.pick("选择启动模型", "Startup Model"),
                        );
                        for (gi, grp) in snap.model_groups.iter().enumerate() {
                            let group_has_sel =
                                selected_idx.map(|(sel_g, _)| sel_g == gi).unwrap_or(false);
                            let prov_model_sub = new_submenu(&grp.provider_name);
                            for (mi, opt) in grp.models.iter().enumerate() {
                                let is_sel = selected_idx == Some((gi, mi));
                                add_action_item(
                                    prov_model_sub,
                                    target,
                                    &opt.label,
                                    "",
                                    if is_sel { STATE_ON } else { STATE_OFF },
                                    TrayCommand::SelectClientModel {
                                        client_id: client.id.clone(),
                                        model: opt.value.clone(),
                                    },
                                );
                            }
                            add_submenu_parent_item(
                                client_sub,
                                &grp.provider_name,
                                if group_has_sel { STATE_ON } else { STATE_OFF },
                                prov_model_sub,
                            );
                        }
                    }
                }

                add_submenu_parent_item(menu, &parent_title, parent_state, client_sub);
            }
        }

        add_separator(menu);

        // 3. 服务商渠道快速启停与复制网关地址
        let providers_title = if snap.providers.is_empty() {
            locale.pick("服务商", "Providers").to_string()
        } else {
            format!(
                "{} ({}/{})",
                locale.pick("服务商", "Providers"),
                enabled_providers,
                snap.providers.len()
            )
        };
        let providers_sub = new_submenu(&providers_title);
        if snap.providers.is_empty() {
            add_action_item(
                providers_sub,
                target,
                locale.pick("添加服务商…", "Add Provider…"),
                "",
                STATE_OFF,
                TrayCommand::ShowWindow {
                    tab: Some(crate::app::ActiveTab::Providers),
                    open_add_provider: true,
                },
            );
        } else {
            for p in &snap.providers {
                let row_title = match p.sub_status {
                    Some((_, _, true)) => {
                        format!("{} ({})", p.name, locale.pick("授权失效", "Auth Expired"))
                    }
                    Some((avail, total, false)) if total > 0 => {
                        format!("{} ({}/{})", p.name, avail, total)
                    }
                    _ => p.name.clone(),
                };
                add_action_item(
                    providers_sub,
                    target,
                    &row_title,
                    "",
                    if p.enabled { STATE_ON } else { STATE_OFF },
                    TrayCommand::ToggleProvider {
                        provider_id: p.id.clone(),
                        enabled: !p.enabled,
                    },
                );
            }
        }
        add_submenu_parent_item(menu, &providers_title, STATE_OFF, providers_sub);

        let copy_title = locale.pick("复制网关地址", "Copy Base URL");
        let copy_sub = new_submenu(copy_title);
        let openai_url = format!("http://127.0.0.1:{}/v1", snap.port);
        let anthropic_url = format!("http://127.0.0.1:{}", snap.port);
        add_action_item(
            copy_sub,
            target,
            &format!(
                "{} · {}",
                locale.pick("OpenAI 兼容", "OpenAI Compatible"),
                openai_url
            ),
            "",
            STATE_OFF,
            TrayCommand::CopyText(openai_url),
        );
        add_action_item(
            copy_sub,
            target,
            &format!(
                "{} · {}",
                locale.pick("Anthropic 兼容", "Anthropic Compatible"),
                anthropic_url
            ),
            "",
            STATE_OFF,
            TrayCommand::CopyText(anthropic_url),
        );
        add_submenu_parent_item(menu, copy_title, STATE_OFF, copy_sub);

        add_separator(menu);

        // 4. 新版本提醒、打开主窗口与设置
        match &snap.update_banner {
            TrayUpdateBanner::Available(ver) => {
                let label = format!(
                    "✨ {} v{}…",
                    locale.pick("发现新版本", "Update Available"),
                    ver
                );
                add_action_item(
                    menu,
                    target,
                    &label,
                    "",
                    STATE_OFF,
                    TrayCommand::ShowWindow {
                        tab: Some(crate::app::ActiveTab::Settings),
                        open_add_provider: false,
                    },
                );
            }
            TrayUpdateBanner::ReadyToRestart(ver) => {
                let label = format!(
                    "✨ {} (v{})…",
                    locale.pick("重启以完成更新", "Restart to Apply Update"),
                    ver
                );
                add_action_item(
                    menu,
                    target,
                    &label,
                    "",
                    STATE_OFF,
                    TrayCommand::ShowWindow {
                        tab: Some(crate::app::ActiveTab::Settings),
                        open_add_provider: false,
                    },
                );
            }
            TrayUpdateBanner::None => {}
        }

        add_action_item(
            menu,
            target,
            locale.pick("打开 Pivot", "Open Pivot"),
            "",
            STATE_OFF,
            TrayCommand::ShowWindow {
                tab: None,
                open_add_provider: false,
            },
        );
        add_action_item(
            menu,
            target,
            locale.pick("设置…", "Settings…"),
            ",",
            STATE_OFF,
            TrayCommand::ShowWindow {
                tab: Some(crate::app::ActiveTab::Settings),
                open_add_provider: false,
            },
        );

        add_separator(menu);

        // 5. 退出应用
        add_action_item(
            menu,
            target,
            locale.pick("退出 Pivot", "Quit Pivot"),
            "q",
            STATE_OFF,
            TrayCommand::QuitApp,
        );
    }

    extern "C" fn handle_menu_needs_update(this: &Object, _cmd: Sel, menu: id) {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let target = this as *const Object as id;
            let snapshot = TRAY_SNAPSHOT
                .get()
                .and_then(|lock| lock.lock().ok().map(|g| g.clone()));
            if let Some(snap) = snapshot {
                rebuild_menu_from_snapshot(menu, target, &snap);
            }
            let _: () = msg_send![pool, drain];
        }
    }

    extern "C" fn handle_menu_action(_this: &Object, _cmd: Sel, sender: id) {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            if sender != nil {
                let tag: isize = msg_send![sender, tag];
                let cmd_opt = if tag >= 0 {
                    action_table()
                        .lock()
                        .ok()
                        .and_then(|guard| guard.get(tag as usize).cloned())
                } else {
                    None
                };

                if let Some(cmd) = cmd_opt {
                    if matches!(cmd, TrayCommand::QuitApp) {
                        mark_quitting();
                    }
                    if let Some(tx) = TRAY_TX.get() {
                        let _ = tx.send(cmd);
                    }
                }
            }
            let _: () = msg_send![pool, drain];
        }
    }

    fn ensure_target_class() -> *const Class {
        REGISTER_TARGET_CLASS.call_once(|| unsafe {
            if Class::get("PivotGpuiTrayTarget").is_none() {
                if let Some(mut decl) = ClassDecl::new("PivotGpuiTrayTarget", class!(NSObject)) {
                    decl.add_method(
                        sel!(menuNeedsUpdate:),
                        handle_menu_needs_update as extern "C" fn(&Object, Sel, id),
                    );
                    decl.add_method(
                        sel!(onMenuAction:),
                        handle_menu_action as extern "C" fn(&Object, Sel, id),
                    );
                    decl.register();
                }
            }
        });
        Class::get("PivotGpuiTrayTarget").unwrap()
    }

    pub fn setup_status_item(port: u16) {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let target_cls = ensure_target_class();
            let target: id = msg_send![target_cls, new];
            let _: id = msg_send![target, retain];

            let status_bar = NSStatusBar::systemStatusBar(nil);
            let status_item: id = status_bar.statusItemWithLength_(NSVariableStatusItemLength);
            if status_item == nil {
                let _: () = msg_send![pool, drain];
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
                    let _: () = msg_send![ns_image, release];
                }
            }

            // 构建状态栏动态菜单并挂载 NSMenuDelegate
            let menu: id = msg_send![class!(NSMenu), new];
            let _: () = msg_send![menu, setAutoenablesItems: NO];
            let _: () = msg_send![menu, setDelegate: target];

            let initial = TRAY_SNAPSHOT
                .get()
                .and_then(|lock| lock.lock().ok().map(|g| g.clone()))
                .unwrap_or_else(|| TrayMenuSnapshot::default_with_port(port));
            rebuild_menu_from_snapshot(menu, target, &initial);

            let _: () = msg_send![status_item, setMenu: menu];
            let _: () = msg_send![menu, release];
            let _: () = msg_send![pool, drain];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_groups() -> Vec<TrayProviderModelGroup> {
        vec![
            TrayProviderModelGroup {
                provider_name: "Antigravity".to_string(),
                models: vec![
                    TrayModelOption {
                        value: "antigravity/claude-sonnet-4-6".to_string(),
                        raw_id: "claude-sonnet-4-6".to_string(),
                        label: "claude-sonnet-4-6".to_string(),
                    },
                    TrayModelOption {
                        value: "gemini-2.5-pro".to_string(),
                        raw_id: "gemini-2.5-pro".to_string(),
                        label: "gemini-2.5-pro".to_string(),
                    },
                ],
            },
            TrayProviderModelGroup {
                provider_name: "Anthropic".to_string(),
                models: vec![TrayModelOption {
                    value: "anthropic/claude-sonnet-4-6".to_string(),
                    raw_id: "claude-sonnet-4-6".to_string(),
                    label: "claude-sonnet-4-6".to_string(),
                }],
            },
        ]
    }

    #[test]
    fn resolves_exact_prefixed_model() {
        let groups = sample_groups();
        assert_eq!(
            resolve_selected_model_index("anthropic/claude-sonnet-4-6", &groups),
            Some((1, 0))
        );
    }

    #[test]
    fn resolves_unprefixed_duplicate_to_highest_priority_provider() {
        let groups = sample_groups();
        assert_eq!(
            resolve_selected_model_index("claude-sonnet-4-6", &groups),
            Some((0, 0))
        );
    }

    #[test]
    fn returns_none_for_missing_or_empty_model() {
        let groups = sample_groups();
        assert_eq!(resolve_selected_model_index("", &groups), None);
        assert_eq!(resolve_selected_model_index("unknown-model", &groups), None);
    }
}
