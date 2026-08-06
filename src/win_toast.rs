use std::mem::ManuallyDrop;
use std::path::PathBuf;

use windows::core::{Interface, GUID, HSTRING, PWSTR};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile,
    StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0},
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::{
    FOLDERID_Programs, IShellLinkW, PropertiesSystem::IPropertyStore, SHGetKnownFolderPath,
    SetCurrentProcessExplicitAppUserModelID, ShellLink, KF_FLAG_DEFAULT,
};

pub const AUMID: &str = "org.remotrix.app.Remotrix";

const PKEY_APPUSERMODEL_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};

pub fn init() {
    set_process_aumid();
    let _ = std::thread::Builder::new()
        .name("remotrix-win-toast".into())
        .spawn(|| {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(ensure_shortcut));
        });
}

fn set_process_aumid() {
    let aumid = HSTRING::from(AUMID);
    let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(&aumid) };
}

fn start_menu_programs_dir() -> Option<PathBuf> {
    unsafe {
        let pw = SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None).ok()?;
        let wide = pw.as_wide();
        let path = PathBuf::from(String::from_utf16_lossy(wide));
        CoTaskMemFree(Some(pw.0 as *const core::ffi::c_void));
        Some(path)
    }
}

// Public so notifications can re-register the AUMID if the user
// deletes the start-menu shortcut while the app is running (self-heal).
pub fn ensure_shortcut() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(programs_dir) = start_menu_programs_dir() else {
        return false;
    };
    let shortcut_path = programs_dir.join("Remotrix").join("Remotrix.lnk");
    if shortcut_aumid(&shortcut_path).as_deref() == Some(AUMID) {
        return true;
    }

    if let Err(e) = std::fs::create_dir_all(shortcut_path.parent().unwrap_or(&programs_dir)) {
        tracing::debug!(error = %e, "win_toast: failed to create shortcut directory");
        return false;
    }

    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if !hr.is_ok() {
        tracing::debug!(error = %hr, "win_toast: CoInitializeEx failed");
        return false;
    }
    let result = build_shortcut(&exe, &shortcut_path);
    if hr.0 == 0 {
        unsafe { CoUninitialize() };
    }
    result
}

fn shortcut_aumid(path: &std::path::Path) -> Option<String> {
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if !hr.is_ok() {
        return None;
    }
    let result = (|| -> Option<String> {
        let shell: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.ok()?;
        let persist: IPersistFile = shell.cast().ok()?;
        let path_h = HSTRING::from(path.to_string_lossy().as_ref());
        unsafe { persist.Load(&path_h, STGM_READ) }.ok()?;
        let store: IPropertyStore = shell.cast().ok()?;
        let propvar = unsafe { store.GetValue(&PKEY_APPUSERMODEL_ID) }.ok()?;
        if propvar.Anonymous.Anonymous.vt != VT_LPWSTR {
            return None;
        }
        let pwsz = propvar.Anonymous.Anonymous.Anonymous.pwszVal;
        Some(unsafe { HSTRING::from_ptr(pwsz.0) }.to_string())
    })();
    if hr.0 == 0 {
        unsafe { CoUninitialize() };
    }
    result
}

fn build_shortcut(exe: &std::path::Path, shortcut_path: &std::path::Path) -> bool {
    let run = || -> Result<(), String> {
        let shell: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
                .map_err(|e| format!("CoCreateInstance: {e:?}"))?;

        let exe_h = HSTRING::from(exe.to_string_lossy().as_ref());
        unsafe { shell.SetPath(&exe_h) }.map_err(|e| format!("SetPath: {e:?}"))?;
        unsafe { shell.SetIconLocation(&exe_h, 0) }
            .map_err(|e| format!("SetIconLocation: {e:?}"))?;

        let store: IPropertyStore = shell
            .cast()
            .map_err(|e| format!("IPropertyStore cast: {e:?}"))?;
        let wide: Vec<u16> = AUMID.encode_utf16().chain(std::iter::once(0)).collect();
        let propvar = PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_LPWSTR,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: PROPVARIANT_0_0_0 {
                        pwszVal: PWSTR(wide.as_ptr() as *mut u16),
                    },
                }),
            },
        };
        unsafe { store.SetValue(&PKEY_APPUSERMODEL_ID, &propvar) }
            .map_err(|e| format!("SetValue: {e:?}"))?;
        unsafe { store.Commit() }.map_err(|e| format!("Commit: {e:?}"))?;

        let persist: IPersistFile = shell.cast().map_err(|e| format!("cast: {e:?}"))?;
        let path_h = HSTRING::from(shortcut_path.to_string_lossy().as_ref());
        unsafe { persist.Save(&path_h, true) }.map_err(|e| format!("Save: {e:?}"))?;
        Ok(())
    };

    match run() {
        Ok(()) => true,
        Err(e) => {
            tracing::debug!(error = %e, "win_toast: failed to create start menu shortcut");
            false
        }
    }
}
