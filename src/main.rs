#![allow(static_mut_refs)]
#![allow(non_snake_case)]

use std::fs;
use std::time::Instant;
use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Direct2D::*, Win32::Graphics::DirectWrite::*,
    Win32::Graphics::Dwm::*, Win32::Graphics::Gdi::*, Win32::System::Com::*,
    Win32::System::LibraryLoader::GetModuleHandleW, Win32::UI::HiDpi::SetProcessDpiAwareness,
    Win32::UI::Input::KeyboardAndMouse::*, Win32::UI::WindowsAndMessaging::*,
};

mod animations;
mod config;
mod data;
mod system;
mod ui;

use animations::*;
use config::*;
use data::history::*;
use system::explorer::*;
use system::hotkeys::*;
use system::registry::*;
use ui::resources::*;
use ui::*;

fn main() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok();
        let _ = SetProcessDpiAwareness(windows::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE);

        D2D_FACTORY = D2D1CreateFactory(
            D2D1_FACTORY_TYPE_SINGLE_THREADED,
            Some(&D2D1_FACTORY_OPTIONS::default()),
        )
        .ok();
        DWRITE_FACTORY = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok();

        let instance = GetModuleHandleW(None)?;
        let class_name = w!("SwiftRunClass");
        let dropdown_class_name = w!("SwiftRunDropdown");
        let tooltip_class_name = w!("SwiftRunTooltip");
        let dialog_class_name = w!("SwiftDialog");

        let h_icon = LoadImageW(
            instance,
            w!("icon.ico"),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        )
        .map(|h| HICON(h.0 as _))
        .unwrap_or(LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON(0)));

        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hInstance: instance.into(),
            lpszClassName: class_name,
            lpfnWndProc: Some(wndproc),
            hbrBackground: HBRUSH::default(),
            hIcon: h_icon,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let wc_dropdown = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hInstance: instance,
            lpszClassName: dropdown_class_name,
            lpfnWndProc: Some(dropdown_wndproc),
            hbrBackground: HBRUSH::default(),
            hIcon: h_icon,
            ..Default::default()
        };
        RegisterClassW(&wc_dropdown);

        let wc_tooltip = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hInstance: instance,
            lpszClassName: tooltip_class_name,
            lpfnWndProc: Some(tooltip_wndproc),
            hbrBackground: HBRUSH::default(),
            hIcon: h_icon,
            ..Default::default()
        };
        RegisterClassW(&wc_tooltip);

        let wc_dialog = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hInstance: instance,
            lpszClassName: dialog_class_name,
            lpfnWndProc: Some(dialog_wndproc),
            hbrBackground: HBRUSH::default(),
            hIcon: h_icon,
            ..Default::default()
        };
        RegisterClassW(&wc_dialog);

        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1 {
            if args[1] == "--install" {
                if let Err(e) = manage_registry_hooks(true) {
                    show_fluent_dialog(
                        "Setup Error",
                        &format!("Failed to install registry hooks: {:?}", e),
                    );
                    return Err(e);
                }
                show_fluent_dialog(
                    "SwiftRun Setup",
                    "SwiftRun installed! Explorer will now restart to finalize the takeover.",
                );
                restart_explorer();
            } else if args[1] == "--uninstall" {
                if let Err(e) = manage_registry_hooks(false) {
                    show_fluent_dialog(
                        "Setup Error",
                        &format!("Failed to uninstall registry hooks: {:?}", e),
                    );
                    return Err(e);
                }
                show_fluent_dialog(
                    "SwiftRun Setup",
                    "SwiftRun uninstalled! Win+R will return to default behavior after restart.",
                );
                restart_explorer();
                return Ok(());
            }
        }

        load_history();

        let mut work_area = RECT::default();
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut work_area as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );

        let x = work_area.left + 18;
        let y = work_area.bottom - WIN_H as i32 - 18;

        FINAL_X = x;
        FINAL_Y = y;
        START_Y = work_area.bottom;

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("SwiftRun"),
            WS_POPUP,
            x,
            START_Y,
            WIN_W as i32,
            WIN_H as i32,
            None,
            None,
            instance,
            None,
        );

        register_hotkeys(hwnd);

        H_DROPDOWN = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            dropdown_class_name,
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            hwnd,
            None,
            instance,
            None,
        );

        let v: i32 = 1;
        DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(20), &v as *const _ as _, 4).ok();
        set_acrylic_effect(hwnd);
        let v: i32 = 2; // Round corners
        DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(33), &v as *const _ as _, 4).ok();
        let m = windows::Win32::UI::Controls::MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        DwmExtendFrameIntoClientArea(hwnd, &m).ok();

        H_EDIT = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            None,
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(0x0080),
            0,
            0,
            0,
            0,
            hwnd,
            HMENU(EDIT_ID as _),
            instance,
            None,
        );
        let hfont = CreateFontW(20, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"));
        SendMessageW(H_EDIT, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));

        if let Some(history) = &HISTORY {
            if let Some(latest) = history.first() {
                if let Ok(mut lock) = INPUT_BUFFER.lock() {
                    *lock = latest.clone();
                }
                let latest_u16: Vec<u16> =
                    latest.encode_utf16().chain(std::iter::once(0)).collect();
                SetWindowTextW(H_EDIT, PCWSTR(latest_u16.as_ptr()));
                SendMessageW(
                    H_EDIT,
                    windows::Win32::UI::Controls::EM_SETSEL,
                    WPARAM(0),
                    LPARAM(-1),
                );
            }
        }

        let blink_time = GetCaretBlinkTime();
        SetTimer(
            hwnd,
            1,
            if blink_time == 0 { 500 } else { blink_time },
            None,
        );
        ANIM_TYPE = AnimType::Entering;
        ANIM_START_TIME = Some(Instant::now());
        SetTimer(hwnd, 3, 10, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if (msg.message == WM_KEYDOWN || msg.message == WM_KEYUP) && msg.hwnd == H_EDIT {
                let vk = msg.wParam.0 as i32;
                if msg.message == WM_KEYDOWN && vk == VK_RETURN.0 as i32 {
                    if let Ok(cmd) = INPUT_BUFFER.lock() {
                        if cmd.trim() == ":quit" || cmd.trim() == ":exit" {
                            start_exit_animation(hwnd, true);
                            continue;
                        }
                    }
                }
                if msg.message == WM_KEYDOWN {
                    if vk == VK_UP.0 as i32 {
                        cycle_history(-1, H_EDIT);
                        let _ = InvalidateRect(hwnd, None, BOOL(0));
                        continue;
                    }
                    if vk == VK_DOWN.0 as i32 {
                        cycle_history(1, H_EDIT);
                        let _ = InvalidateRect(hwnd, None, BOOL(0));
                        continue;
                    }
                    if GetKeyState(VK_CONTROL.0 as i32) < 0
                        && GetKeyState(VK_SHIFT.0 as i32) < 0
                        && vk == VK_BACK.0 as i32
                    {
                        if let Some(path) = get_history_path() {
                            let _ = fs::remove_file(path);
                        }
                        HISTORY = Some(Vec::new());
                        HISTORY_INDEX = -1;
                        if SHOW_DROPDOWN {
                            SHOW_DROPDOWN = false;
                            ShowWindow(H_DROPDOWN, SW_HIDE);
                        }
                        show_tooltip("Command History Has Been Cleared");
                        let _ = InvalidateRect(hwnd, None, BOOL(0));
                        continue;
                    }
                    if vk == VK_RETURN.0 as i32 {
                        PostMessageW(hwnd, WM_APP_RUN_COMMAND, WPARAM(0), LPARAM(0));
                        continue;
                    }
                }
                let _ = InvalidateRect(hwnd, None, BOOL(0));
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 == 0x1B {
                start_exit_animation(hwnd, false);
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
