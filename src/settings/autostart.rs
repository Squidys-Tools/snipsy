const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

fn reg(args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg")
        .args(args)
        .creation_flags(0x0800_0000)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub(super) fn launch_at_login() -> bool {
    reg(&["query", RUN_KEY, "/v", "Snipsy"])
}

pub(super) fn set_launch_at_login(on: bool) {
    if on {
        if let Ok(exe) = std::env::current_exe() {
            let v = format!("\"{}\"", exe.display());
            reg(&["add", RUN_KEY, "/v", "Snipsy", "/t", "REG_SZ", "/d", &v, "/f"]);
        }
    } else {
        reg(&["delete", RUN_KEY, "/v", "Snipsy", "/f"]);
    }
}
