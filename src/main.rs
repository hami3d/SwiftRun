#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]
#![allow(non_snake_case)]
#![windows_subsystem = "windows"]

use std::fs;
use std::time::Instant;
use windows::{
    Win32::Foundation::*, Win32::Graphics::Direct2D::*, Win32::Graphics::DirectWrite::*,
    Win32::Graphics::Dwm::*, Win32::Graphics::Gdi::*, Win32::Media::*, Win32::System::Com::*,
    Win32::System::LibraryLoader::GetModuleHandleW, Win32::System::RemoteDesktop::*,
    Win32::UI::HiDpi::SetProcessDpiAwareness, Win32::UI::Input::KeyboardAndMouse::*,
    Win32::UI::WindowsAndMessaging::*, core::*,
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
use system::shortcut::*;
use ui::resources::*;
use ui::*;

fn main() -> Result<()> {
    unsafe {
        let _ = timeBeginPeriod(1);
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() {
            eprintln!("CoInitializeEx failed: {:?}", hr);
        }
        let _ = SetProcessDpiAwareness(windows::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE);

        D2D_FACTORY = match D2D1CreateFactory(
            D2D1_FACTORY_TYPE_SINGLE_THREADED,
            Some(&D2D1_FACTORY_OPTIONS::default()),
        ) {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("D2D1CreateFactory failed: {:?}", e);
                None
            }
        };
        DWRITE_FACTORY = match DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("DWriteCreateFactory failed: {:?}", e);
                None
            }
        };

        let instance = match GetModuleHandleW(None) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("GetModuleHandleW failed: {:?}", e);
                return Err(e.into());
            }
        };
        let class_name = w!("SwiftRunClass");
        let dropdown_class_name = w!("SwiftRunDropdown");
        let tooltip_class_name = w!("SwiftRunTooltip");
        let dialog_class_name = w!("SwiftDialog");

        let h_icon = LoadImageW(
            Some(instance.into()),
            w!("icon.ico"),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        )
        .map(|h| HICON(h.0 as _))
        .unwrap_or(LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON(std::ptr::null_mut())));

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
        let _ = RegisterClassW(&wc);

        let wc_dropdown = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hInstance: instance.into(),
            lpszClassName: dropdown_class_name,
            lpfnWndProc: Some(dropdown_wndproc),
            hbrBackground: HBRUSH::default(),
            hIcon: h_icon,
            ..Default::default()
        };
        let _ = RegisterClassW(&wc_dropdown);

        let wc_tooltip = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hInstance: instance.into(),
            lpszClassName: tooltip_class_name,
            lpfnWndProc: Some(tooltip_wndproc),
            hbrBackground: HBRUSH::default(),
            hIcon: h_icon,
            ..Default::default()
        };
        let _ = RegisterClassW(&wc_tooltip);

        let wc_dialog = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hInstance: instance.into(),
            lpszClassName: dialog_class_name,
            lpfnWndProc: Some(dialog_wndproc),
            hbrBackground: HBRUSH::default(),
            hIcon: h_icon,
            ..Default::default()
        };
        let _ = RegisterClassW(&wc_dialog);

        let args: Vec<String> = std::env::args().collect();

        // Single instance check
        if let Ok(existing_hwnd) = FindWindowW(w!("SwiftRunClass"), w!("SwiftRun")) {
            if !existing_hwnd.0.is_null() {
                if args.len() == 1 || (args.len() > 1 && args[1] == "--show") {
                    let mut pid = 0u32;
                    GetWindowThreadProcessId(existing_hwnd, Some(&mut pid));
                    let _ = AllowSetForegroundWindow(pid);
                    let _ = SendMessageW(
                        existing_hwnd,
                        WM_APP_SHOW_UI,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    );
                    return Ok(());
                }
            }
        }

        if args.len() > 1 {
            if args[1] == "--install" {
                if let Err(e) = manage_registry_hooks(true) {
                    show_fluent_dialog(
                        "Setup Error",
                        &format!("Failed to install registry hooks: {:?}", e),
                    );
                    return Err(e.into());
                }
                let _ = manage_start_menu_shortcut(true);
                show_fluent_dialog(
                    "SwiftRun Setup",
                    "SwiftRun installed! Explorer will now restart to finalize the takeover.",
                );
                restart_explorer();
                return Ok(());
            } else if args[1] == "--uninstall" {
                kill_processes_by_name("swift_run.exe");
                if let Err(e) = manage_registry_hooks(false) {
                    show_fluent_dialog(
                        "Setup Error",
                        &format!("Failed to uninstall registry hooks: {:?}", e),
                    );
                    return Err(e.into());
                }
                let _ = manage_start_menu_shortcut(false);
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
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut work_area as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );

        let dpi = windows::Win32::UI::HiDpi::GetDpiForSystem();
        let scale = dpi as f32 / 96.0;
        let x = work_area.left + (18.0 * scale) as i32;
        let y = work_area.bottom - (WIN_H * scale) as i32 - (18.0 * scale) as i32;

        FINAL_X = x;
        FINAL_Y = y;
        START_Y = work_area.bottom;

        let hwnd_result = CreateWindowExW(
            WS_EX_TOPMOST,
            class_name,
            w!("SwiftRun"),
            WS_POPUP,
            x,
            START_Y,
            (WIN_W * scale) as i32,
            (WIN_H * scale) as i32,
            None,
            None,
            Some(instance.into()),
            None,
        );
        let hwnd = match hwnd_result {
            Ok(h) => h,
            Err(e) => {
                eprintln!("CreateWindowExW(main) failed: {:?}", e);
                return Err(e.into());
            }
        };
        H_MAIN = hwnd;
        let _ = ChangeWindowMessageFilterEx(hwnd, WM_APP_SHOW_UI, MSGFLT_ALLOW, None);

        register_hotkeys(hwnd);
        let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);

        H_DROPDOWN = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            dropdown_class_name,
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            Some(hwnd),
            None,
            Some(instance.into()),
            None,
        )
        .unwrap();

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
            Some(hwnd),
            Some(HMENU(EDIT_ID as _)),
            Some(instance.into()),
            None,
        )
        .unwrap();
        let hfont = CreateFontW(
            FONT_SZ_INPUT,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            ANSI_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            0,
            FONT_STD,
        );
        SendMessageW(
            H_EDIT,
            WM_SETFONT,
            Some(WPARAM(hfont.0 as usize)),
            Some(LPARAM(1)),
        );

        if let Some(history) = &HISTORY {
            if let Some(latest) = history.first() {
                if let Ok(mut lock) = INPUT_BUFFER.lock() {
                    *lock = latest.clone();
                }
                let latest_u16: Vec<u16> =
                    latest.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = SetWindowTextW(H_EDIT, PCWSTR(latest_u16.as_ptr()));
                SendMessageW(
                    H_EDIT,
                    windows::Win32::UI::Controls::EM_SETSEL,
                    Some(WPARAM(0)),
                    Some(LPARAM(-1)),
                );
            }
        }

        let blink_time = GetCaretBlinkTime();
        SetTimer(
            Some(hwnd),
            1,
            if blink_time == 0 { 500 } else { blink_time },
            None,
        );
        let should_show = args.len() == 1 || (args.len() > 1 && args[1] == "--show");
        if should_show {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(H_EDIT));
            ANIM_TYPE = AnimType::Entering;
            ANIM_START_TIME = Some(Instant::now());
            SetTimer(Some(hwnd), 3, ANIM_TIMER_MS, None);
        } else {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        // Heartbeat timer to ensure hotkeys stay registered (Timer ID 4, every 30s)
        SetTimer(Some(hwnd), 4, 30000, None);

        let mut msg = MSG::default();
        loop {
            if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break;
                }

                if (msg.message == WM_KEYDOWN || msg.message == WM_KEYUP) && msg.hwnd == H_EDIT {
                    let vk = msg.wParam.0 as i32;
                    if msg.message == WM_KEYDOWN {
                        match vk {
                            v if v == VK_RETURN.0 as i32 => {
                                let is_elevated = (GetKeyState(VK_CONTROL.0 as i32) < 0
                                    && GetKeyState(VK_SHIFT.0 as i32) < 0)
                                    as usize;
                                let _ = PostMessageW(
                                    Some(hwnd),
                                    WM_APP_RUN_COMMAND,
                                    WPARAM(is_elevated),
                                    LPARAM(0),
                                );
                                continue; // Prevent beep
                            }
                            v if v == VK_UP.0 as i32 => {
                                cycle_history(-1, H_EDIT);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                                continue;
                            }
                            v if v == VK_DOWN.0 as i32 => {
                                cycle_history(1, H_EDIT);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                                continue;
                            }
                            v if v == VK_TAB.0 as i32
                                || (v == VK_RIGHT.0 as i32 && !PREDICTION.is_empty()) =>
                            {
                                // Prediction acceptance logic...
                                let pred = PREDICTION.clone();
                                if !pred.is_empty() {
                                    if let Ok(mut lock) = INPUT_BUFFER.lock() {
                                        *lock = pred.clone();
                                    }
                                    let u16_vec: Vec<u16> =
                                        pred.encode_utf16().chain(std::iter::once(0)).collect();
                                    IS_CYCLING = true;
                                    let _ = SetWindowTextW(H_EDIT, PCWSTR(u16_vec.as_ptr()));
                                    SendMessageW(
                                        H_EDIT,
                                        windows::Win32::UI::Controls::EM_SETSEL,
                                        Some(WPARAM(0)),
                                        Some(LPARAM(-1)),
                                    );
                                    IS_CYCLING = false;
                                    PREDICTION = String::new();
                                    FILTERED_HISTORY = None;
                                    if SHOW_DROPDOWN {
                                        SHOW_DROPDOWN = false;
                                        let _ = ShowWindow(H_DROPDOWN, SW_HIDE);
                                    }
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                }
                                continue; // Prevent beep/focus shift
                            }
                            v if v == VK_BACK.0 as i32
                                && GetKeyState(VK_CONTROL.0 as i32) < 0
                                && GetKeyState(VK_SHIFT.0 as i32) < 0 =>
                            {
                                if let Some(path) = get_history_path() {
                                    let _ = fs::remove_file(path);
                                }
                                HISTORY = Some(Vec::new());
                                HISTORY_INDEX = -1;
                                if SHOW_DROPDOWN {
                                    SHOW_DROPDOWN = false;
                                    let _ = ShowWindow(H_DROPDOWN, SW_HIDE);
                                }
                                show_tooltip(
                                    "History Cleared",
                                    "The command history has been successfully removed.",
                                );
                                let _ = InvalidateRect(Some(hwnd), None, false);
                                continue;
                            }
                            _ => {}
                        }
                    } else if msg.message == WM_KEYUP {
                        // Also consume KEYUP for these keys to be safe
                        match vk {
                            v if v == VK_RETURN.0 as i32
                                || v == VK_UP.0 as i32
                                || v == VK_DOWN.0 as i32
                                || v == VK_TAB.0 as i32 =>
                            {
                                continue;
                            }
                            _ => {}
                        }
                    }
                }

                if msg.message == WM_KEYDOWN && msg.wParam.0 == 0x1B {
                    start_exit_animation(hwnd, false);
                }

                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                if is_any_animation_active() {
                    let _ = DwmFlush();
                    update_animations(hwnd);
                } else {
                    WaitMessage().ok();
                }
            }
        }
        let _ = timeEndPeriod(1);
    }
    Ok(())
}
