//! Application auto-update system.
//!
//! Checks GitHub Releases for newer versions, downloads platform-specific
//! assets, and performs in-place installation with automatic relaunch.

use std::io::Read;
use std::path::{Path, PathBuf};

const GITHUB_API_LATEST: &str = "https://api.github.com/repos/awebo-org/awebo-app/releases/latest";

/// Parsed semantic version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre: String,
}

impl Version {
    /// Parse a semver string, optionally prefixed with `v`.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.strip_prefix('v').unwrap_or(s);
        let mut parts = s.splitn(2, '-');
        let version_part = parts.next()?;
        let pre = parts.next().unwrap_or("").to_string();

        let nums: Vec<&str> = version_part.split('.').collect();
        if nums.len() != 3 {
            return None;
        }
        Some(Self {
            major: nums[0].parse().ok()?,
            minor: nums[1].parse().ok()?,
            patch: nums[2].parse().ok()?,
            pre,
        })
    }

    /// Whether this is a pre-release version.
    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }

    /// Whether `self` is newer than `other`.
    pub fn is_newer_than(&self, other: &Version) -> bool {
        let self_tuple = (self.major, self.minor, self.patch);
        let other_tuple = (other.major, other.minor, other.patch);

        if self_tuple != other_tuple {
            return self_tuple > other_tuple;
        }

        match (self.is_prerelease(), other.is_prerelease()) {
            (false, true) => true,
            (true, false) => false,
            (true, true) => self.pre > other.pre,
            (false, false) => false,
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre)?;
        }
        Ok(())
    }
}

/// Information about an available release.
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version: Version,
    pub download_url: String,
    pub asset_name: String,
}

/// Current application version from Cargo.toml (compile-time).
pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("invalid CARGO_PKG_VERSION")
}

/// Construct the expected asset filename for this platform.
pub fn asset_name(version: &str) -> Option<String> {
    let target = target_triple()?;
    let ext = platform_extension();
    Some(format!("Awebo-v{version}-{target}.{ext}"))
}

/// Target triple for the current build.
fn target_triple() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        _ => None,
    }
}

/// File extension for the platform-specific release artifact.
fn platform_extension() -> &'static str {
    match std::env::consts::OS {
        "macos" => "dmg",
        _ => "tar.gz",
    }
}

/// Check GitHub Releases for a newer version (blocking).
///
/// Returns `Some(ReleaseInfo)` when a newer stable release exists.
/// Skips pre-releases unless the current build is itself a pre-release.
pub fn check_for_update() -> Result<Option<ReleaseInfo>, String> {
    let current = current_version();

    let response = ureq::get(GITHUB_API_LATEST)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "awebo-updater")
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let text = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {e}"))?;

    let tag = json["tag_name"]
        .as_str()
        .ok_or("Missing tag_name")?
        .to_string();

    let remote_version = Version::parse(&tag).ok_or_else(|| format!("Cannot parse tag: {tag}"))?;

    if remote_version.is_prerelease() && !current.is_prerelease() {
        return Ok(None);
    }

    if !remote_version.is_newer_than(&current) {
        return Ok(None);
    }

    let version_str = remote_version.to_string();
    let expected_asset = asset_name(&version_str).ok_or("Unsupported platform")?;

    let download_url = json["assets"]
        .as_array()
        .ok_or("Missing assets")?
        .iter()
        .find(|a| a["name"].as_str() == Some(&expected_asset))
        .and_then(|a| a["browser_download_url"].as_str())
        .ok_or_else(|| format!("No asset matching {expected_asset}"))?
        .to_string();

    Ok(Some(ReleaseInfo {
        version: remote_version,
        download_url,
        asset_name: expected_asset,
    }))
}

