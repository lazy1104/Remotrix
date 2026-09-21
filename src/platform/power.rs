use std::process::Child;
use std::sync::Mutex;

#[cfg(not(target_os = "windows"))]
static BLOCK_CHILD: Mutex<Option<Child>> = Mutex::new(None);

pub fn set_sleep_blocked(blocked: bool) {
    if blocked {
        block();
    } else {
        release();
    }
}

pub fn release() {
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(mut guard) = BLOCK_CHILD.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        set_thread_execution_state(windows::Win32::System::Power::ES_CONTINUOUS);
    }
}

#[cfg(target_os = "windows")]
fn block() {
    set_thread_execution_state(
        windows::Win32::System::Power::ES_CONTINUOUS
            | windows::Win32::System::Power::ES_SYSTEM_REQUIRED,
    );
}

#[cfg(target_os = "windows")]
fn set_thread_execution_state(new_state: windows::Win32::System::Power::EXECUTION_STATE) {
    let _ = unsafe { windows::Win32::System::Power::SetThreadExecutionState(new_state) };
}

#[cfg(target_os = "macos")]
fn block() {
    use std::process::Command;
    if let Ok(mut guard) = BLOCK_CHILD.lock() {
        if guard.is_some() {
            return;
        }
        match Command::new("caffeinate").arg("-i").spawn() {
            Ok(child) => *guard = Some(child),
            Err(e) => tracing::warn!(error = %e, "failed to spawn caffeinate"),
        }
    }
}

#[cfg(target_os = "linux")]
fn block() {
    use std::process::Command;
    if let Ok(mut guard) = BLOCK_CHILD.lock() {
        if guard.is_some() {
            return;
        }
        match Command::new("systemd-inhibit")
            .args([
                "--what=sleep",
                "--mode=block",
                "--who=remotrix",
                "--why=Downloading",
                "sleep",
                "infinity",
            ])
            .spawn()
        {
            Ok(child) => *guard = Some(child),
            Err(e) => {
                tracing::warn!(error = %e, "failed to spawn systemd-inhibit; sleep blocking unavailable")
            }
        }
    }
}
