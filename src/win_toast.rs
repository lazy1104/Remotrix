use std::mem::ManuallyDrop;
use std::path::PathBuf;

use windows::core::{Interface, GUID, HSTRING, PWSTR};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile,
    StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0},
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
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
    ensure_shortcut();
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
    let shortcut_path = programs_dir.join("Remotrix.lnk");

    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let result = build_shortcut(&exe, &shortcut_path);
    unsafe { CoUninitialize() };
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

        let persist: IPersistFile = shell.cast().map_err(|e| format!("cast: {e:?}"))?;
        let path_h = HSTRING::from(shortcut_path.to_string_lossy().as_ref());
        unsafe { persist.Save(&path_h, true) }.map_err(|e| format!("Save: {e:?}"))?;

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
