use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::i18n::Locale;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiPreferences {
    #[serde(default)]
    pub theme: ThemePreference,
    #[serde(default)]
    pub locale: Locale,
}

fn pivot_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pivot")
}

pub fn load_ui_preferences() -> UiPreferences {
    let path = pivot_config_dir().join("ui-prefs.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<UiPreferences>(&raw).ok())
        .unwrap_or_default()
}

pub fn save_ui_preferences(prefs: &UiPreferences) {
    let dir = pivot_config_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string_pretty(prefs) {
        let _ = std::fs::write(dir.join("ui-prefs.json"), json);
    }
}

/// 持久化候选模型池 (`~/.pivot/provider-pools.json`)，对应 TS 版 `pivot_provider_pool_{id}`
pub fn load_provider_pools() -> HashMap<String, Vec<String>> {
    let path = pivot_config_dir().join("provider-pools.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<HashMap<String, Vec<String>>>(&raw).ok())
        .unwrap_or_default()
}

pub fn save_provider_pool(provider_id: &str, pool: &[String]) {
    let mut all = load_provider_pools();
    all.insert(provider_id.to_string(), pool.to_vec());
    let dir = pivot_config_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string_pretty(&all) {
        let _ = std::fs::write(dir.join("provider-pools.json"), json);
    }
}

pub fn remove_provider_pool(provider_id: &str) {
    let mut all = load_provider_pools();
    if all.remove(provider_id).is_some() {
        let dir = pivot_config_dir();
        if let Ok(json) = serde_json::to_string_pretty(&all) {
            let _ = std::fs::write(dir.join("provider-pools.json"), json);
        }
    }
}

pub fn is_dark_appearance(pref: ThemePreference, window: &Window) -> bool {
    match pref {
        ThemePreference::Dark => true,
        ThemePreference::Light => false,
        ThemePreference::System => matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

pub fn sync_gpui_component_theme(is_dark: bool, window: Option<&mut Window>, cx: &mut App) {
    let target = if is_dark {
        gpui_kit::component::ThemeMode::Dark
    } else {
        gpui_kit::component::ThemeMode::Light
    };
    if cx.global::<gpui_kit::component::Theme>().mode != target {
        gpui_kit::component::Theme::change(target, window, cx);
    }
}

/// 1:1 映射 `src/App.css` 的 OKLCH 语义色板
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub is_dark: bool,
    pub background: Rgba,
    pub header_bg: Rgba,
    pub foreground: Rgba,
    pub card: Rgba,
    pub card_hover: Rgba,
    pub popover: Rgba,
    pub primary: Rgba,
    pub primary_hover: Rgba,
    pub primary_foreground: Rgba,
    pub secondary: Rgba,
    pub secondary_hover: Rgba,
    pub secondary_foreground: Rgba,
    pub muted: Rgba,
    pub muted_subtle: Rgba,
    pub muted_foreground: Rgba,
    pub subtext: Rgba,
    pub accent: Rgba,
    pub accent_foreground: Rgba,
    pub border: Rgba,
    pub border_subtle: Rgba,
    pub input: Rgba,
    pub ring_border: Rgba,
    pub search_bg: Rgba,
    pub destructive: Rgba,
    pub destructive_hover: Rgba,
    pub destructive_foreground: Rgba,
    pub destructive_soft_bg: Rgba,
    pub destructive_soft_border: Rgba,
    pub destructive_text: Rgba,

    // 专属控件语义色
    pub icon_slot_bg: Rgba,
    pub icon_slot_border: Rgba,
    pub switch_unchecked_bg: Rgba,
    pub switch_unchecked_hover: Rgba,
    pub switch_thumb_checked: Rgba,
    pub switch_thumb_unchecked: Rgba,
    pub segmented_bg: Rgba,
    pub segmented_active_bg: Rgba,
    pub segmented_active_border: Rgba,
    pub overlay_backdrop: Rgba,

    // 状态徽章色
    pub sub_badge_bg: Rgba,
    pub sub_badge_border: Rgba,
    pub sub_badge_text: Rgba,
    pub emerald_text: Rgba,
    pub emerald_soft_bg: Rgba,
    pub emerald_soft_border: Rgba,
    pub amber_text: Rgba,
    pub amber_soft_bg: Rgba,
    pub amber_soft_border: Rgba,
    pub rose_text: Rgba,
    pub rose_soft_bg: Rgba,
    pub rose_soft_border: Rgba,
    pub blue_text: Rgba,
    pub blue_soft_bg: Rgba,
    pub blue_soft_border: Rgba,
}

impl ThemePalette {
    pub fn resolve(pref: ThemePreference, window: &Window) -> Self {
        if is_dark_appearance(pref, window) {
            Self::dark()
        } else {
            Self::light()
        }
    }

    pub fn dark() -> Self {
        Self {
            is_dark: true,
            background: rgb(0x0b0b0d),
            header_bg: rgba(0x0b0b0de8),
            foreground: rgb(0xfafafa),
            card: rgb(0x121215),
            card_hover: rgba(0xffffff08),
            popover: rgb(0x16161a),
            primary: rgb(0xe7e7ea),
            primary_hover: rgb(0xd4d4d8),
            primary_foreground: rgb(0x09090b),
            secondary: rgb(0x1c1c20),
            secondary_hover: rgb(0x25252a),
            secondary_foreground: rgb(0xf4f4f5),
            muted: rgb(0x1c1c20),
            muted_subtle: rgba(0x1c1c2080),
            muted_foreground: rgb(0x8e8e95),
            subtext: rgb(0x71717a),
            accent: rgb(0x1f1f24),
            accent_foreground: rgb(0xfafafa),
            border: rgb(0x242429),
            border_subtle: rgba(0x24242999),
            input: rgb(0x27272c),
            ring_border: rgb(0x52525b),
            search_bg: rgba(0xffffff0d),
            destructive: rgb(0xdc2626),
            destructive_hover: rgb(0xb91c1c),
            destructive_foreground: rgb(0xfafafa),
            destructive_soft_bg: rgba(0xf43f5e1a),
            destructive_soft_border: rgba(0xf43f5e38),
            destructive_text: rgb(0xfb7185),

            icon_slot_bg: rgba(0xffffff12),
            icon_slot_border: rgba(0xffffff12),
            switch_unchecked_bg: rgba(0x3f3f46cc),
            switch_unchecked_hover: rgba(0x52525bcc),
            switch_thumb_checked: rgb(0x09090b),
            switch_thumb_unchecked: rgb(0xffffff),
            segmented_bg: rgba(0xffffff14),
            segmented_active_bg: rgb(0x3f3f46),
            segmented_active_border: rgba(0xffffff1a),
            overlay_backdrop: rgba(0x00000088),

            sub_badge_bg: rgba(0x6366f11f),
            sub_badge_border: rgba(0x6366f140),
            sub_badge_text: rgb(0xa5b4fc),
            emerald_text: rgb(0x34d399),
            emerald_soft_bg: rgba(0x10b9811a),
            emerald_soft_border: rgba(0x10b98138),
            amber_text: rgb(0xfbbf24),
            amber_soft_bg: rgba(0xf59e0b1a),
            amber_soft_border: rgba(0xf59e0b38),
            rose_text: rgb(0xfb7185),
            rose_soft_bg: rgba(0xf43f5e1a),
            rose_soft_border: rgba(0xf43f5e38),
            blue_text: rgb(0x60a5fa),
            blue_soft_bg: rgba(0x3b82f61a),
            blue_soft_border: rgba(0x3b82f638),
        }
    }

    pub fn light() -> Self {
        Self {
            is_dark: false,
            background: rgb(0xf9f9fb),
            header_bg: rgba(0xf9f9fbe8),
            foreground: rgb(0x0a0a0c),
            card: rgb(0xffffff),
            card_hover: rgba(0x00000006),
            popover: rgb(0xffffff),
            primary: rgb(0x18181b),
            primary_hover: rgb(0x27272a),
            primary_foreground: rgb(0xfafafa),
            secondary: rgb(0xf2f2f5),
            secondary_hover: rgb(0xe4e4e7),
            secondary_foreground: rgb(0x18181b),
            muted: rgb(0xf2f2f5),
            muted_subtle: rgba(0xf2f2f599),
            muted_foreground: rgb(0x71717a),
            subtext: rgb(0x8e8e95),
            accent: rgb(0xefeff3),
            accent_foreground: rgb(0x18181b),
            border: rgb(0xe4e4e7),
            border_subtle: rgba(0xe4e4e7b3),
            input: rgb(0xe4e4e7),
            ring_border: rgb(0xa1a1aa),
            search_bg: rgba(0x71717a12),
            destructive: rgb(0xdc2626),
            destructive_hover: rgb(0xb91c1c),
            destructive_foreground: rgb(0xffffff),
            destructive_soft_bg: rgba(0xf43f5e14),
            destructive_soft_border: rgba(0xf43f5e33),
            destructive_text: rgb(0xe11d48),

            icon_slot_bg: rgba(0x71717a12),
            icon_slot_border: rgba(0x0000000c),
            switch_unchecked_bg: rgba(0xe4e4e7e6),
            switch_unchecked_hover: rgba(0xd4d4d8e6),
            switch_thumb_checked: rgb(0xffffff),
            switch_thumb_unchecked: rgb(0xffffff),
            segmented_bg: rgba(0x71717a14),
            segmented_active_bg: rgb(0xffffff),
            segmented_active_border: rgba(0x00000010),
            overlay_backdrop: rgba(0x00000066),

            sub_badge_bg: rgba(0x6366f114),
            sub_badge_border: rgba(0x6366f133),
            sub_badge_text: rgb(0x4f46e5),
            emerald_text: rgb(0x059669),
            emerald_soft_bg: rgba(0x10b98114),
            emerald_soft_border: rgba(0x10b98133),
            amber_text: rgb(0xd97706),
            amber_soft_bg: rgba(0xf59e0b14),
            amber_soft_border: rgba(0xf59e0b33),
            rose_text: rgb(0xe11d48),
            rose_soft_bg: rgba(0xf43f5e14),
            rose_soft_border: rgba(0xf43f5e33),
            blue_text: rgb(0x2563eb),
            blue_soft_bg: rgba(0x3b82f614),
            blue_soft_border: rgba(0x3b82f633),
        }
    }

    /// 浮层（下拉菜单、右键菜单）双层柔和立体投影，解决浅色白底浮层与白底卡片融化问题
    pub fn popover_shadows(&self) -> Vec<BoxShadow> {
        if self.is_dark {
            vec![
                BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.56),
                    offset: point(px(0.0), px(10.0)),
                    blur_radius: px(28.0),
                    spread_radius: px(-2.0),
                    inset: false,
                },
                BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.36),
                    offset: point(px(0.0), px(2.0)),
                    blur_radius: px(8.0),
                    spread_radius: px(0.0),
                    inset: false,
                },
            ]
        } else {
            vec![
                BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.12),
                    offset: point(px(0.0), px(8.0)),
                    blur_radius: px(24.0),
                    spread_radius: px(-2.0),
                    inset: false,
                },
                BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.06),
                    offset: point(px(0.0), px(2.0)),
                    blur_radius: px(6.0),
                    spread_radius: px(-1.0),
                    inset: false,
                },
            ]
        }
    }

    /// 模态对话框（确认框、冲突弹窗）高阶投影
    pub fn modal_shadows(&self) -> Vec<BoxShadow> {
        if self.is_dark {
            vec![BoxShadow {
                color: hsla(0.0, 0.0, 0.0, 0.65),
                offset: point(px(0.0), px(16.0)),
                blur_radius: px(40.0),
                spread_radius: px(-4.0),
                inset: false,
            }]
        } else {
            vec![
                BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.16),
                    offset: point(px(0.0), px(14.0)),
                    blur_radius: px(36.0),
                    spread_radius: px(-4.0),
                    inset: false,
                },
                BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.06),
                    offset: point(px(0.0), px(4.0)),
                    blur_radius: px(10.0),
                    spread_radius: px(-2.0),
                    inset: false,
                },
            ]
        }
    }
}
