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
    let local_app_data = env::var("LOCALAPPDATA").unwrap_or_default();
    let mut install_dir = PathBuf::from(&local_app_data);
    install_dir.push("SwiftRun");

    let mut exe_path = install_dir.clone();
    exe_path.push("swift_run.exe");

    // 1. Try to find the actual exe path from registry (it might be different)
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
                let full_str =
                    String::from_utf16_lossy(&data[..(size as usize / 2).saturating_sub(1)]);
                // Strip arguments (e.g. "C:\path\to\exe" --background)
                let actual_path = if let Some(idx) = full_str.find(".exe") {
                    &full_str[..idx + 4]
                } else {
                    &full_str
                };
                exe_path = PathBuf::from(actual_path.trim_matches('"'));
            }
            let _ = RegCloseKey(h_key);
        }
    }

    // 2. Kill the app before extraction or cleanup
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "swift_run.exe", "/T"])
        .output();

    // 3. Extract uninstaller helper to temp
    let temp_exe = env::temp_dir().join("swift_run_uninstall_helper.exe");
    if let Err(_) = fs::write(&temp_exe, APP_BYTES) {
        // If we can't write to temp, try to use the existing one if it's there
        if !exe_path.exists() {
            return;
        }
    }

    // 4. Run the helper with --uninstall
    let target_runner = if temp_exe.exists() {
        &temp_exe
    } else {
        &exe_path
    };
    let _ = Command::new(target_runner).arg("--uninstall").status();

    // 5. Aggressive cleanup loop
    for _ in 0..20 {
        if exe_path.exists() {
            let _ = fs::remove_file(&exe_path);
        }
        if let Some(parent) = exe_path.parent() {
            if parent.exists() && parent.ends_with("SwiftRun") {
                let _ = fs::remove_dir_all(parent);
            }
        }

        // Also check the default path just in case
        if install_dir.exists() {
            let _ = fs::remove_dir_all(&install_dir);
        }

        if !exe_path.exists() && !install_dir.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // 6. Final cleanup of temp helper
    if temp_exe.exists() {
        let _ = fs::remove_file(&temp_exe);
    }
}
