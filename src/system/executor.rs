#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]
#![allow(non_snake_case)]

use std::thread;
use windows::Win32::Foundation::*;

use windows::Win32::System::Com::*;
use windows::Win32::System::Environment::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::data::history::*;
use crate::ui::resources::*;

pub unsafe fn run_command(elevated: bool) {
    let mut input_str = String::new();
    if let Ok(buf) = INPUT_BUFFER.lock() {
        if !buf.is_empty() {
            input_str = buf.clone();
            save_history(&input_str);
        }
    }

    if input_str.is_empty() {
        return;
    }

    let main_hwnd = unsafe { FindWindowW(w!("SwiftRunClass"), w!("SwiftRun")) };
    let main_hwnd = match main_hwnd {
        Ok(h) => h,
        Err(_) => return,
    };

    let input_str = unsafe { expand_aliases_and_env(input_str.trim()) };

    let is_url =
        input_str.starts_with("http") || input_str.starts_with("www") || input_str.contains("://");

    let main_hwnd_val = main_hwnd.0 as usize;

    thread::spawn(move || {
        unsafe {
            let main_hwnd = HWND(main_hwnd_val as *mut std::ffi::c_void);
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let mut file_path = input_str.clone();
            let mut params = String::new();
            let verb = if elevated {
                w!("runas")
            } else {
                PCWSTR::null()
            };

            if !is_url {
                // Improved parsing: if it's an absolute path to an existing file/dir, don't split by spaces unless quoted
                let path_exists = std::path::Path::new(&input_str).exists();
                if !path_exists {
                    if let Some(idx) = input_str.find(' ') {
                        file_path = input_str[..idx].to_string();
                        params = input_str[idx + 1..].to_string();
                    }
                }
            }

            if params.is_empty() && file_path.contains(' ') && !file_path.starts_with('"') {
                file_path = format!("\"{}\"", file_path);
            }

            let file_u16: Vec<u16> = file_path.encode_utf16().chain(std::iter::once(0)).collect();
            let params_u16: Vec<u16> = params.encode_utf16().chain(std::iter::once(0)).collect();

            if let Ok(cwd) = std::env::current_dir() {
                let _ = std::env::set_current_dir(cwd);
            }

            let res = ShellExecuteW(
                None,
                verb,
                PCWSTR(file_u16.as_ptr()),
                if params.is_empty() {
                    PCWSTR::null()
                } else {
                    PCWSTR(params_u16.as_ptr())
                },
                None,
                SW_SHOWNORMAL,
            );

            CoUninitialize();

            if (res.0 as isize) > 32 {
                let _ = PostMessageW(Some(main_hwnd), WM_APP_CLOSE, WPARAM(0), LPARAM(0));
            } else {
                let _ = PostMessageW(Some(main_hwnd), WM_APP_ERROR, WPARAM(0), LPARAM(0));
            }
        }
    });
}

unsafe fn expand_aliases_and_env(input: &str) -> String {
    let mut result = input.to_string();

    // 1. Handle Quick Aliases (Command-only)
    let parts: Vec<&str> = result.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let alias_path = match cmd.as_str() {
        "docs" | "documents" => get_known_folder_path(&FOLDERID_Documents),
        "pictures" | "pics" => get_known_folder_path(&FOLDERID_Pictures),
        "videos" | "vids" => get_known_folder_path(&FOLDERID_Videos),
        "music" => get_known_folder_path(&FOLDERID_Music),
        "downloads" => get_known_folder_path(&FOLDERID_Downloads),
        "desktop" => get_known_folder_path(&FOLDERID_Desktop),
        "terminal" | "term" => Some("wt".to_string()),
        _ => None,
    };

    if let Some(path) = alias_path {
        if parts.len() > 1 {
            result = format!("{} {}", path, parts[1]);
        } else {
            result = path;
        }
    }

    // 2. Expand Environment Variables (e.g. %appdata%)
    if result.contains('%') {
        let input_u16: Vec<u16> = result.encode_utf16().chain(std::iter::once(0)).collect();
        let mut buffer = [0u16; 32768];
        let len = ExpandEnvironmentStringsW(PCWSTR(input_u16.as_ptr()), Some(&mut buffer));
        if len > 0 && len <= 32768 {
            result = String::from_utf16_lossy(&buffer[..len as usize - 1]);
        }
    }

    result
}

unsafe fn get_known_folder_path(folder_id: *const GUID) -> Option<String> {
    if let Ok(path) = SHGetKnownFolderPath(folder_id, KF_FLAG_DEFAULT, None) {
        let s = path.to_string().ok();
        CoTaskMemFree(Some(path.as_ptr() as *const _));
        s
    } else {
        None
    }
}
