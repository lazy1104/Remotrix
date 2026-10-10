use std::path::PathBuf;

use super::settings::UrlSchemePrefs;

/// The real, persistent launcher path: `$APPIMAGE` when running as an AppImage
/// (the mount `current_exe()`/`/proc/self/exe` is temporary and read-only),
/// otherwise the regular executable.
pub(crate) fn app_launch_exe() -> Option<PathBuf> {
    crate::app_updater::appimage_path()
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::current_exe().ok())
}

pub(crate) fn enabled_schemes(prefs: &UrlSchemePrefs) -> Vec<&'static str> {
    let mut out = Vec::new();
    if prefs.register_remotrix {
        out.push("remotrix");
    }
    if prefs.register_magnet {
        out.push("magnet");
    }
    if prefs.register_ed2k {
        out.push("ed2k");
    }
    out
}

/// Install (or refresh) the user-scope `remotrix.desktop` XDG entry.
///
/// Writes under `$XDG_DATA_HOME/applications/` and is a no-op when:
/// - running under an AppImage (the AppImage runtime provides its own entry);
/// - no URL scheme is enabled (all flags off) — the existing file is removed
///   instead so the user is not left with stale registrations;
/// - the data directory cannot be resolved;
/// - the launcher path cannot be determined;
/// - the file already exists with identical contents.
///
/// The function always tries to remove the entry first when running under
/// AppImage so a previously-installed broken entry does not linger.
pub fn install_desktop_file(prefs: &UrlSchemePrefs) {
    use super::paths::data_home;

    if crate::app_updater::appimage_path().is_some() {
        if let Some(data_home) = data_home() {
            let _ = std::fs::remove_file(data_home.join("applications").join("remotrix.desktop"));
        }
        return;
    }
    let schemes = enabled_schemes(prefs);
    let Some(data_home) = data_home() else { return };
    let path = data_home.join("applications").join("remotrix.desktop");
    if schemes.is_empty() {
        let _ = std::fs::remove_file(&path);
        #[cfg(target_os = "linux")]
        unregister_default_url_handlers(&[]);
        return;
    }
    let dir = path.parent().unwrap_or(&data_home);
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let Some(exe) = app_launch_exe() else {
        return;
    };
    let content = format!(
        "{}StartupWMClass=remotrix\n",
        desktop_entry_header(
            &format!("\"{}\" %u", escape_exec(&exe.display().to_string())),
            &schemes,
        )
    );
    if path.exists() {
        if let Ok(existing) = std::fs::read_to_string(&path) {
            if existing == content {
                #[cfg(target_os = "linux")]
                set_default_url_handlers(&schemes);
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
    #[cfg(target_os = "linux")]
    set_default_url_handlers(&schemes);
}

#[cfg(target_os = "linux")]
fn set_default_url_handlers(schemes: &[&str]) {
    for scheme in schemes {
        let status = std::process::Command::new("xdg-mime")
            .arg("default")
            .arg("remotrix.desktop")
            .arg(format!("x-scheme-handler/{scheme}"))
            .status();
        match status {
            Ok(s) if !s.success() => {
                tracing::warn!(scheme, "xdg-mime default returned non-zero");
            }
            Err(e) => {
                tracing::warn!(scheme, error = %e, "xdg-mime unavailable");
            }
            _ => {}
        }
    }
}

#[cfg(target_os = "linux")]
fn unregister_default_url_handlers(schemes: &[&str]) {
    for scheme in schemes {
        let _ = std::process::Command::new("xdg-mime")
            .arg("default")
            .arg("")
            .arg(format!("x-scheme-handler/{scheme}"))
            .status();
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
    #[cfg(not(target_os = "windows"))]
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
/// `Exec=` line and the URL scheme handlers to declare. The `MimeType=` line
/// is omitted when `schemes` is empty; otherwise it lists every scheme as
/// `x-scheme-handler/<scheme>;`. Kept separate so the format is unit-testable
/// without touching the filesystem.
pub(crate) fn desktop_entry_header(exec: &str, schemes: &[&str]) -> String {
    let mime_line = if schemes.is_empty() {
        String::new()
    } else {
        let joined: Vec<String> = schemes
            .iter()
            .map(|s| format!("x-scheme-handler/{s};"))
            .collect();
        format!("MimeType={};\n", joined.join(""))
    };
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Remotrix\n\
         Comment=Download manager\n\
         Exec={exec}\n\
         Icon=remotrix\n\
         Terminal=false\n\
         Categories=Network;FileTransfer;\n\
         {mime_line}"
    )
}

/// Escape backslashes and double quotes for safe inclusion in a `.desktop`
/// `Exec=` line. The caller is responsible for wrapping the result in
/// surrounding quotes before passing it to [`desktop_entry_header`].
pub(crate) fn escape_exec(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Apply the URL-scheme registration to the host OS based on `prefs`. Called
/// from `apply_settings` so the user's selection takes effect immediately
/// without a restart. All failures are `warn!`-logged; this function never
/// panics or returns errors — settings apply must not be blocked by a
/// missing registry or `xdg-mime`.
///
/// Platform behaviour:
/// - **Linux**: rewrites (or removes) `~/.local/share/applications/remotrix.desktop`
///   with the enabled scheme set in `MimeType=`, then re-runs `xdg-mime default`.
/// - **Windows**: writes (or deletes) `HKCU\Software\Classes\<scheme>` for each
///   enabled scheme with the standard `URL:<scheme>` / `shell\open\command`
///   layout. Disabled schemes whose root key is owned by us (default value
///   starts with `URL:`) are removed.
pub fn apply_url_scheme_registration(prefs: &UrlSchemePrefs) {
    install_desktop_file(prefs);
    #[cfg(target_os = "windows")]
    sync_windows_scheme_registry(prefs);
}

/// Start-up entry point for the `remotrix://` scheme on Windows. Writes the
/// HKCU registration only when the user has enabled `register_remotrix`; a
/// no-op otherwise. This keeps the launcher self-healing on every launch
/// (idempotent rewrite) without overriding the user's choice.
#[cfg(target_os = "windows")]
pub fn install_url_scheme(prefs: &UrlSchemePrefs) {
    if !prefs.register_remotrix {
        return;
    }
    write_windows_scheme("remotrix", "URL:Remotrix");
}

#[cfg(target_os = "windows")]
fn sync_windows_scheme_registry(prefs: &UrlSchemePrefs) {
    let schemes: [(&str, &str, bool); 3] = [
        ("remotrix", "URL:Remotrix", prefs.register_remotrix),
        ("magnet", "URL:Magnet", prefs.register_magnet),
        ("ed2k", "URL:ED2K", prefs.register_ed2k),
    ];
    for (scheme, url_value, enabled) in schemes {
        if enabled {
            write_windows_scheme(scheme, url_value);
        } else {
            remove_windows_scheme(scheme, url_value);
        }
    }
}

#[cfg(target_os = "windows")]
fn write_windows_scheme(scheme: &str, url_value: &str) {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
        REG_CREATE_KEY_DISPOSITION, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    let Some(exe) = app_launch_exe() else { return };
    let command = format!("\"{}\" \"%1\"", exe.display());
    let default_payload = url_value
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>();
    let default_bytes = unsafe {
        std::slice::from_raw_parts(
            default_payload.as_ptr() as *const u8,
            default_payload.len() * 2,
        )
    };
    let command_payload = command
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>();
    let command_bytes = unsafe {
        std::slice::from_raw_parts(
            command_payload.as_ptr() as *const u8,
            command_payload.len() * 2,
        )
    };

    let root_subkey = format!("Software\\Classes\\{scheme}");
    let mut key = HKEY::default();
    let mut disposition = REG_CREATE_KEY_DISPOSITION(0);
    let create_status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(root_subkey.as_str()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            Some(&mut disposition),
        )
    };
    if create_status != ERROR_SUCCESS {
        tracing::warn!(scheme, status = ?create_status, "failed to open HKCU scheme root");
        return;
    }
    let set_default =
        unsafe { RegSetValueExW(key, &HSTRING::from(""), None, REG_SZ, Some(default_bytes)) };
    if set_default != ERROR_SUCCESS {
        tracing::warn!(scheme, status = ?set_default, "failed to set HKCU scheme default value");
    }
    unsafe {
        let _ = RegCloseKey(key);
    }

    let cmd_subkey = format!("Software\\Classes\\{scheme}\\shell\\open\\command");
    let mut cmd_key = HKEY::default();
    let mut cmd_disp = REG_CREATE_KEY_DISPOSITION(0);
    let cmd_create = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(cmd_subkey.as_str()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut cmd_key,
            Some(&mut cmd_disp),
        )
    };
    if cmd_create != ERROR_SUCCESS {
        tracing::warn!(scheme, status = ?cmd_create, "failed to open HKCU command subkey");
        return;
    }
    let set_cmd = unsafe {
        RegSetValueExW(
            cmd_key,
            &HSTRING::from(""),
            None,
            REG_SZ,
            Some(command_bytes),
        )
    };
    if set_cmd != ERROR_SUCCESS {
        tracing::warn!(scheme, status = ?set_cmd, "failed to set HKCU command default value");
    }
    unsafe {
        let _ = RegCloseKey(cmd_key);
    }
}

#[cfg(target_os = "windows")]
fn remove_windows_scheme(scheme: &str, url_value: &str) {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteKeyW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_READ, KEY_WRITE,
    };

    let root_subkey = format!("Software\\Classes\\{scheme}");
    let mut key = HKEY::default();
    let open_status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(root_subkey.as_str()),
            None,
            KEY_READ | KEY_WRITE,
            &mut key,
        )
    };
    if open_status != ERROR_SUCCESS {
        return;
    }

    let mut buf = [0u16; 64];
    let mut buf_len = (buf.len() * 2) as u32;
    let query_status = unsafe {
        RegQueryValueExW(
            key,
            &HSTRING::from(""),
            None,
            None,
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut buf_len),
        )
    };
    let owned = if query_status == ERROR_SUCCESS {
        let len = (buf_len as usize) / 2;
        let s = String::from_utf16_lossy(&buf[..len.saturating_sub(1)]);
        s == url_value
    } else {
        false
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    if !owned {
        return;
    }

    let cmd_subkey = format!("Software\\Classes\\{scheme}\\shell\\open\\command");
    let cmd_status =
        unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(cmd_subkey.as_str())) };
    if cmd_status != ERROR_SUCCESS {
        tracing::warn!(scheme, status = ?cmd_status, "failed to delete HKCU command subkey");
    }
    let shell_subkey = format!("Software\\Classes\\{scheme}\\shell\\open");
    let shell_status =
        unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(shell_subkey.as_str())) };
    if shell_status != ERROR_SUCCESS {
        tracing::warn!(scheme, status = ?shell_status, "failed to delete HKCU open subkey");
    }
    let shell_root = format!("Software\\Classes\\{scheme}\\shell");
    let shell_root_status =
        unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(shell_root.as_str())) };
    if shell_root_status != ERROR_SUCCESS {
        tracing::warn!(scheme, status = ?shell_root_status, "failed to delete HKCU shell subkey");
    }
    let root_status =
        unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(root_subkey.as_str())) };
    if root_status != ERROR_SUCCESS {
        tracing::warn!(scheme, status = ?root_status, "failed to delete HKCU scheme root");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::UrlSchemePrefs;

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
        let h = desktop_entry_header("\"/usr/bin/remotrix\"", &["remotrix"]);
        assert!(h.starts_with("[Desktop Entry]\n"));
        assert!(h.contains("Type=Application"));
        assert!(h.contains("Name=Remotrix"));
        assert!(h.contains("Exec=\"/usr/bin/remotrix\""));
        assert!(h.contains("Icon=remotrix"));
        assert!(h.contains("Terminal=false"));
        assert!(h.contains("Categories=Network;FileTransfer;"));
        assert!(h.contains("MimeType=x-scheme-handler/remotrix;"));
    }

    #[test]
    fn desktop_entry_header_preserves_appimage_exec() {
        let exec = "\"$APPIMAGE\" --no-sandbox";
        let h = desktop_entry_header(exec, &["remotrix"]);
        assert!(h.contains("Exec=\"$APPIMAGE\" --no-sandbox"));
    }

    #[test]
    fn desktop_entry_header_no_schemes() {
        let h = desktop_entry_header("\"/usr/bin/remotrix\"", &[]);
        assert!(!h.contains("MimeType="));
    }

    #[test]
    fn desktop_entry_header_multiple_schemes() {
        let h = desktop_entry_header("\"/usr/bin/remotrix\"", &["remotrix", "magnet", "ed2k"]);
        assert!(h.contains("x-scheme-handler/remotrix;"));
        assert!(h.contains("x-scheme-handler/magnet;"));
        assert!(h.contains("x-scheme-handler/ed2k;"));
        assert!(h.contains(
            "MimeType=x-scheme-handler/remotrix;x-scheme-handler/magnet;x-scheme-handler/ed2k;"
        ));
    }

    #[test]
    fn enabled_schemes_default_all_off() {
        let prefs = UrlSchemePrefs::default();
        assert!(enabled_schemes(&prefs).is_empty());
    }

    #[test]
    fn enabled_schemes_preserves_order() {
        let prefs = UrlSchemePrefs {
            register_remotrix: true,
            register_magnet: true,
            register_ed2k: true,
        };
        assert_eq!(enabled_schemes(&prefs), vec!["remotrix", "magnet", "ed2k"]);
    }
}
