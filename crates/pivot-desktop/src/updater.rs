use crate::i18n::Locale;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

pub const RELEASE_LATEST_PAGE_URL: &str = "https://github.com/vsnote/pivot/releases/latest";
const LATEST_MANIFEST_URL: &str =
    "https://github.com/vsnote/pivot/releases/latest/download/latest.json";

#[derive(Debug, Clone)]
pub struct UpdateReleaseInfo {
    pub version: String,
    pub notes: Option<String>,
    pub asset_url: Option<String>,
    pub release_url: String,
    pub app_bundle: Option<PathBuf>,
}

impl UpdateReleaseInfo {
    pub fn can_self_update(&self) -> bool {
        self.asset_url.is_some() && self.app_bundle.is_some()
    }
}

#[derive(Debug, Clone)]
pub enum UpdateCheckState {
    Idle,
    Checking,
    UpToDate,
    Available(UpdateReleaseInfo),
    Downloading {
        info: UpdateReleaseInfo,
        progress: u8,
    },
    Installing {
        info: UpdateReleaseInfo,
    },
    ReadyToRestart {
        version: String,
        app_bundle: PathBuf,
    },
    Failed {
        error: String,
        retry_info: Option<UpdateReleaseInfo>,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum InstallProgressEvent {
    Downloading(u8),
    Installing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedManifest {
    pub version: String,
    pub notes: Option<String>,
    pub asset_url: Option<String>,
}

pub fn current_platform_key() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "darwin-aarch64"
    } else {
        "darwin-x86_64"
    }
}

pub fn is_newer_semver(remote: &str, local: &str) -> bool {
    fn parse_semver(s: &str) -> Option<((u64, u64, u64), Option<String>)> {
        let cleaned = s
            .trim()
            .trim_start_matches(['v', 'V'])
            .split('+')
            .next()
            .unwrap_or("");
        if cleaned.is_empty() {
            return None;
        }
        let (core_str, pre_str) = match cleaned.split_once('-') {
            Some((core, pre)) if !pre.is_empty() => (core, Some(pre.to_string())),
            Some((core, _)) => (core, None),
            None => (cleaned, None),
        };
        let mut nums = [0u64; 3];
        let parts: Vec<&str> = core_str.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return None;
        }
        for (idx, part) in parts.iter().enumerate() {
            if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            nums[idx] = part.parse::<u64>().ok()?;
        }
        Some(((nums[0], nums[1], nums[2]), pre_str))
    }

    fn cmp_prerelease_segment(a: &str, b: &str) -> std::cmp::Ordering {
        let a_is_num = !a.is_empty() && a.chars().all(|c| c.is_ascii_digit());
        let b_is_num = !b.is_empty() && b.chars().all(|c| c.is_ascii_digit());
        match (a_is_num, b_is_num) {
            (true, true) => {
                let an = a.parse::<u64>().unwrap_or(0);
                let bn = b.parse::<u64>().unwrap_or(0);
                an.cmp(&bn)
            }
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (false, false) => a.cmp(b),
        }
    }

    let Some((r_core, r_pre)) = parse_semver(remote) else {
        return false;
    };
    let Some((l_core, l_pre)) = parse_semver(local) else {
        return false;
    };

    if r_core != l_core {
        return r_core > l_core;
    }

    match (r_pre, l_pre) {
        (None, Some(_)) => true,
        (Some(_), None) => false,
        (None, None) => false,
        (Some(rp), Some(lp)) => {
            let r_parts: Vec<&str> = rp.split('.').collect();
            let l_parts: Vec<&str> = lp.split('.').collect();
            for (ra, la) in r_parts.iter().zip(l_parts.iter()) {
                let ord = cmp_prerelease_segment(ra, la);
                if ord != std::cmp::Ordering::Equal {
                    return ord == std::cmp::Ordering::Greater;
                }
            }
            r_parts.len() > l_parts.len()
        }
    }
}

