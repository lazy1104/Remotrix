use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::aria2_fetcher;

pub struct AppUpdateDownload {
    pub version: String,
    pub slug: String,
    pub download_url: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PendingInfo {
    version: String,
    slug: String,
    sha256: String,
}

const PENDING_FILE: &str = ".pending-update";

pub fn app_update_dir() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
    let dir = proj.data_dir().join("app");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

pub async fn stage_app_update(download: &AppUpdateDownload) -> Result<String, String> {
    let dir = app_update_dir().ok_or("cannot determine app data directory")?;
    let bin_name = format!("remotrix-{}-{}", download.version, download.slug);
    let part_path = dir.join(format!("{bin_name}.part"));
    let bin_path = dir.join(&bin_name);

    aria2_fetcher::download_file(&download.download_url, &part_path).await?;

    if let Some(expected) = &download.sha256 {
        let digest = aria2_fetcher::sha256_file(&part_path).map_err(|e| format!("sha256: {e}"))?;
        if digest != *expected {
            let _ = std::fs::remove_file(&part_path);
            return Err(format!(
                "sha256 mismatch: expected {expected}, got {digest}"
            ));
        }
    }

    std::fs::rename(&part_path, &bin_path).map_err(|e| format!("rename: {e}"))?;
    aria2_fetcher::set_perms(&bin_path)?;

    let pending = PendingInfo {
        version: download.version.clone(),
        slug: download.slug.clone(),
        sha256: download.sha256.clone().unwrap_or_default(),
    };
    let json =
        serde_json::to_string_pretty(&pending).map_err(|e| format!("serialize pending: {e}"))?;
    std::fs::write(dir.join(PENDING_FILE), &json)
        .map_err(|e| format!("write {PENDING_FILE}: {e}"))?;

    Ok(download.version.clone())
}

pub fn apply_pending_app_update() -> Result<Option<String>, String> {
    let Some(dir) = app_update_dir() else {
        return Ok(None);
    };
    let pending_path = dir.join(PENDING_FILE);
    let Ok(content) = std::fs::read_to_string(&pending_path) else {
        return Ok(None);
    };
    let pending: PendingInfo = match serde_json::from_str(&content) {
        Ok(p) => p,
        Err(_) => {
            let _ = std::fs::remove_file(&pending_path);
            return Ok(None);
        }
    };
    let bin_name = format!("remotrix-{}-{}", pending.version, pending.slug);
    let staged = dir.join(&bin_name);
    if !staged.exists() {
        let _ = std::fs::remove_file(&pending_path);
        return Ok(None);
    }
    match aria2_fetcher::sha256_file(&staged) {
        Ok(digest) if digest == pending.sha256 => {}
        _ => {
            let _ = std::fs::remove_file(&staged);
            let _ = std::fs::remove_file(&pending_path);
            return Ok(None);
        }
    }

    let current_exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    swap_in_pending(&current_exe, &staged)?;
    let _ = std::fs::remove_file(&pending_path);
    Ok(Some(pending.version))
}

/// Relaunch a fresh instance so a just-swapped binary takes effect.
/// On Windows the detached swap helper (spawned inside `swap_in_pending`)
/// is responsible for relaunching, so this is a no-op there.
pub fn relaunch_after_update() {
    #[cfg(unix)]
    {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let args: Vec<String> = std::env::args().skip(1).collect();
        let _ = std::process::Command::new(exe)
            .env("REMOTRIX_RESTART", "1")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    #[cfg(windows)]
    {
        // swap helper already scheduled the relaunch; nothing to do here.
    }
}

#[cfg(unix)]
fn swap_in_pending(current_exe: &Path, staged: &Path) -> Result<(), String> {
    let bak = current_exe.with_extension("bak");
    std::fs::rename(current_exe, &bak).map_err(|e| format!("rename current->bak: {e}"))?;
    std::fs::rename(staged, current_exe).map_err(|e| format!("rename staged->current: {e}"))?;
    aria2_fetcher::set_perms(current_exe)?;
    Ok(())
}

#[cfg(windows)]
fn swap_in_pending(current_exe: &Path, staged: &Path) -> Result<(), String> {
    use std::io::Write;

    let pid = std::process::id();
    let dir = app_update_dir().ok_or("cannot determine app data directory")?;
    let helper = dir.join("swap-update.cmd");
    let bak = format!("{}.bak", current_exe.display());
    let current = current_exe.display().to_string();
    let staged = staged.display().to_string();

    let script = format!(
        "@echo off\r\n\
         :wait\r\n\
         tasklist /FI \"PID eq {pid}\" 2>nul | find \"{pid}\" >nul\r\n\
         if not errorlevel 1 (\r\n\
         \x20 timeout /t 1 /nobreak >nul\r\n\
         \x20 goto wait\r\n\
         )\r\n\
         move /Y \"{current}\" \"{bak}\" >nul\r\n\
         move /Y \"{staged}\" \"{current}\" >nul\r\n\
         start \"\" \"{current}\"\r\n"
    );
    let mut f = std::fs::File::create(&helper).map_err(|e| format!("create swap helper: {e}"))?;
    f.write_all(script.as_bytes())
        .map_err(|e| format!("write swap helper: {e}"))?;

    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", "/min", helper.to_string_lossy().as_ref()])
        .spawn();
    Ok(())
}

pub fn current_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
