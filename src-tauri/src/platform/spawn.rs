use std::process::Command;

/// On Windows, child processes spawned from a GUI application (`windows_subsystem
/// = "windows"`) inherit the console-subsystem flag, which causes a console window
/// to briefly flash for every spawned process. This is extremely distracting when
/// yt-dlp / node / ffmpeg / reg are invoked frequently.
///
/// Calling this after `Command::new(...)` sets the `CREATE_NO_WINDOW` flag
/// (`0x0800_0000`) so the child runs without allocating a console.
#[cfg(windows)]
pub fn hide_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

/// No-op on non-Windows platforms where console windows are not an issue.
#[cfg(not(windows))]
pub fn hide_console_window(_command: &mut Command) {}