pub fn parse_manifest_for_platform(
    json: &serde_json::Value,
    platform_key: &str,
) -> ParsedManifest {
    let version = json
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .trim_start_matches('v')
        .to_string();

    let notes = json
        .get("notes")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let asset_url = json
        .get("platforms")
        .and_then(|p| p.get(platform_key))
        .and_then(|entry| entry.get("url"))
        .and_then(|u| u.as_str())
        .map(str::trim)
        .filter(|u| u.starts_with("https://"))
        .map(|u| u.to_string());

    ParsedManifest {
        version,
        notes,
        asset_url,
    }
}

/// 从 `latest.json` 的 `notes` 文本中提取并清洗单条更新说明。
pub fn extract_release_note_items(notes: Option<&str>) -> Vec<String> {
    let Some(raw) = notes else {
        return Vec::new();
    };

    raw.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed == "---"
                || trimmed.starts_with('#')
                || (trimmed.starts_with("Pivot ") && trimmed.contains("原生 Rust + GPUI 桌面版"))
            {
                return None;
            }

            let mut text = trimmed
                .strip_prefix(['•', '·'])
                .or_else(|| trimmed.strip_prefix("- "))
                .or_else(|| trimmed.strip_prefix("* "))
                .unwrap_or(trimmed)
                .trim();

            if let Some(idx) = text.rfind(" ([") {
                let tail = &text[idx..];
                if tail.contains("](http") && tail.ends_with("))") {
                    text = text[..idx].trim();
                }
            }

            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        })
        .collect()
}

/// 根据可执行文件路径定位可原地更新的 macOS `.app` Bundle 路径。
/// 排除 `/Volumes/`（只读 DMG 镜像直接运行）与 `/AppTranslocation/`（Gatekeeper 随机只读转位目录）。
pub fn resolve_updatable_app_bundle_from_exe(exe_path: &Path) -> Option<PathBuf> {
    let macos_dir = exe_path.parent()?;
    if macos_dir.file_name().and_then(|n| n.to_str()) != Some("MacOS") {
        return None;
    }
    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name().and_then(|n| n.to_str()) != Some("Contents") {
        return None;
    }
    let app_bundle = contents_dir.parent()?;
    if app_bundle.extension().and_then(|e| e.to_str()) != Some("app") {
        return None;
    }

    let bundle_str = app_bundle.to_string_lossy();
    if bundle_str.starts_with("/Volumes/") || bundle_str.contains("/AppTranslocation/") {
        return None;
    }

    app_bundle.parent()?;
    Some(app_bundle.to_path_buf())
}

pub fn resolve_current_app_bundle() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let exe = std::env::current_exe().ok()?;
        resolve_updatable_app_bundle_from_exe(&exe)
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

pub async fn fetch_latest_manifest(
    client: &reqwest::Client,
) -> Result<ParsedManifest, String> {
    let resp = client
        .get(LATEST_MANIFEST_URL)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parse_manifest_for_platform(&json, current_platform_key()))
}

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct DmgMountGuard {
    mount_point: PathBuf,
    active: bool,
}