/// Download a release asset to the updates cache directory (blocking).
///
/// Returns the path to the downloaded file.
pub fn download_update(info: &ReleaseInfo) -> Result<PathBuf, String> {
    let cache_dir = updates_cache_dir();
    std::fs::create_dir_all(&cache_dir).map_err(|e| format!("Failed to create cache dir: {e}"))?;

    let dest = cache_dir.join(&info.asset_name);
    let tmp = dest.with_extension("part");

    log::info!("Downloading update from {}", info.download_url);

    let response = ureq::get(&info.download_url)
        .header("User-Agent", "awebo-updater")
        .call()
        .map_err(|e| format!("Download request failed: {e}"))?;

    let mut reader = response.into_body().into_reader();
    let mut file =
        std::fs::File::create(&tmp).map_err(|e| format!("Failed to create temp file: {e}"))?;

    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Download read error: {e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| format!("Write error: {e}"))?;
    }

    std::fs::rename(&tmp, &dest).map_err(|e| format!("Failed to finalize download: {e}"))?;

    log::info!("Update downloaded to {}", dest.display());
    Ok(dest)
}

/// Stage a downloaded update for installation on next launch.
///
/// On macOS: mounts the DMG, copies the `.app` bundle into the staging
/// directory, unmounts the DMG, and writes a marker file so the next launch
/// can detect and apply the update.
#[cfg(target_os = "macos")]
pub fn stage_update(dmg_path: &Path) -> Result<(), String> {
    let mount_output = std::process::Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-noverify", "-noautoopen"])
        .arg(dmg_path)
        .output()
        .map_err(|e| format!("hdiutil attach failed: {e}"))?;

    if !mount_output.status.success() {
        let stderr = String::from_utf8_lossy(&mount_output.stderr);
        return Err(format!("hdiutil attach failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&mount_output.stdout);
    let mount_point =
        parse_hdiutil_mount_point(&stdout).ok_or("Failed to find mount point in hdiutil output")?;

    let result = stage_from_mount(&mount_point);

    let _ = std::process::Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(&mount_point)
        .output();

    let _ = std::fs::remove_file(dmg_path);

    result
}

/// Copy the `.app` from a mounted volume into the staging directory and
/// write the pending-update marker.
#[cfg(target_os = "macos")]
fn stage_from_mount(mount_point: &Path) -> Result<(), String> {
    let new_app = find_app_in_volume(mount_point)?;
    let staging = staged_app_path();

    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .map_err(|e| format!("Failed to remove old staged app: {e}"))?;
    }

    if let Some(parent) = staging.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create staging dir: {e}"))?;
    }

    let status = std::process::Command::new("cp")
        .args(["-R"])
        .arg(&new_app)
        .arg(&staging)
        .status()
        .map_err(|e| format!("Failed to copy app bundle: {e}"))?;

    if !status.success() {
        return Err("cp -R failed while staging app bundle".into());
    }

    std::fs::write(pending_marker_path(), "")
        .map_err(|e| format!("Failed to write pending marker: {e}"))?;

    log::info!("Update staged at {}", staging.display());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn stage_update(_path: &Path) -> Result<(), String> {
    Err("Auto-install not yet supported on this platform. Please download manually.".into())
}

/// Check for a staged update and apply it before the app fully starts.
///
/// Called very early in `main()`. If a pending update exists, replaces the
/// current `.app` bundle (or binary) with the staged version and re-execs
/// the process so the user immediately runs the new build.
///
/// Returns `true` if an update was applied (caller should expect re-exec),
/// `false` if there was nothing to apply.
#[cfg(target_os = "macos")]
pub fn apply_pending_update() -> bool {
    let marker = pending_marker_path();
    if !marker.exists() {
        return false;
    }

    let staged = staged_app_path();
    if !staged.exists() {
        let _ = std::fs::remove_file(&marker);
        return false;
    }

    let app_bundle = match find_app_bundle() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("updater: cannot locate app bundle: {e}");
            let _ = std::fs::remove_file(&marker);
            return false;
        }
    };

    let backup = app_bundle.with_extension("app.old");
    if backup.exists() {
        let _ = std::fs::remove_dir_all(&backup);
    }

    if let Err(e) = std::fs::rename(&app_bundle, &backup) {
        eprintln!("updater: failed to move current bundle to backup: {e}");
        let _ = std::fs::remove_file(&marker);
        return false;
    }

    if let Err(e) = std::fs::rename(&staged, &app_bundle) {
        eprintln!("updater: failed to move staged bundle into place: {e}");
        let _ = std::fs::rename(&backup, &app_bundle);
        let _ = std::fs::remove_file(&marker);
        return false;
    }

    let _ = std::fs::remove_dir_all(&backup);
    let _ = std::fs::remove_file(&marker);

    eprintln!("updater: update applied, re-launching…");

    let exe = app_bundle.join("Contents").join("MacOS").join("awebo");

    let args: Vec<String> = std::env::args().skip(1).collect();

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(&exe).args(&args).exec();
        eprintln!("updater: re-exec failed: {err}");
    }

    true
}

