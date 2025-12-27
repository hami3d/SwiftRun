#![windows_subsystem = "windows"]
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

// This will embed the main executable into the installer.
// The user should build the main app in release mode first.
// If this fails to compile, ensure target\release\swift_run.exe exists.
const APP_BYTES: &[u8] = include_bytes!("../target/release/swift_run.exe");

fn main() {
    // Ensure the app is not running before we try to overwrite it
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "swift_run.exe", "/T"])
        .output();

    let local_app_data = env::var("LOCALAPPDATA").expect("Failed to get LOCALAPPDATA");
    let mut install_dir = PathBuf::from(local_app_data);
    install_dir.push("SwiftRun");

    if !install_dir.exists() {
        if let Err(e) = fs::create_dir_all(&install_dir) {
            eprintln!("Failed to create install directory: {:?}", e);
            return;
        }
    }

    let mut exe_path = install_dir.clone();
    exe_path.push("swift_run.exe");

    // Copy (extract) the embedded app to the install directory
    // We retry a few times in case the process is still releasing the file handle
    let mut written = false;
    for _ in 0..50 {
        if exe_path.exists() {
            let _ = fs::remove_file(&exe_path);
        }

        if let Err(_e) = fs::write(&exe_path, APP_BYTES) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        } else {
            written = true;
            break;
        }
    }

    if !written {
        eprintln!(
            "Failed to write swift_run.exe after multiple attempts. Please close the app manually."
        );
        return;
    }

    // Now run the newly extracted exe with --install to set registry hooks
    let status = Command::new(&exe_path).arg("--install").status();

    match status {
        Ok(s) if s.success() => {
            // Success! The app has performed installation and closed.
            // Now spawn the app for real and exit.
            // We launch via explorer.exe to ensure it runs as standard user even if installer is admin
            let _ = Command::new("explorer.exe").arg(&exe_path).spawn();
        }
        _ => {
            eprintln!("Installation failed or was cancelled.");
        }
    }
}