impl DmgMountGuard {
    fn detach_now(&mut self) {
        if self.active {
            self.active = false;
            let _ = Command::new("hdiutil")
                .arg("detach")
                .arg(&self.mount_point)
                .arg("-force")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

impl Drop for DmgMountGuard {
    fn drop(&mut self) {
        self.detach_now();
    }
}

fn ensure_parent_writable(app_bundle: &Path, locale: Locale) -> Result<PathBuf, String> {
    let parent = app_bundle.parent().ok_or_else(|| {
        locale
            .pick("无法定位应用安装目录", "Cannot locate application directory")
            .to_string()
    })?;

    let probe = parent.join(format!(".pivot-update-probe-{}", uuid::Uuid::new_v4().simple()));
    match fs::create_dir(&probe) {
        Ok(()) => {
            let _ = fs::remove_dir(&probe);
            Ok(parent.to_path_buf())
        }
        Err(_) => Err(locale
            .pick(
                "当前账户无应用目录写入权限，请手动下载安装包覆盖",
                "No write permission for application directory; please download manually",
            )
            .to_string()),
    }
}

/// 同分区原子替换 `.app` 目录：先将当前 `.app` 备份重命名，再将新 `.app` 移入；若移入失败立即回滚。
pub fn atomic_swap_app_bundle(
    target_app: &Path,
    staging_app: &Path,
    backup_app: &Path,
) -> Result<(), std::io::Error> {
    if backup_app.exists() {
        let _ = fs::remove_dir_all(backup_app);
    }

    let had_existing = target_app.exists();
    if had_existing {
        fs::rename(target_app, backup_app)?;
    }

    if let Err(swap_err) = fs::rename(staging_app, target_app) {
        if had_existing {
            let _ = fs::rename(backup_app, target_app);
        }
        let _ = fs::remove_dir_all(staging_app);
        return Err(swap_err);
    }

    if had_existing {
        let _ = fs::remove_dir_all(backup_app);
    }
    Ok(())
}

fn find_app_in_mount(mount_point: &Path) -> Option<PathBuf> {
    let preferred = mount_point.join("Pivot.app");
    if preferred.is_dir() {
        return Some(preferred);
    }
    let entries = fs::read_dir(mount_point).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.extension().and_then(|e| e.to_str()) == Some("app") {
            return Some(path);
        }
    }
    None
}

fn install_dmg_blocking(
    dmg_path: &Path,
    work_dir: &Path,
    target_app: &Path,
    target_parent: &Path,
    locale: Locale,
) -> Result<(), String> {
    let mount_point = work_dir.join("mnt");
    fs::create_dir_all(&mount_point).map_err(|e| e.to_string())?;

    let attach_out = Command::new("hdiutil")
        .arg("attach")
        .arg(dmg_path)
        .arg("-mountpoint")
        .arg(&mount_point)
        .arg("-nobrowse")
        .arg("-readonly")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("hdiutil: {}", e))?;

    if !attach_out.status.success() {
        return Err(locale
            .pick(
                "安装包镜像校验或挂载失败，请重试或手动下载",
                "Failed to verify or mount DMG image",
            )
            .to_string());
    }

    let mut mount_guard = DmgMountGuard {
        mount_point: mount_point.clone(),
        active: true,
    };

    let source_app = find_app_in_mount(&mount_point).ok_or_else(|| {
        locale
            .pick("镜像中未找到 Pivot.app", "Pivot.app not found in DMG")
            .to_string()
    })?;

    let app_name = target_app
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Pivot.app");
    let token = uuid::Uuid::new_v4().simple().to_string();
    let short_id = &token[..8];
    let staging_app = target_parent.join(format!(".{}.updating-{}", app_name, short_id));
    let backup_app = target_parent.join(format!(".{}.backup-{}", app_name, short_id));

    if staging_app.exists() {
        let _ = fs::remove_dir_all(&staging_app);
    }

    let copy_status = Command::new("ditto")
        .arg(&source_app)
        .arg(&staging_app)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| {
            let _ = fs::remove_dir_all(&staging_app);
            format!("ditto: {}", e)
        })?;

    if !copy_status.success() {
        let _ = fs::remove_dir_all(&staging_app);
        return Err(locale
            .pick("解压拷贝新版本失败", "Failed to extract updated app bundle")
            .to_string());
    }

    // 拷贝完毕即可提前卸载镜像
    mount_guard.detach_now();

    let _ = Command::new("xattr")
        .arg("-cr")
        .arg(&staging_app)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let verify_status = Command::new("codesign")
        .arg("--verify")
        .arg("--deep")
        .arg("--strict")
        .arg(&staging_app)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if !matches!(verify_status, Ok(s) if s.success()) {
        let _ = fs::remove_dir_all(&staging_app);
        return Err(locale
            .pick(
                "更新包签名完整性校验未通过",
                "Updated app bundle signature verification failed",
            )
            .to_string());
    }