#[cfg(not(target_os = "macos"))]
pub fn apply_pending_update() -> bool {
    false
}

/// Spawn a detached instance of the current executable so the app relaunches
/// after the event loop exits. The new process will call
/// `apply_pending_update()` early in `main()` and swap the staged bundle.
pub fn spawn_relaunch() {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("updater: cannot determine current exe for relaunch: {e}");
            return;
        }
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    match std::process::Command::new(&exe)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .spawn()
    {
        Ok(_) => eprintln!("updater: relaunch spawned"),
        Err(e) => eprintln!("updater: relaunch failed: {e}"),
    }
}

/// Cache directory for downloaded update assets.
fn updates_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("awebo")
        .join("updates")
}

/// Path where the staged `.app` bundle is stored before the swap.
fn staged_app_path() -> PathBuf {
    updates_cache_dir().join("Awebo.app")
}

/// Marker file whose existence signals a pending update.
fn pending_marker_path() -> PathBuf {
    updates_cache_dir().join("pending")
}

/// Resolve the running `.app` bundle path.
///
/// Walks up from the current executable looking for a directory ending in
/// `.app`. Falls back to `/Applications/Awebo.app` when running outside
/// a bundle (e.g. via `cargo run`).
#[cfg(target_os = "macos")]
fn find_app_bundle() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot get exe path: {e}"))?;

    let mut path = exe.as_path();
    while let Some(parent) = path.parent() {
        if path.extension().is_some_and(|ext| ext == "app") {
            return Ok(path.to_path_buf());
        }
        path = parent;
    }

    let fallback = PathBuf::from("/Applications/Awebo.app");
    if fallback.exists() {
        return Ok(fallback);
    }

    Err("Cannot locate Awebo.app bundle".into())
}

/// Parse the mount point path from `hdiutil attach` stdout.
#[cfg(target_os = "macos")]
fn parse_hdiutil_mount_point(stdout: &str) -> Option<PathBuf> {
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if let Some(idx) = trimmed.rfind('\t') {
            let path_str = trimmed[idx + 1..].trim();
            if path_str.starts_with('/') {
                return Some(PathBuf::from(path_str));
            }
        }
    }
    None
}

/// Find the `.app` bundle inside a mounted DMG volume.
#[cfg(target_os = "macos")]
fn find_app_in_volume(mount_point: &Path) -> Result<PathBuf, String> {
    let entries =
        std::fs::read_dir(mount_point).map_err(|e| format!("Cannot read mount point: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "app") {
            return Ok(path);
        }
    }
    Err(format!("No .app found in {}", mount_point.display()))
}

/// Spawn a background update check. Sends result via the event-loop proxy.
pub fn spawn_update_check(
    proxy: winit::event_loop::EventLoopProxy<crate::terminal::TerminalEvent>,
) {
    std::thread::spawn(move || match check_for_update() {
        Ok(Some(info)) => {
            log::info!("Update available: v{}", info.version);
            let _ = proxy.send_event(crate::terminal::TerminalEvent::UpdateAvailable(info));
        }
        Ok(None) => {
            log::info!("No update available");
        }
        Err(e) => {
            log::warn!("Update check failed: {e}");
        }
    });
}

