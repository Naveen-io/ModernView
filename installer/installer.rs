#![windows_subsystem = "windows"]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const MODERNVIEW_EXE: &[u8] = include_bytes!("../target/release/modernview.exe");

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(
        hwnd: isize,
        text: *const u16,
        caption: *const u16,
        utype: u32,
    ) -> i32;
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn show_message(text: &str, title: &str, icon_info: bool) {
    let wtext = to_wide(text);
    let wtitle = to_wide(title);
    let utype = if icon_info { 0x00000040 } else { 0x00000010 }; // MB_ICONINFORMATION : MB_ICONERROR
    unsafe {
        MessageBoxW(0, wtext.as_ptr(), wtitle.as_ptr(), utype);
    }
}

fn main() {
    let local_app_data = match std::env::var("LOCALAPPDATA") {
        Ok(v) => PathBuf::from(v),
        Err(_) => {
            show_message("Could not determine LOCALAPPDATA directory.", "ModernView Setup Error", false);
            return;
        }
    };

    let install_dir = local_app_data.join("Programs").join("ModernView");
    if let Err(e) = fs::create_dir_all(&install_dir) {
        show_message(&format!("Failed to create install directory: {}", e), "ModernView Setup Error", false);
        return;
    }

    let target_exe = install_dir.join("modernview.exe");

    // Write executable
    if let Err(e) = fs::write(&target_exe, MODERNVIEW_EXE) {
        show_message(&format!("Failed to write modernview.exe: {}\nEnsure ModernView is not currently running.", e), "ModernView Setup Error", false);
        return;
    }

    // Register file associations and shell integration
    let _ = Command::new(&target_exe).arg("--register").output();

    // Create Start Menu and Desktop shortcuts via PowerShell
    let ps_cmd = format!(
        r#"$wsh = New-Object -ComObject WScript.Shell; $desk = [Environment]::GetFolderPath('Desktop'); $start = [Environment]::GetFolderPath('Programs'); $s1 = $wsh.CreateShortcut("$desk\ModernView.lnk"); $s1.TargetPath = '{0}'; $s1.WorkingDirectory = '{1}'; $s1.Description = 'ModernView Image Viewer'; $s1.Save(); $s2 = $wsh.CreateShortcut("$start\ModernView.lnk"); $s2.TargetPath = '{0}'; $s2.WorkingDirectory = '{1}'; $s2.Description = 'ModernView Image Viewer'; $s2.Save();"#,
        target_exe.to_string_lossy(),
        install_dir.to_string_lossy()
    );

    let _ = Command::new("powershell")
        .args(&["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_cmd])
        .output();

    // Launch ModernView
    let _ = Command::new(&target_exe).spawn();

    show_message(
        "ModernView has been successfully installed!\n\n• Installed to: %LOCALAPPDATA%\\Programs\\ModernView\n• Desktop & Start Menu shortcuts created\n• Registered as Windows Default Photo Viewer\n• Added 'Open with ModernView' to right-click context menu\n\nLaunching ModernView now...",
        "ModernView Setup",
        true,
    );
}
