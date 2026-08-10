use std::time::Instant;

#[derive(Default)]
pub struct ShutdownControl {
    pub card_open: bool,
    pub after_complete: bool,
    pub timer_enabled: bool,
    pub timer_minutes: u32,
    pub timer_deadline: Option<Instant>,
}

pub fn shutdown_command() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        vec!["systemctl".to_string(), "poweroff".to_string()]
    }
    #[cfg(target_os = "windows")]
    {
        vec![
            "shutdown".to_string(),
            "/s".to_string(),
            "/t".to_string(),
            "0".to_string(),
        ]
    }
    #[cfg(target_os = "macos")]
    {
        vec!["shutdown".to_string(), "-h".to_string(), "now".to_string()]
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Vec::new()
    }
}

pub fn shutdown_system_blocking() -> Result<(), String> {
    let cmd = shutdown_command();
    if cmd.is_empty() {
        return Err("no shutdown command for this platform".to_string());
    }
    let (program, args) = cmd.split_first().expect("non-empty command");
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn {:?}: {e}", program))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(format!("shutdown command exited with {}", output.status))
        } else {
            Err(stderr)
        }
    }
}