    atomic_swap_app_bundle(target_app, &staging_app, &backup_app).map_err(|e| {
        format!(
            "{}: {}",
            locale.pick("替换应用文件失败", "Failed to replace application bundle"),
            e
        )
    })?;

    Ok(())
}

pub async fn download_and_install_update(
    client: reqwest::Client,
    info: UpdateReleaseInfo,
    locale: Locale,
    progress_tx: mpsc::UnboundedSender<InstallProgressEvent>,
) -> Result<PathBuf, String> {
    let asset_url = info.asset_url.ok_or_else(|| {
        locale
            .pick("未找到当前架构的更新包地址", "No update package URL for current architecture")
            .to_string()
    })?;
    let target_app = info.app_bundle.ok_or_else(|| {
        locale
            .pick("当前运行环境不支持原地覆盖更新", "In-place update is not supported here")
            .to_string()
    })?;

    let target_parent = ensure_parent_writable(&target_app, locale)?;

    let work_dir = std::env::temp_dir().join(format!(
        "pivot-update-{}",
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&work_dir).map_err(|e| e.to_string())?;
    let _work_guard = TempDirGuard(work_dir.clone());

    let dmg_path = work_dir.join("PivotUpdate.dmg");

    let mut resp = tokio::time::timeout(Duration::from_secs(15), client.get(&asset_url).send())
        .await
        .map_err(|_| {
            locale
                .pick("连接下载服务器超时", "Connection timed out while downloading")
                .to_string()
        })?
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }

    let total_bytes = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut last_pct: u8 = 0;

    let mut file = tokio::fs::File::create(&dmg_path)
        .await
        .map_err(|e| e.to_string())?;

    loop {
        let next_chunk = tokio::time::timeout(Duration::from_secs(30), resp.chunk())
            .await
            .map_err(|_| {
                locale
                    .pick("下载更新包读取超时", "Download timed out")
                    .to_string()
            })?
            .map_err(|e| e.to_string())?;

        let Some(chunk) = next_chunk else {
            break;
        };

        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        if total_bytes > 0 {
            let pct = ((downloaded.saturating_mul(100)) / total_bytes).min(100) as u8;
            if pct > last_pct {
                last_pct = pct;
                let _ = progress_tx.send(InstallProgressEvent::Downloading(pct));
            }
        }
    }

    file.flush().await.map_err(|e| e.to_string())?;
    file.sync_all().await.map_err(|e| e.to_string())?;
    drop(file);

    if total_bytes > 0 && downloaded != total_bytes {
        return Err(locale
            .pick("更新包下载不完整，请重试", "Incomplete download, please retry")
            .to_string());
    }
    if downloaded == 0 {
        return Err(locale
            .pick("下载到的更新包为空", "Downloaded package is empty")
            .to_string());
    }

    let _ = progress_tx.send(InstallProgressEvent::Installing);

    let dmg_for_block = dmg_path.clone();
    let work_for_block = work_dir.clone();
    let app_for_block = target_app.clone();

    tokio::task::spawn_blocking(move || {
        install_dmg_blocking(
            &dmg_for_block,
            &work_for_block,
            &app_for_block,
            &target_parent,
            locale,
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(target_app)
}

/// 拉起脱离进程组的后台等待脚本：等当前 PID 完全退出释放 16888 网关端口后，重新打开新版 `.app`。
pub fn schedule_relaunch_after_exit(app_bundle: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;

        let pid = std::process::id().to_string();
        Command::new("/bin/sh")
            .arg("-c")
            .arg(r#"for _ in $(seq 1 50); do kill -0 "$1" 2>/dev/null || break; sleep 0.1; done; open -n "$2""#)
            .arg("pivot-relaunch")
            .arg(&pid)
            .arg(app_bundle)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app_bundle;
        Err("Unsupported platform".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_semver() {
        assert!(is_newer_semver("0.3.1", "0.3.0"));
        assert!(is_newer_semver("v0.3.2", "0.3.1"));
        assert!(is_newer_semver("1.0.0", "0.9.99"));
        assert!(!is_newer_semver("0.3.1", "0.3.1"));
        assert!(!is_newer_semver("0.3.0", "0.3.1"));
    }

    #[test]
    fn test_parse_manifest_for_platform() {
        let raw = serde_json::json!({
            "version": "v0.3.1",
            "notes": "Pivot v0.3.1 — 原生 Rust + GPUI 桌面版",
            "platforms": {
                "darwin-aarch64": {
                    "url": "https://github.com/vsnote/pivot/releases/download/v0.3.1/Pivot_0.3.1_aarch64.dmg"
                },
                "darwin-x86_64": {
                    "url": "https://github.com/vsnote/pivot/releases/download/v0.3.1/Pivot_0.3.1_x64.dmg"
                }
            }
        });

        let arm = parse_manifest_for_platform(&raw, "darwin-aarch64");
        assert_eq!(arm.version, "0.3.1");
        assert_eq!(
            arm.asset_url.as_deref(),
            Some("https://github.com/vsnote/pivot/releases/download/v0.3.1/Pivot_0.3.1_aarch64.dmg")
        );

        let x64 = parse_manifest_for_platform(&raw, "darwin-x86_64");
        assert_eq!(
            x64.asset_url.as_deref(),
            Some("https://github.com/vsnote/pivot/releases/download/v0.3.1/Pivot_0.3.1_x64.dmg")
        );
    }

    #[test]
    fn test_extract_release_note_items() {
        assert!(extract_release_note_items(None).is_empty());
        assert!(extract_release_note_items(Some("   \n\t  ")).is_empty());
        assert!(extract_release_note_items(Some("Pivot v0.3.7 — 原生 Rust + GPUI 桌面版")).is_empty());

        let mixed = "\n### 🚀 新功能\n---\n• 支持多订阅等级账号混绑\n- 修复推理首包等待超时 ([aa8e1af](https://github.com/vsnote/pivot/commit/aa8e1af))\n* 优化托盘菜单布局\n· 统一统计页流水文案\n";
        assert_eq!(
            extract_release_note_items(Some(mixed)),
            vec![
                "支持多订阅等级账号混绑",
                "修复推理首包等待超时",
                "优化托盘菜单布局",
                "统一统计页流水文案",
            ]
        );
    }

    #[test]
    fn test_resolve_updatable_app_bundle_from_exe() {
        let normal = Path::new("/Applications/Pivot.app/Contents/MacOS/pivot");
        assert_eq!(
            resolve_updatable_app_bundle_from_exe(normal),
            Some(PathBuf::from("/Applications/Pivot.app"))
        );

        let user_app = Path::new("/Users/demo/Applications/Pivot.app/Contents/MacOS/pivot");
        assert_eq!(
            resolve_updatable_app_bundle_from_exe(user_app),
            Some(PathBuf::from("/Users/demo/Applications/Pivot.app"))
        );

        let dmg_vol = Path::new("/Volumes/Pivot/Pivot.app/Contents/MacOS/pivot");
        assert_eq!(resolve_updatable_app_bundle_from_exe(dmg_vol), None);

        let translocated = Path::new(
            "/private/var/folders/xx/yy/T/AppTranslocation/ABCD/d/Pivot.app/Contents/MacOS/pivot",
        );
        assert_eq!(resolve_updatable_app_bundle_from_exe(translocated), None);

        let dev_bin = Path::new("/Users/demo/projects/pivot/target/debug/pivot");
        assert_eq!(resolve_updatable_app_bundle_from_exe(dev_bin), None);
    }

    #[test]
    fn test_atomic_swap_app_bundle_and_rollback() {
        let base = std::env::temp_dir().join(format!(
            "pivot-swap-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&base).unwrap();

        let target = base.join("Pivot.app");
        let staging = base.join(".Pivot.app.updating");
        let backup = base.join(".Pivot.app.backup");

        // 1. 正常原子替换验证
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("version.txt"), "0.3.0").unwrap();
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("version.txt"), "0.3.1").unwrap();

        atomic_swap_app_bundle(&target, &staging, &backup).unwrap();
        assert_eq!(fs::read_to_string(target.join("version.txt")).unwrap(), "0.3.1");
        assert!(!staging.exists());
        assert!(!backup.exists());

        // 2. 当 staging 不存在导致失败时，原 target 必须自动回滚无损
        let missing_staging = base.join(".Pivot.app.missing");
        let res = atomic_swap_app_bundle(&target, &missing_staging, &backup);
        assert!(res.is_err());
        assert_eq!(fs::read_to_string(target.join("version.txt")).unwrap(), "0.3.1");
        assert!(!backup.exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_install_dmg_blocking_end_to_end() {
        let base = std::env::temp_dir().join(format!(
            "pivot-dmg-e2e-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let dmg_src = base.join("dmg-src");
        let new_app = dmg_src.join("Pivot.app");
        let install_root = base.join("Applications");
        let old_app = install_root.join("Pivot.app");
        let work_dir = base.join("work");

        fs::create_dir_all(new_app.join("Contents/MacOS")).unwrap();
        fs::create_dir_all(old_app.join("Contents/MacOS")).unwrap();
        fs::create_dir_all(&work_dir).unwrap();

        let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>com.pivot.desktop.test</string>
  <key>CFBundleName</key>
  <string>Pivot</string>
  <key>CFBundleExecutable</key>
  <string>pivot</string>
  <key>CFBundleVersion</key>
  <string>0.3.2</string>
</dict>
</plist>"#;
        fs::write(new_app.join("Contents/Info.plist"), plist).unwrap();
        // 拷贝系统极小合法 Mach-O 二进制 `/usr/bin/true` 供 codesign 签名校验
        fs::copy("/usr/bin/true", new_app.join("Contents/MacOS/pivot")).unwrap();
        fs::write(old_app.join("Contents/marker.txt"), "old-0.3.0").unwrap();

        let sign_status = Command::new("codesign")
            .args(["--force", "--deep", "--sign", "-"])
            .arg(&new_app)
            .status()
            .unwrap();
        assert!(sign_status.success());

        let dmg_path = base.join("Pivot_0.3.2_test.dmg");
        let create_status = Command::new("hdiutil")
            .args(["create", "-volname", "Pivot", "-srcfolder"])
            .arg(&dmg_src)
            .args(["-ov", "-format", "ULMO"])
            .arg(&dmg_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(create_status.success());

        install_dmg_blocking(&dmg_path, &work_dir, &old_app, &install_root, Locale::ZhCn)
            .expect("End-to-end DMG mount, verify, and atomic swap should succeed");

        assert!(!old_app.join("Contents/marker.txt").exists());
        assert!(old_app.join("Contents/MacOS/pivot").exists());
        let plist_installed = fs::read_to_string(old_app.join("Contents/Info.plist")).unwrap();
        assert!(plist_installed.contains("0.3.2"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_is_newer_semver_prerelease_and_build() {
        assert!(is_newer_semver("1.0.0", "1.0.0-rc.1"));
        assert!(!is_newer_semver("1.0.0-rc.1", "1.0.0"));
        assert!(is_newer_semver("1.0.0-beta.10", "1.0.0-beta.2"));
        assert!(is_newer_semver("1.0.0-rc.1", "1.0.0-beta.9"));
        assert!(!is_newer_semver("v1.0.0+build.2", "1.0.0+build.1"));
        assert!(is_newer_semver("v1.0.1", "1.0.0-alpha.1"));
    }
}
