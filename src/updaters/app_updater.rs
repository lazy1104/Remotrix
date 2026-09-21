use std::path::{Path, PathBuf};

use crate::aria2_fetcher;

/// How the running app was installed; controls which GitHub release asset
/// is picked and how the downloaded file is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    WindowsSetup,
    Deb,
    AppImage,
}

impl InstallKind {
    /// Whether an installer asset name belongs to this install kind.
    pub fn asset_matches(&self, name: &str) -> bool {
        match self {
            InstallKind::WindowsSetup => name.contains("setup") && name.ends_with(".exe"),
            InstallKind::Deb => name.ends_with(".deb"),
            InstallKind::AppImage => name.ends_with(".AppImage"),
        }
    }
}

/// How the running app was installed. Prefer the real environment, but allow
/// overriding via `REMOTRIX_FORCE_INSTALL_KIND` for testing other branches.
/// Detect the install kind of the currently running binary.
///
/// Honours `REMOTRIX_FORCE_INSTALL_KIND` (values `windows-setup`, `deb`,
/// `appimage`) so other code paths can be exercised in CI; otherwise falls
/// back to [`default_install_kind`] which inspects `$APPIMAGE` on Linux
/// and the target OS elsewhere.
pub fn detect_install_kind() -> InstallKind {
    if let Ok(k) = std::env::var("REMOTRIX_FORCE_INSTALL_KIND") {
        return match k.as_str() {
            "windows-setup" => InstallKind::WindowsSetup,
            "deb" => InstallKind::Deb,
            "appimage" => InstallKind::AppImage,
            _ => default_install_kind(),
        };
    }
    default_install_kind()
}

#[cfg(target_os = "linux")]
fn default_install_kind() -> InstallKind {
    if std::env::var_os("APPIMAGE").is_some() {
        InstallKind::AppImage
    } else {
        InstallKind::Deb
    }
}

#[cfg(target_os = "windows")]
fn default_install_kind() -> InstallKind {
    InstallKind::WindowsSetup
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn default_install_kind() -> InstallKind {
    InstallKind::Deb
}

/// The path of the running AppImage (empty when not running as an AppImage).
/// Return the path of the running AppImage from `$APPIMAGE`, or `None`
/// when the variable is unset/empty. Used by AppImage-specific code paths
/// that must avoid the (read-only) mount path returned by `current_exe`.
pub fn appimage_path() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
}

/// Outcome of [`apply_after_download`].
#[derive(Debug, Clone)]
pub struct AppUpdateOutcome {
    /// Install kind that produced this outcome; used by the UI to decide
    /// which dialog (relaunch / locate / done) to show.
    pub kind: InstallKind,
    /// Path to the artifact on disk after the apply step. `None` for
    /// Windows because the installer process owns the file once spawned.
    pub path: Option<PathBuf>,
}

/// Reduce a GitHub asset name to a safe bare filename, rejecting traversal.
pub(crate) fn sanitize_asset_name(name: &str) -> Result<String, String> {
    let base = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if base.is_empty() || base == "." || base == ".." {
        return Err(format!("invalid asset name: {name:?}"));
    }
    Ok(base.to_string())
}

