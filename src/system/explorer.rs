use windows::Win32::Foundation::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::*;

pub unsafe fn kill_processes_by_name(name: &str) {
    let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    if Process32FirstW(snapshot, &mut entry).is_ok() {
        loop {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let exe_name = String::from_utf16_lossy(&entry.szExeFile[..len]);

            if exe_name.eq_ignore_ascii_case(name) {
                if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID) {
                    let _ = TerminateProcess(handle, 0);
                    let _ = CloseHandle(handle);
                }
            }
            if !Process32NextW(snapshot, &mut entry).is_ok() {
                break;
            }
        }
    }
    let _ = CloseHandle(snapshot);
}

pub unsafe fn restart_explorer() {
    kill_processes_by_name("explorer.exe");
    let _ = ShellExecuteW(None, None, w!("explorer.exe"), None, None, SW_SHOWNORMAL);
}
