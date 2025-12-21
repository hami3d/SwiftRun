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
    if let Err(e) = fs::write(&exe_path, APP_BYTES) {
        eprintln!("Failed to write swift_run.exe: {:?}", e);
        return;
    }

    // Now run the newly extracted exe with --install to set registry hooks
    let status = Command::new(&exe_path).arg("--install").status();

    match status {
        Ok(s) if s.success() => {
            // Success! The app has performed installation and closed.
            // Now spawn the app for real and exit.
            let _ = Command::new(&exe_path).spawn();
        }
        _ => {
            eprintln!("Installation failed or was cancelled.");
        }
    }
}