/// Atomically replace the running AppImage at `target` with the newly
/// downloaded `new`, keeping a `.bak` until the swap succeeds and restoring it
/// if the swap fails. The new file must live in the same directory as `target`
/// so the renames stay on one filesystem.
#[cfg(target_os = "linux")]
/// Atomically replace the running AppImage at `target` with the newly
/// downloaded `new`, keeping a `.bak` until the swap succeeds and restoring it
/// if the swap fails. The new file must live in the same directory as `target`
/// so the renames stay on one filesystem.
#[cfg(target_os = "linux")]
pub fn replace_appimage(new: &Path, target: &Path) -> Result<(), String> {
    if new == target {
        return Err("new AppImage path equals target".to_string());
    }
    let bak = target.with_extension("bak");
    std::fs::rename(target, &bak).map_err(|e| format!("rename target->bak: {e}"))?;
    if let Err(e) = std::fs::rename(new, target) {
        let _ = std::fs::rename(&bak, target);
        return Err(format!("rename new->target: {e}"));
    }
    aria2_fetcher::set_perms(target)?;
    let _ = std::fs::remove_file(&bak);
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn replace_appimage(_new: &Path, _target: &Path) -> Result<(), String> {
    Err("AppImage replacement is only supported on Linux".to_string())
}

/// Spawn a fresh instance after an update, using `$APPIMAGE` when running as
/// an AppImage (the mount point is read-only, so `current_exe()` is invalid),
/// otherwise the current executable.
pub fn relaunch_after_update() {
    let exe = appimage_path()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::env::current_exe().unwrap_or_else(|_| PathBuf::new()));
    if exe.as_os_str().is_empty() {
        return;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _ = std::process::Command::new(exe)
        .env("REMOTRIX_RESTART", "1")
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Launch the downloaded setup.exe installer (Windows in-place update).
#[cfg(target_os = "windows")]
/// Launch the downloaded `setup.exe` installer in-place.
#[cfg(target_os = "windows")]
pub fn run_installer(path: &Path) -> Result<(), String> {
    std::process::Command::new(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("spawn installer: {e}"))
}

#[cfg(not(target_os = "windows"))]
pub fn run_installer(_path: &Path) -> Result<(), String> {
    Err("installer launch is only supported on Windows".to_string())
}

/// Directory where downloaded packages (.deb) are placed. Falls back to the
/// data directory when `download_dir` is empty or `.`.
/// Directory where downloaded packages (e.g. `.deb`) are placed. Falls
/// back to `<data_home>/remotrix/downloads` when `download_dir` is `None`,
/// empty, or `.`.
pub fn packages_dir(download_dir: Option<&Path>) -> PathBuf {
    if let Some(dir) = download_dir {
        let s = dir.to_string_lossy();
        if !s.trim().is_empty() && s.trim() != "." {
            return dir.to_path_buf();
        }
    }
    crate::config::data_home()
        .unwrap_or_else(std::env::temp_dir)
        .join("remotrix")
        .join("downloads")
}

/// Compute the destination path for an installer package based on install kind.
/// Keeps the AppImage guard and path logic in one place so the reqwest fallback
/// and the engine-routed path share it.
/// Compute the destination path for an installer package based on install
/// kind. Keeps the AppImage guard and path logic in one place so the
/// reqwest fallback and the engine-routed path share it.
///
/// # Errors
/// Returns an error if the asset name fails [`sanitize_asset_name`] (empty,
/// `.`, `..`, or contains path separators) or if `AppImage` is requested
/// but `$APPIMAGE` is not set.
pub fn app_update_dest(
    kind: InstallKind,
    asset_name: &str,
    download_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    let asset_name = sanitize_asset_name(asset_name)?;
    if kind == InstallKind::AppImage && appimage_path().is_none() {
        return Err("not running as an AppImage".to_string());
    }
    match kind {
        InstallKind::AppImage => {
            let target = appimage_path().expect("checked AppImage above");
            let parent = target
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .ok_or("invalid AppImage path")?;
            Ok(parent.join(&asset_name))
        }
        InstallKind::WindowsSetup => Ok(std::env::temp_dir().join(&asset_name)),
        InstallKind::Deb => Ok(packages_dir(download_dir).join(&asset_name)),
    }
}

/// Apply the kind-specific side effects after an installer has been
/// downloaded to `dest`: swap the running AppImage, spawn the Windows
/// installer, or simply report the path for the user to install the
/// Debian package manually.
pub fn apply_after_download(kind: InstallKind, dest: &Path) -> Result<AppUpdateOutcome, String> {
    match kind {
        InstallKind::AppImage => {
            let target = appimage_path().expect("checked AppImage above");
            replace_appimage(dest, &target)?;
            Ok(AppUpdateOutcome {
                kind,
                path: Some(target),
            })
        }
        InstallKind::WindowsSetup => {
            run_installer(dest)?;
            Ok(AppUpdateOutcome { kind, path: None })
        }
        InstallKind::Deb => Ok(AppUpdateOutcome {
            kind,
            path: Some(dest.to_path_buf()),
        }),
    }
}

/// The version baked into the binary at compile time (from
/// `Cargo.toml`'s `version` field). Compared against the GitHub release
/// tag by the update checker.
pub fn current_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_asset_name_valid() {
        assert_eq!(
            sanitize_asset_name("remotrix_0.1.2_amd64.deb").unwrap(),
            "remotrix_0.1.2_amd64.deb"
        );
        assert_eq!(
            sanitize_asset_name("Remotrix-0.1.2.AppImage").unwrap(),
            "Remotrix-0.1.2.AppImage"
        );
        assert_eq!(
            sanitize_asset_name("Remotrix_0.1.2_x64-setup.exe").unwrap(),
            "Remotrix_0.1.2_x64-setup.exe"
        );
    }

    #[test]
    fn sanitize_asset_name_strips_directory() {
        // The basename is taken; directory parts are dropped silently.
        let r = sanitize_asset_name("some/dir/asset.deb");
        assert_eq!(r.unwrap(), "asset.deb");
    }

    #[test]
    fn sanitize_asset_name_rejects_empty() {
        assert!(sanitize_asset_name("").is_err());
    }

    #[test]
    fn sanitize_asset_name_rejects_dot() {
        assert!(sanitize_asset_name(".").is_err());
    }

    #[test]
    fn sanitize_asset_name_rejects_double_dot() {
        assert!(sanitize_asset_name("..").is_err());
    }

    #[test]
    fn packages_dir_falls_back_for_none() {
        let p = packages_dir(None);
        assert!(p.ends_with("downloads"));
    }

    #[test]
    fn packages_dir_falls_back_for_empty() {
        let empty = std::path::Path::new("");
        let p = packages_dir(Some(empty));
        assert!(p.ends_with("downloads"));
    }

    #[test]
    fn packages_dir_falls_back_for_dot() {
        let dot = std::path::Path::new(".");
        let p = packages_dir(Some(dot));
        assert!(p.ends_with("downloads"));
    }

    #[test]
    fn packages_dir_passes_through_valid_path() {
        let dir = std::path::PathBuf::from("/var/cache/remotrix");
        let p = packages_dir(Some(&dir));
        assert_eq!(p, dir);
    }

    #[test]
    fn app_update_dest_deb_uses_packages_dir() {
        let dest = app_update_dest(InstallKind::Deb, "remotrix_0.1.2_amd64.deb", None).unwrap();
        assert!(dest.ends_with("remotrix_0.1.2_amd64.deb"));
    }

    #[test]
    fn app_update_dest_windows_setup_uses_temp_dir() {
        let dest = app_update_dest(InstallKind::WindowsSetup, "remotrix-setup.exe", None).unwrap();
        assert!(dest.ends_with("remotrix-setup.exe"));
    }

    #[test]
    fn app_update_dest_appimage_requires_appimage_env() {
        // Without $APPIMAGE, the call must fail.
        std::env::remove_var("APPIMAGE");
        let result = app_update_dest(InstallKind::AppImage, "Remotrix.AppImage", None);
        assert!(result.is_err());
    }

    #[test]
    fn app_update_dest_rejects_invalid_asset_name() {
        let result = app_update_dest(InstallKind::Deb, "..", None);
        assert!(result.is_err());
    }

    #[test]
    fn install_kind_asset_matches() {
        assert!(InstallKind::Deb.asset_matches("remotrix_0.1.2_amd64.deb"));
        assert!(!InstallKind::Deb.asset_matches("remotrix.AppImage"));
        assert!(InstallKind::AppImage.asset_matches("Remotrix-0.1.2.AppImage"));
        assert!(InstallKind::WindowsSetup.asset_matches("remotrix-setup.exe"));
        assert!(!InstallKind::WindowsSetup.asset_matches("remotrix.exe"));
    }

    #[test]
    fn apply_after_download_deb_reports_path() {
        let dest = std::path::Path::new("/tmp/remotrix_0.1.2_amd64.deb");
        let outcome = apply_after_download(InstallKind::Deb, dest).unwrap();
        assert_eq!(outcome.kind, InstallKind::Deb);
        assert_eq!(outcome.path.as_deref(), Some(dest));
    }

    #[test]
    fn detect_install_kind_force_override() {
        std::env::set_var("REMOTRIX_FORCE_INSTALL_KIND", "deb");
        assert_eq!(detect_install_kind(), InstallKind::Deb);
        std::env::set_var("REMOTRIX_FORCE_INSTALL_KIND", "appimage");
        assert_eq!(detect_install_kind(), InstallKind::AppImage);
        std::env::remove_var("REMOTRIX_FORCE_INSTALL_KIND");
    }
}
