#![windows_subsystem = "windows"]
use std::env;
use std::process::Command;

fn main() {
    let mut exe_path = env::current_exe().expect("Failed to get current exe path");
    exe_path.pop();
    exe_path.push("swift_run.exe");

    if !exe_path.exists() {
        println!("Error: swift_run.exe not found in the same directory.");
        return;
    }

    let status = Command::new(exe_path)
        .arg("--install")
        .status()
        .expect("Failed to execute swift_run.exe");

    if !status.success() {
        println!("Error: Installation failed.");
    }
}
