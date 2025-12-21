use std::thread;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::data::history::*;
use crate::ui::resources::*;

pub unsafe fn run_command() {
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

    let main_hwnd = match FindWindowW(w!("SwiftRunClass"), w!("SwiftRun")) {
        Ok(h) => h,
        Err(_) => return,
    };

    let is_url =
        input_str.starts_with("http") || input_str.starts_with("www") || input_str.contains("://");

    let main_hwnd_val = main_hwnd.0 as usize;

    thread::spawn(move || {
        let main_hwnd = HWND(main_hwnd_val as *mut std::ffi::c_void);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let mut file_path = input_str.clone();
        let mut params = String::new();
        let mut verb = PCWSTR::null();

        if !is_url {
            if GetKeyState(VK_CONTROL.0 as i32) < 0 && GetKeyState(VK_SHIFT.0 as i32) < 0 {
                verb = w!("runas");
            }
            if let Some(idx) = input_str.find(' ') {
                file_path = input_str[..idx].to_string();
                params = input_str[idx + 1..].to_string();
            }
        }

        if params.is_empty() && file_path.contains(' ') && !file_path.starts_with('"') {
            file_path = format!("\"{}\"", file_path);
        }

        let file_u16: Vec<u16> = file_path.encode_utf16().chain(std::iter::once(0)).collect();
        let params_u16: Vec<u16> = params.encode_utf16().chain(std::iter::once(0)).collect();

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
            PostMessageW(Some(main_hwnd), WM_APP_CLOSE, WPARAM(0), LPARAM(0));
        } else {
            PostMessageW(Some(main_hwnd), WM_APP_ERROR, WPARAM(0), LPARAM(0));
        }
    });
}
