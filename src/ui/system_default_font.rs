//! Resolve the OS default UI sans-serif family at boot when
//! `Settings.font_family` is empty.
//!
//! Each platform uses its native API for the authoritative answer
//! (fontconfig user prefs on Linux, the global `AppleSystemUIFont` key
//! on macOS, `SPI_GETNONCLIENTMETRICS.lfMessageFont` on Windows). On
//! Linux and macOS, any failure in the OS-native path falls back to
//! [`crate::ui::font_autopick::pick_default_family`], which enumerates
//! fontdb and sorts by locale-aware heuristics. Windows has no fallback
//! because the SPI value is the source of truth for system UI fonts.
//! When everything returns `None`, the boot path leaves
//! `effective_font_family` empty and lets `iced::Font::DEFAULT` resolve
//! through iced's own fontconfig/fontdb path.

#[cfg(target_os = "linux")]
fn native_query() -> Option<String> {
    use std::process::Command;
    let output = Command::new("fc-match")
        .args(["-f", "%{family}\n", "sans-serif"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let family = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if family.is_empty() {
                tracing::warn!("fc-match returned empty family; falling back to font_autopick");
                crate::ui::font_autopick::pick_default_family()
            } else {
                Some(family)
            }
        }
        Ok(o) => {
            tracing::warn!(
                status = %o.status,
                "fc-match exited non-zero; falling back to font_autopick"
            );
            crate::ui::font_autopick::pick_default_family()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!("fc-match not found on PATH; falling back to font_autopick");
            crate::ui::font_autopick::pick_default_family()
        }
        Err(e) => {
            tracing::warn!(error = %e, "fc-match failed; falling back to font_autopick");
            crate::ui::font_autopick::pick_default_family()
        }
    }
}

#[cfg(target_os = "macos")]
fn native_query() -> Option<String> {
    use std::process::Command;
    let output = Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleSystemUIFont"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let family = raw.trim_matches('"').trim().to_string();
            if family.is_empty() {
                tracing::warn!("AppleSystemUIFont empty; falling back to font_autopick");
                crate::ui::font_autopick::pick_default_family()
            } else {
                Some(family)
            }
        }
        Ok(o) => {
            tracing::warn!(
                status = %o.status,
                "defaults read AppleSystemUIFont failed; falling back to font_autopick"
            );
            crate::ui::font_autopick::pick_default_family()
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "defaults not available; falling back to font_autopick"
            );
            crate::ui::font_autopick::pick_default_family()
        }
    }
}

#[cfg(target_os = "windows")]
fn native_query() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let mut metrics = NONCLIENTMETRICSW::default();
    metrics.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
    let res = unsafe {
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            Some(&mut metrics as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    match res {
        Ok(()) => {
            let face = String::from_utf16_lossy(&metrics.lfMessageFont.lfFaceName);
            let trimmed = face.trim_end_matches('\0').trim().to_string();
            if trimmed.is_empty() {
                tracing::warn!("SPI_GETNONCLIENTMETRICS returned empty face name");
                None
            } else {
                Some(trimmed)
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "SPI_GETNONCLIENTMETRICS failed");
            None
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn native_query() -> Option<String> {
    crate::ui::font_autopick::pick_default_family()
}

pub fn query() -> Option<String> {
    native_query()
}
