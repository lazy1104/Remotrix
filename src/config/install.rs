use std::path::PathBuf;

/// The real, persistent launcher path: `$APPIMAGE` when running as an AppImage
/// (the mount `current_exe()`/`/proc/self/exe` is temporary and read-only),
/// otherwise the regular executable.
pub(crate) fn app_launch_exe() -> Option<PathBuf> {
    crate::app_updater::appimage_path()
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::current_exe().ok())
}

/// Install (or refresh) the user-scope `remotrix.desktop` XDG entry.
///
/// Writes under `$XDG_DATA_HOME/applications/` and is a no-op when:
/// - running under an AppImage (the AppImage runtime provides its own entry);
/// - the data directory cannot be resolved;
/// - the launcher path cannot be determined;
/// - the file already exists with identical contents.
///
/// The function always tries to remove the entry first when running under
/// AppImage so a previously-installed broken entry does not linger.
pub fn install_desktop_file() {
    use super::paths::data_home;

    if crate::app_updater::appimage_path().is_some() {
        if let Some(data_home) = data_home() {
            let _ = std::fs::remove_file(data_home.join("applications").join("remotrix.desktop"));
        }
        return;
    }
    let Some(data_home) = data_home() else { return };
    let dir = data_home.join("applications");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("remotrix.desktop");
    let Some(exe) = app_launch_exe() else {
        return;
    };
    let content = format!(
        "{}StartupWMClass=remotrix\n",
        desktop_entry_header(&format!("\"{}\"", escape_exec(&exe.display().to_string())))
    );
    if path.exists() {
        if let Ok(existing) = std::fs::read_to_string(&path) {
            if existing == content {
                return;
            }
        }
    }
    let tmp = path.with_extension("desktop.tmp");
    if std::fs::write(&tmp, &content).is_ok() {
        if std::fs::rename(&tmp, &path).is_ok() {
            install_user_icon(&data_home);
        }
    }
}

const USER_APP_ICON: &[u8] = include_bytes!("../../assets/icon.png");

fn install_user_icon(data_home: &std::path::Path) {
    let icon_dir = data_home
        .join("icons")
        .join("hicolor")
        .join("256x256")
        .join("apps");
    if std::fs::create_dir_all(&icon_dir).is_err() {
        return;
    }
    let icon_path = icon_dir.join("remotrix.png");
    let needs_write = match std::fs::read(&icon_path) {
        Ok(existing) => existing != USER_APP_ICON,
        Err(_) => true,
    };
    if !needs_write {
        return;
    }
    let tmp = icon_path.with_extension("png.tmp");
    if std::fs::write(&tmp, USER_APP_ICON).is_ok() {
        let _ = std::fs::rename(&tmp, &icon_path);
    }
    refresh_icon_theme_cache(&icon_dir);
}

#[cfg(not(target_os = "windows"))]
fn refresh_icon_theme_cache(icon_dir: &std::path::Path) {
    let Some(theme_root) = icon_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
    else {
        return;
    };
    let status = std::process::Command::new("gtk-update-icon-cache")
        .arg("-f")
        .arg(&theme_root)
        .arg("-q")
        .status();
    match status {
        Ok(s) if !s.success() => tracing::warn!(
            theme_root = %theme_root.display(),
            "gtk-update-icon-cache returned non-zero"
        ),
        Err(e) => tracing::warn!(
            theme_root = %theme_root.display(),
            error = %e,
            "gtk-update-icon-cache unavailable"
        ),
        _ => {}
    }
}

/// Compose the standard `[Desktop Entry]` header used by
/// [`install_desktop_file`], with the supplied (already-quoted and escaped)
/// `Exec=` line. Kept separate so the format is unit-testable without
/// touching the filesystem.
pub(crate) fn desktop_entry_header(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Remotrix\n\
         Comment=Download manager\n\
         Exec={exec}\n\
         Icon=remotrix\n\
         Terminal=false\n\
         Categories=Network;FileTransfer;\n"
    )
}

/// Escape backslashes and double quotes for safe inclusion in a `.desktop`
/// `Exec=` line. The caller is responsible for wrapping the result in
/// surrounding quotes before passing it to [`desktop_entry_header`].
pub(crate) fn escape_exec(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_exec_passthrough() {
        assert_eq!(escape_exec("remotrix"), "remotrix");
        assert_eq!(escape_exec("/usr/bin/remotrix"), "/usr/bin/remotrix");
    }

    #[test]
    fn escape_exec_quotes_and_backslashes() {
        assert_eq!(escape_exec("a\"b"), "a\\\"b");
        assert_eq!(escape_exec("a\\b"), "a\\\\b");
        assert_eq!(escape_exec("\\"), "\\\\");
    }

    #[test]
    fn desktop_entry_header_basic() {
        let h = desktop_entry_header("\"/usr/bin/remotrix\"");
        assert!(h.starts_with("[Desktop Entry]\n"));
        assert!(h.contains("Type=Application"));
        assert!(h.contains("Name=Remotrix"));
        assert!(h.contains("Exec=\"/usr/bin/remotrix\""));
        assert!(h.contains("Icon=remotrix"));
        assert!(h.contains("Terminal=false"));
        assert!(h.contains("Categories=Network;FileTransfer;"));
    }

    #[test]
    fn desktop_entry_header_preserves_appimage_exec() {
        let exec = "\"$APPIMAGE\" --no-sandbox";
        let h = desktop_entry_header(exec);
        assert!(h.contains("Exec=\"$APPIMAGE\" --no-sandbox"));
    }
}
