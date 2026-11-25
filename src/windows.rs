use std::path::PathBuf;
use std::process::Command;

pub fn parse_cli_image_path() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--register" || arg == "--set-default" {
            let _ = register_windows_integration();
            continue;
        }
        if !arg.starts_with('-') {
            let p = PathBuf::from(&arg);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

/// Generates the complete Windows Registry registration script for ModernView.
/// Configures:
/// 1. HKCU Applications registration with SupportedTypes (Open With menu)
/// 2. ProgID ModernView.Image with icon and open command
/// 3. Direct Explorer right-click context menu via SystemFileAssociations\image
/// 4. File extension associations (.jpg, .jpeg, .png, .webp, .bmp, .gif, .tiff, .tif, .ico)
/// 5. Sets ModernView as Default Handler for each format in HKCU\Software\Classes
/// 6. Windows 10/11 RegisteredApplications capability (Settings > Default Apps)
pub fn generate_file_associations_reg_script() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_escaped = exe_path.to_string_lossy().replace('\\', "\\\\");

    let reg_content = format!(
        r#"Windows Registry Editor Version 5.00

; ====================================================================
; ModernView Windows Shell & Default App Registration
; Current User Registration (No Administrator Privileges Required)
; ====================================================================

; 1. ModernView Application Handler (for Windows Open With Submenu)
[HKEY_CURRENT_USER\Software\Classes\Applications\modernview.exe]
@="ModernView"
"FriendlyAppName"="ModernView"

[HKEY_CURRENT_USER\Software\Classes\Applications\modernview.exe\DefaultIcon]
@="\"{exe}\",0"

[HKEY_CURRENT_USER\Software\Classes\Applications\modernview.exe\shell\open]
@="Open with ModernView"
"FriendlyAppName"="ModernView"

[HKEY_CURRENT_USER\Software\Classes\Applications\modernview.exe\shell\open\command]
@="\"{exe}\" \"%1\""

[HKEY_CURRENT_USER\Software\Classes\Applications\modernview.exe\SupportedTypes]
".jpg"=""
".jpeg"=""
".png"=""
".webp"=""
".bmp"=""
".gif"=""
".tiff"=""
".tif"=""
".ico"=""
".avif"=""
".heic"=""
".heif"=""
".qoi"=""
".tga"=""
".hdr"=""

; 2. ProgID Registration
[HKEY_CURRENT_USER\Software\Classes\ModernView.Image]
@="ModernView Image Document"
"FriendlyTypeName"="ModernView Image"

[HKEY_CURRENT_USER\Software\Classes\ModernView.Image\DefaultIcon]
@="\"{exe}\",0"

[HKEY_CURRENT_USER\Software\Classes\ModernView.Image\shell\open]
@="Open with ModernView"

[HKEY_CURRENT_USER\Software\Classes\ModernView.Image\shell\open\command]
@="\"{exe}\" \"%1\""

; 3. Windows Explorer Context Menu for Images (SystemFileAssociations)
[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\image\shell\ModernView]
@="Open with ModernView"
"Icon"="\"{exe}\",0"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\image\shell\ModernView\command]
@="\"{exe}\" \"%1\""

; 4. File Extension Default Association & OpenWith Registration
[HKEY_CURRENT_USER\Software\Classes\.jpg]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.jpg\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.jpeg]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.jpeg\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.png]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.png\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.webp]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.webp\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.bmp]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.bmp\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.gif]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.gif\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.tiff]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.tiff\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.tif]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.tif\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.ico]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.ico\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.avif]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.avif\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.heic]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.heic\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.heif]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.heif\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.qoi]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.qoi\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.tga]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.tga\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.hdr]
@="ModernView.Image"

[HKEY_CURRENT_USER\Software\Classes\.hdr\OpenWithProgids]
"ModernView.Image"=""

; 5. Windows Default Apps / RegisteredApplications Integration
[HKEY_CURRENT_USER\Software\RegisteredApplications]
"ModernView"="Software\\ModernView\\Capabilities"

[HKEY_CURRENT_USER\Software\ModernView\Capabilities]
"ApplicationName"="ModernView"
"ApplicationDescription"="Fast, Modern Image Viewer & Editor for Windows"

[HKEY_CURRENT_USER\Software\ModernView\Capabilities\FileAssociations]
".jpg"="ModernView.Image"
".jpeg"="ModernView.Image"
".png"="ModernView.Image"
".webp"="ModernView.Image"
".bmp"="ModernView.Image"
".gif"="ModernView.Image"
".tiff"="ModernView.Image"
".tif"="ModernView.Image"
".ico"="ModernView.Image"
".avif"="ModernView.Image"
".heic"="ModernView.Image"
".heif"="ModernView.Image"
".qoi"="ModernView.Image"
".tga"="ModernView.Image"
".hdr"="ModernView.Image"
"#,
        exe = exe_escaped
    );

    let temp_dir = std::env::temp_dir();
    let reg_path = temp_dir.join("modernview_associations.reg");
    std::fs::write(&reg_path, reg_content).map_err(|e| e.to_string())?;
    Ok(reg_path)
}

/// Automatically registers ModernView into Windows Registry and notifies the Windows Shell.
pub fn register_windows_integration() -> Result<String, String> {
    let reg_path = generate_file_associations_reg_script()?;
    
    // Run reg.exe import <file>
    let status = Command::new("reg")
        .args(&["import", &reg_path.to_string_lossy()])
        .status()
        .map_err(|e| format!("Failed to execute reg.exe: {}", e))?;

    if !status.success() {
        return Err("reg.exe returned a non-zero exit code while importing associations.".into());
    }

    // Notify Windows Explorer shell of file association change (SHCNE_ASSOCCHANGED = 0x08000000)
    let _ = Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-Command",
            r#"$code = '[DllImport("shell32.dll")] public static extern void SHChangeNotify(int id, int flags, IntPtr p1, IntPtr p2);'; $t = Add-Type -MemberDefinition $code -Name Win32Shell -PassThru; $t::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)"#,
        ])
        .output();

    Ok("ModernView registered as default viewer & context menu handler.".to_string())
}