/// Spawn a background download. Sends result via the event-loop proxy.
pub fn spawn_update_download(
    info: ReleaseInfo,
    proxy: winit::event_loop::EventLoopProxy<crate::terminal::TerminalEvent>,
) {
    std::thread::spawn(move || match download_update(&info) {
        Ok(path) => {
            log::info!("Update downloaded: {}", path.display());
            let _ = proxy.send_event(crate::terminal::TerminalEvent::UpdateDownloaded(path));
        }
        Err(e) => {
            log::error!("Update download failed: {e}");
            let _ = proxy.send_event(crate::terminal::TerminalEvent::UpdateFailed(e));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_version() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.pre.is_empty());
    }

    #[test]
    fn parse_version_with_v_prefix() {
        let v = Version::parse("v0.1.0").unwrap();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn parse_prerelease() {
        let v = Version::parse("v2.0.0-beta.1").unwrap();
        assert_eq!(v.major, 2);
        assert!(v.is_prerelease());
        assert_eq!(v.pre, "beta.1");
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(Version::parse("").is_none());
        assert!(Version::parse("1.2").is_none());
        assert!(Version::parse("abc").is_none());
        assert!(Version::parse("1.2.x").is_none());
    }

    #[test]
    fn version_comparison() {
        let v010 = Version::parse("0.1.0").unwrap();
        let v011 = Version::parse("0.1.1").unwrap();
        let v020 = Version::parse("0.2.0").unwrap();
        let v100 = Version::parse("1.0.0").unwrap();

        assert!(v011.is_newer_than(&v010));
        assert!(v020.is_newer_than(&v011));
        assert!(v100.is_newer_than(&v020));
        assert!(!v010.is_newer_than(&v011));
        assert!(!v010.is_newer_than(&v010));
    }

    #[test]
    fn stable_beats_prerelease() {
        let stable = Version::parse("1.0.0").unwrap();
        let pre = Version::parse("1.0.0-rc.1").unwrap();
        assert!(stable.is_newer_than(&pre));
        assert!(!pre.is_newer_than(&stable));
    }

    #[test]
    fn version_display() {
        assert_eq!(Version::parse("1.2.3").unwrap().to_string(), "1.2.3");
        assert_eq!(
            Version::parse("v2.0.0-beta.1").unwrap().to_string(),
            "2.0.0-beta.1"
        );
    }

    #[test]
    fn asset_name_current_platform() {
        let name = asset_name("0.2.0");
        assert!(name.is_some());
        let name = name.unwrap();
        assert!(name.starts_with("Awebo-v0.2.0-"));
        #[cfg(target_os = "macos")]
        assert!(name.ends_with(".dmg"));
    }

    #[test]
    fn current_version_parses() {
        let v = current_version();
        assert!(v.major < 100);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn parse_hdiutil_output() {
        let sample = "/dev/disk4          \tGUID_partition_scheme          \t\n\
                       /dev/disk4s1        \tApple_HFS                      \t/Volumes/Awebo\n";
        let mp = parse_hdiutil_mount_point(sample).unwrap();
        assert_eq!(mp, PathBuf::from("/Volumes/Awebo"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn find_app_bundle_from_exe_path() {
        let bundle = find_app_bundle();
        // In test context (cargo test) there's no .app bundle, so this may
        // return the fallback or an error — either is acceptable.
        if let Ok(p) = bundle {
            assert!(p.to_string_lossy().ends_with(".app"));
        }
    }

    #[test]
    fn updates_cache_dir_is_absolute() {
        let dir = updates_cache_dir();
        assert!(dir.is_absolute());
        assert!(dir.to_string_lossy().contains("awebo"));
    }

    #[test]
    fn staging_paths_inside_cache_dir() {
        let cache = updates_cache_dir();
        assert!(staged_app_path().starts_with(&cache));
        assert!(pending_marker_path().starts_with(&cache));
    }

    #[test]
    fn no_pending_update_without_marker() {
        assert!(!apply_pending_update());
    }
}
