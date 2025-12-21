#![windows_subsystem = "windows"]
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use windows::Win32::System::Registry::*;
use windows::core::*;

// Embed the main binary to be standalone and show correct dialogs even if app is missing
const APP_BYTES: &[u8] = include_bytes!("../target/release/swift_run.exe");

fn main() {
    let mut install_path: Option<PathBuf> = None;

    // 1. First, try to find where the app IS installed (for cleanup later)
    unsafe {
        let run_key_path = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let mut h_key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            run_key_path,
            Some(0),
            KEY_QUERY_VALUE,
            &mut h_key,
        )
        .is_ok()
        {
            let mut data = [0u16; 512];
            let mut size = (data.len() * 2) as u32;
            if RegQueryValueExW(
                h_key,
                w!("SwiftRun"),
                None,
                None,
                Some(data.as_mut_ptr() as _),
                Some(&mut size),
            )
            .is_ok()
            {
                let path_str =
                    String::from_utf16_lossy(&data[..(size as usize / 2).saturating_sub(1)]);
                install_path = Some(PathBuf::from(path_str));
            }
            let _ = RegCloseKey(h_key);
        }
    }

    // 2. Extract embedded app to target\release (or temp)
    let temp_dir = env::temp_dir();
    let temp_exe = temp_dir.join("swift_run_uninstall_helper.exe");
    if let Err(e) = fs::write(&temp_exe, APP_BYTES) {
        // Fallback: If we can't write to temp, try to use the existing path if it exists
        if install_path.is_none() || !install_path.as_ref().unwrap().exists() {
            eprintln!("Failed to extract uninstaller helper: {:?}", e);
            return;
        }
    }

    // 3. Run the extracted app with --uninstall
    // This will perform registry cleanup, restart explorer, and show the CUSTOM FLUENT DIALOG.
    let target_exe = if temp_exe.exists() {
        &temp_exe
    } else {
        install_path.as_ref().unwrap()
    };
    let status = Command::new(target_exe).arg("--uninstall").status();

    // 4. Cleanup the installation folder
    if let Ok(s) = status {
        if s.success() {
            if let Some(path) = install_path {
                if path.exists() {
                    let _ = fs::remove_file(&path);
                    if let Some(parent) = path.parent() {
                        if parent.ends_with("SwiftRun") {
                            let _ = fs::remove_dir_all(parent);
                        }
                    }
                }
            }
        }
    }

    // 5. Cleanup the temp exe
    if temp_exe.exists() {
        let _ = fs::remove_file(&temp_exe);
    }
}
