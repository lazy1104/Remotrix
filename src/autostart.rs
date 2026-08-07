pub fn set_enabled(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        linux_set_enabled(enabled)
    }
    #[cfg(target_os = "windows")]
    {
        windows_set_enabled(enabled)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = enabled;
        Ok(())
    }
}

pub fn is_autostart_launch() -> bool {
    std::env::var_os("REMOTRIX_AUTOSTART").is_some() || std::env::args().any(|a| a == "--autostart")
}

#[cfg(target_os = "linux")]
fn autostart_dir() -> Option<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Some(std::path::PathBuf::from(dir).join("autostart"));
        }
    }
    directories::BaseDirs::new().map(|b| b.config_dir().join("autostart"))
}

#[cfg(target_os = "linux")]
fn autostart_content() -> Option<String> {
    let exe = crate::config::app_launch_exe()?;
    Some(format!(
        "{}X-GNOME-Autostart-enabled=true\n",
        crate::config::desktop_entry_header(&format!(
            "env REMOTRIX_AUTOSTART=1 \"{}\"",
            crate::config::escape_exec(&exe.display().to_string())
        ))
    ))
}

#[cfg(target_os = "linux")]
fn linux_set_enabled(enabled: bool) -> Result<(), String> {
    let Some(dir) = autostart_dir() else {
        return Ok(());
    };
    let path = dir.join("remotrix.desktop");
    if !enabled {
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("failed to remove autostart file: {e}")),
        }
    } else {
        let Some(content) = autostart_content() else {
            return Err("failed to resolve current executable".into());
        };
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("failed to create autostart dir: {e}"))?;
        let tmp = path.with_extension("desktop.tmp");
        std::fs::write(&tmp, content)
            .map_err(|e| format!("failed to write autostart file: {e}"))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("failed to install autostart file: {e}"))
    }
}

#[cfg(target_os = "windows")]
fn windows_set_enabled(enabled: bool) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_SET_VALUE, REG_CREATE_KEY_DISPOSITION, REG_OPTION_NON_VOLATILE,
        REG_SZ,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "Remotrix";

    if !enabled {
        let mut key = HKEY::default();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN_KEY),
                None,
                KEY_SET_VALUE,
                &mut key,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        if status != ERROR_SUCCESS {
            return Err(format!("failed to open Run key: {status:?}"));
        }
        let status = unsafe { RegDeleteValueW(key, &HSTRING::from(VALUE_NAME)) };
        unsafe {
            let _ = RegCloseKey(key);
        };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("failed to delete Run value: {status:?}"))
        }
    } else {
        let exe = std::env::current_exe()
            .map_err(|e| format!("failed to resolve current executable: {e}"))?;
        let data = format!("\"{}\" --autostart", exe.display());
        let mut data_utf16: Vec<u16> = data.encode_utf16().collect();
        data_utf16.push(0);
        let data_bytes = unsafe {
            std::slice::from_raw_parts(data_utf16.as_ptr() as *const u8, data_utf16.len() * 2)
        };
        let mut key = HKEY::default();
        let mut disposition = REG_CREATE_KEY_DISPOSITION(0);
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN_KEY),
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                Some(&mut disposition),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("failed to create Run key: {status:?}"));
        }
        let status = unsafe {
            RegSetValueExW(
                key,
                &HSTRING::from(VALUE_NAME),
                None,
                REG_SZ,
                Some(data_bytes),
            )
        };
        unsafe {
            let _ = RegCloseKey(key);
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("failed to write Run value: {status:?}"))
        }
    }
}
