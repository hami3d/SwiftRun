use std::time::Instant;
use windows::Win32::Foundation::*;
use windows::core::*;

use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_numerics::Vector2 as D2D_POINT_2F;

use crate::animations::*;
use crate::config::*;
use crate::data::history::*;
use crate::system::executor::run_command;
use crate::system::hotkeys::*;
use crate::ui::resources::*;
use crate::ui::tooltip::show_tooltip;
use crate::ui::*;

pub unsafe fn is_input_empty() -> bool {
    if let Ok(buf) = INPUT_BUFFER.lock() {
        buf.is_empty()
    } else {
        true
    }
}

pub unsafe fn hit_test(x: i32, y: i32, w: f32, _h: f32, input_empty: bool) -> HoverId {
    let fx = x as f32;
    let fy = y as f32;

    if fx >= w - WIN_BTN_W && fy < TITLE_BAR_H {
        return HoverId::Close;
    }
    if fx >= w - WIN_BTN_W * 2.0 && fx < w - WIN_BTN_W && fy < TITLE_BAR_H {
        return HoverId::Min;
    }

    let ok_x = w - MARGIN - BTN_W * 2.0 - 8.0;
    if fx >= ok_x && fx < ok_x + BTN_W && fy >= BTN_Y && fy < BTN_Y + BTN_H {
        return if input_empty {
            HoverId::None
        } else {
            HoverId::Ok
        };
    }

    let cancel_x = w - MARGIN - BTN_W;
    if fx >= cancel_x && fx < cancel_x + BTN_W && fy >= BTN_Y && fy < BTN_Y + BTN_H {
        return HoverId::Cancel;
    }

    let chevron_x = w - MARGIN - 20.0;
    let chevron_y = INPUT_Y + INPUT_H / 2.0;
    if fx >= chevron_x - 10.0
        && fx < chevron_x + 10.0
        && fy >= chevron_y - 10.0
        && fy < chevron_y + 10.0
    {
        return HoverId::Dropdown;
    }

    if fx >= MARGIN && fx < w - MARGIN && fy >= INPUT_Y && fy < INPUT_Y + INPUT_H {
        return HoverId::Input;
    }

    HoverId::None
}

pub unsafe fn start_exit_animation(hwnd: HWND, kill: bool) {
    if ANIM_TYPE == AnimType::Exiting {
        EXIT_KILL_PROCESS = kill;
        return;
    }
    EXIT_KILL_PROCESS = kill;

    let mut rect = RECT::default();
    let _ = GetWindowRect(hwnd, &mut rect);

    if rect.left != FINAL_X || rect.top != FINAL_Y {
        if kill {
            PostQuitMessage(0);
        } else {
            ShowWindow(hwnd, SW_HIDE);
        }
        return;
    }

    if SHOW_DROPDOWN {
        SHOW_DROPDOWN = false;
        let _ = ShowWindow(H_DROPDOWN, SW_HIDE);
    }
    if !H_TOOLTIP.0.is_null() {
        let _ = DestroyWindow(H_TOOLTIP);
        H_TOOLTIP = HWND(std::ptr::null_mut());
    }

    ANIM_TYPE = AnimType::Exiting;
    ANIM_START_TIME = Some(Instant::now());
    SetTimer(Some(hwnd), 3, ANIM_TIMER_MS, None);
}

pub unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_APP_RUN_COMMAND => {
            run_command();
            LRESULT(0)
        }
        WM_APP_CLOSE => {
            start_exit_animation(hwnd, false);
            LRESULT(0)
        }
        WM_APP_ERROR => {
            let _ = ShowWindow(hwnd, SW_SHOW);
            if let Ok(buf) = INPUT_BUFFER.lock() {
                show_tooltip(
                    "Command not found",
                    &format!(
                        "SwiftRun cannot find '{}'. Make sure you typed the name correctly, and then try again.",
                        buf
                    ),
                );
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            start_exit_animation(hwnd, false);
            LRESULT(0)
        }
        WM_SIZE => {
            if wp.0 == 1 {
                // SIZE_MINIMIZED
                if SHOW_DROPDOWN {
                    SHOW_DROPDOWN = false;
                    let _ = ShowWindow(H_DROPDOWN, SW_HIDE);
                }
            }
            if let Some(target) = RENDER_TARGET.as_ref() {
                let size = D2D_SIZE_U {
                    width: (lp.0 & 0xFFFF) as u32,
                    height: ((lp.0 >> 16) & 0xFFFF) as u32,
                };
                let _ = target.Resize(&size);
            }
            let w = (lp.0 & 0xFFFF) as i32;
            SetWindowPos(
                H_EDIT,
                None,
                -10000,
                -10000,
                w,
                50,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if wp.0 == 0 {
                if SHOW_DROPDOWN {
                    SHOW_DROPDOWN = false;
                    let _ = ShowWindow(H_DROPDOWN, SW_HIDE);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            } else {
                SetFocus(Some(H_EDIT));
                SendMessageW(H_EDIT, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
            }
            LRESULT(0)
        }
        WM_NCHITTEST => {
            let x = (lp.0 & 0xFFFF) as i16 as i32;
            let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;
            let mut wr = RECT::default();
            let _ = GetWindowRect(hwnd, &mut wr);
            let lx = x - wr.left;
            let ly = y - wr.top;
            let width = (wr.right - wr.left) as f32;
            let scale = get_dpi_scale(hwnd);
            let slx = lx as f32 / scale;
            let sly = ly as f32 / scale;

            if sly < TITLE_BAR_H && slx < (width / scale) - WIN_BTN_W * 2.0 {
                return LRESULT(HTCAPTION as isize);
            }
            LRESULT(HTCLIENT as isize)
        }
        WM_SETCURSOR => {
            if HOVER == HoverId::Input {
                let _ = SetCursor(Some(LoadCursorW(None, IDC_IBEAM).unwrap()));
                LRESULT(1)
            } else {
                DefWindowProcW(hwnd, msg, wp, lp)
            }
        }
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wp.0 as _);
            let is_dark = is_dark_mode();
            if is_dark {
                SetTextColor(hdc, COLORREF(0x00FFFFFF));
                SetBkColor(hdc, COLORREF(0x002C2C2C));
                static mut DARK_BRUSH: HBRUSH = HBRUSH(std::ptr::null_mut());
                if DARK_BRUSH.0.is_null() {
                    DARK_BRUSH = CreateSolidBrush(COLORREF(0x002C2C2C));
                }
                LRESULT(DARK_BRUSH.0 as isize)
            } else {
                SetTextColor(hdc, COLORREF(0x00000000));
                SetBkColor(hdc, COLORREF(0x00FFFFFF));
                static mut LIGHT_BRUSH: HBRUSH = HBRUSH(std::ptr::null_mut());
                if LIGHT_BRUSH.0.is_null() {
                    LIGHT_BRUSH = CreateSolidBrush(COLORREF(0x00FFFFFF));
                }
                LRESULT(LIGHT_BRUSH.0 as isize)
            }
        }
        WM_TIMER => {
            if wp.0 == 1 {
                let _ = InvalidateRect(Some(hwnd), None, false);
            } else if wp.0 == 3 {
                let mut still_animating = false;
                if let Some(start) = ANIM_START_TIME {
                    let elapsed = start.elapsed().as_millis();
                    match ANIM_TYPE {
                        AnimType::Entering => {
                            let progress =
                                (elapsed as f32 / ANIM_ENTER_DURATION_MS as f32).min(1.0);
                            let eased = ease_out_cubic(progress);
                            let current_y = START_Y - ((START_Y - FINAL_Y) as f32 * eased) as i32;
                            SetWindowPos(
                                hwnd,
                                None,
                                FINAL_X,
                                current_y,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                            if progress < 1.0 {
                                still_animating = true;
                            } else {
                                ANIM_TYPE = AnimType::None;
                                SetWindowPos(
                                    hwnd,
                                    None,
                                    FINAL_X,
                                    FINAL_Y,
                                    0,
                                    0,
                                    SWP_NOSIZE | SWP_NOZORDER,
                                );
                            }
                        }
                        AnimType::Exiting => {
                            let progress = (elapsed as f32 / ANIM_EXIT_DURATION_MS as f32).min(1.0);
                            let eased = ease_out_quad(progress);
                            let current_y = FINAL_Y + ((START_Y - FINAL_Y) as f32 * eased) as i32;
                            SetWindowPos(
                                hwnd,
                                None,
                                FINAL_X,
                                current_y,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                            if progress < 1.0 {
                                still_animating = true;
                            } else {
                                ANIM_TYPE = AnimType::None;
                                if EXIT_KILL_PROCESS {
                                    PostQuitMessage(0);
                                } else {
                                    let _ = ShowWindow(hwnd, SW_HIDE);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(start) = DROPDOWN_ANIM_START {
                    let elapsed = start.elapsed().as_millis();
                    let progress = (elapsed as f32 / ANIM_DROPDOWN_DURATION_MS as f32).min(1.0);
                    if progress < 1.0 {
                        still_animating = true;
                    } else if DROPDOWN_ANIM_TYPE == AnimType::Exiting {
                        let _ = ShowWindow(H_DROPDOWN, SW_HIDE);
                        DROPDOWN_ANIM_TYPE = AnimType::None;
                    } else {
                        DROPDOWN_ANIM_TYPE = AnimType::None;
                    }
                    let _ = InvalidateRect(Some(H_DROPDOWN), None, false);
                }
                if let Some(start) = TOOLTIP_ANIM_START {
                    let elapsed = start.elapsed().as_millis();
                    let progress = (elapsed as f32 / ANIM_TOOLTIP_DURATION_MS as f32).min(1.0);
                    if progress < 1.0 {
                        still_animating = true;
                    } else if TOOLTIP_ANIM_TYPE == AnimType::Exiting {
                        let _ = DestroyWindow(H_TOOLTIP);
                        H_TOOLTIP = HWND(std::ptr::null_mut());
                        TOOLTIP_ANIM_TYPE = AnimType::None;
                    } else {
                        TOOLTIP_ANIM_TYPE = AnimType::None;
                    }
                    let _ = InvalidateRect(Some(H_TOOLTIP), None, false);
                }
                if !still_animating {
                    let _ = KillTimer(Some(hwnd), 3);
                }
            } else if wp.0 == 4 {
                // Heartbeat: re-register hotkeys just in case
                register_hotkeys(hwnd);
            }
            LRESULT(0)
        }
        WM_SETTINGCHANGE => {
            set_acrylic_effect(hwnd);
            BRUSHES = None;
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            // Screen resolution or monitor change might affect window environment
            register_hotkeys(hwnd);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let scale = get_dpi_scale(hwnd);
            let (sx, sy) = (
                (lp.0 & 0xFFFF) as i16 as f32 / scale,
                ((lp.0 >> 16) & 0xFFFF) as i16 as f32 / scale,
            );
            let mut cr = RECT::default();
            let _ = GetClientRect(hwnd, &mut cr);
            let w = (cr.right - cr.left) as f32 / scale;
            let h = (cr.bottom - cr.top) as f32 / scale;
            let new_hover = hit_test(sx as i32, sy as i32, w, h, is_input_empty());
            if new_hover != HOVER {
                HOVER = new_hover;
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            if wp.0 & 0x0001 != 0 {
                let n_lp = LPARAM(
                    (((sx - (MARGIN + 10.0)) * scale) as i32 as isize & 0xFFFF)
                        | (((20.0 * scale) as i32 as isize & 0xFFFF) << 16),
                );
                SendMessageW(H_EDIT, WM_MOUSEMOVE, Some(wp), Some(n_lp));
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let _ = ReleaseCapture();
            let n_lp = LPARAM(
                ((((lp.0 & 0xFFFF) as i16 as f32 / get_dpi_scale(hwnd) - (MARGIN + 10.0))
                    * get_dpi_scale(hwnd)) as i32 as isize
                    & 0xFFFF)
                    | (((20.0 * get_dpi_scale(hwnd)) as i32 as isize & 0xFFFF) << 16),
            );
            SendMessageW(H_EDIT, WM_LBUTTONUP, Some(wp), Some(n_lp));
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            if !H_TOOLTIP.0.is_null() {
                let _ = DestroyWindow(H_TOOLTIP);
                H_TOOLTIP = HWND(std::ptr::null_mut());
            }
            let scale = get_dpi_scale(hwnd);
            let (sx, sy) = (
                (lp.0 & 0xFFFF) as i16 as f32 / scale,
                ((lp.0 >> 16) & 0xFFFF) as i16 as f32 / scale,
            );
            let mut cr = RECT::default();
            let _ = GetClientRect(hwnd, &mut cr);
            let (w, h) = (
                (cr.right - cr.left) as f32 / scale,
                (cr.bottom - cr.top) as f32 / scale,
            );

            match hit_test(sx as i32, sy as i32, w, h, is_input_empty()) {
                HoverId::Close => start_exit_animation(hwnd, false),
                HoverId::Min => {
                    if SHOW_DROPDOWN {
                        SHOW_DROPDOWN = false;
                        let _ = ShowWindow(H_DROPDOWN, SW_HIDE);
                    }
                    let _ = ShowWindow(hwnd, SW_MINIMIZE);
                }
                HoverId::Ok => run_command(),
                HoverId::Cancel => start_exit_animation(hwnd, false),
                HoverId::Input => {
                    let _ = SetCapture(hwnd);
                    let n_lp = LPARAM(
                        (((sx - (MARGIN + 10.0)) * scale) as i32 as isize & 0xFFFF)
                            | (((20.0 * scale) as i32 as isize & 0xFFFF) << 16),
                    );
                    SendMessageW(H_EDIT, WM_LBUTTONDOWN, Some(wp), Some(n_lp));
                    SetFocus(Some(H_EDIT));
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                HoverId::None => {
                    SetFocus(Some(H_EDIT));
                    if SHOW_DROPDOWN {
                        SHOW_DROPDOWN = false;
                        ShowWindow(H_DROPDOWN, SW_HIDE);
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                HoverId::Dropdown => {
                    let is_empty = HISTORY.as_ref().map_or(true, |h| h.is_empty());
                    if is_empty {
                        show_tooltip(
                            "History Cleared",
                            "The command history has been successfully removed.",
                        );
                    } else {
                        if !SHOW_DROPDOWN {
                            SHOW_DROPDOWN = true;
                            HOVER_DROPDOWN = Some(0);
                            SCROLL_OFFSET = 0;
                            let mut rect = RECT::default();
                            let _ = GetWindowRect(hwnd, &mut rect);
                            let scale = get_dpi_scale(hwnd);
                            let margin_px = (MARGIN * scale) as i32;
                            let (x, y) = (
                                rect.left + margin_px,
                                rect.top
                                    + ((INPUT_Y + INPUT_H) * scale) as i32
                                    + (DROPDOWN_GAP * scale) as i32,
                            );
                            let w = (rect.right - rect.left) - (margin_px * 2);
                            let mut drop_h = 0;
                            if let Some(h) = HISTORY.as_ref() {
                                drop_h = (h.len().min(DROPDOWN_MAX_ITEMS) as f32 * ITEM_H * scale)
                                    as i32;
                            }
                            if drop_h > 0 {
                                SetWindowPos(
                                    H_DROPDOWN,
                                    Some(HWND_TOPMOST),
                                    x,
                                    y,
                                    w,
                                    drop_h,
                                    SWP_SHOWWINDOW | SWP_NOACTIVATE,
                                );
                                DROPDOWN_ANIM_START = Some(Instant::now());
                                DROPDOWN_ANIM_TYPE = AnimType::Entering;
                                SetTimer(Some(hwnd), 3, ANIM_TIMER_MS, None);
                                SHOW_DROPDOWN = false;
                            } else {
                                SHOW_DROPDOWN = false;
                                DROPDOWN_ANIM_START = Some(Instant::now());
                                DROPDOWN_ANIM_TYPE = AnimType::Exiting;
                                SetTimer(Some(hwnd), 3, ANIM_TIMER_MS, None);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                        }
                    }

                    if TOOLTIP_ANIM_TYPE != AnimType::None {
                        if !H_TOOLTIP.0.is_null() {
                            let _ = InvalidateRect(Some(H_TOOLTIP), None, false);
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let (id, code) = (wp.0 & 0xFFFF, (wp.0 >> 16) & 0xFFFF);
            if id as u32 == EDIT_ID && code == 0x0300 {
                if !IS_CYCLING {
                    HISTORY_INDEX = -1;
                }
                let len = GetWindowTextLengthW(H_EDIT);
                let mut buf = vec![0u16; (len + 1) as usize];
                GetWindowTextW(H_EDIT, &mut buf);
                if let Ok(mut lock) = INPUT_BUFFER.lock() {
                    *lock = String::from_utf16_lossy(&buf[..len as usize]);
                }
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);
            ensure_resources(hwnd);
            paint();
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_SETFOCUS => {
            SetFocus(Some(H_EDIT));
            LRESULT(0)
        }
        WM_HOTKEY => {
            if wp.0 == 1 {
                if IsWindowVisible(hwnd).as_bool() {
                    start_exit_animation(hwnd, false);
                } else {
                    let _ = ShowWindow(hwnd, SW_SHOW);
                    SetForegroundWindow(hwnd);
                    SetFocus(Some(H_EDIT));
                    ANIM_TYPE = AnimType::Entering;
                    ANIM_START_TIME = Some(Instant::now());
                    SetTimer(Some(hwnd), 3, 10, None);
                    if let Some(history) = &HISTORY {
                        if let Some(latest) = history.first() {
                            if let Ok(mut lock) = INPUT_BUFFER.lock() {
                                *lock = latest.clone();
                            }
                            let latest_u16: Vec<u16> =
                                latest.encode_utf16().chain(std::iter::once(0)).collect();
                            SetWindowTextW(H_EDIT, PCWSTR(latest_u16.as_ptr()));
                            SendMessageW(H_EDIT, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_WTSSESSION_CHANGE => {
            // Re-register hotkeys on session unlock to ensure they still work
            if wp.0 == 0x7 || wp.0 == 0x8 {
                // WTS_SESSION_UNLOCK || WTS_SESSION_LOGON
                register_hotkeys(hwnd);
            }
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            // Re-register on resume from sleep
            if wp.0 == 0x7 || wp.0 == 0x12 {
                // PBT_APMRESUMESUSPEND || PBT_APMRESUMEAUTOMATIC
                register_hotkeys(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            crate::system::hotkeys::unregister_hotkeys(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wp.0 as u16;
            if !SHOW_DROPDOWN {
                if vk == VK_UP.0 {
                    cycle_history(-1, H_EDIT);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    return LRESULT(0);
                } else if vk == VK_DOWN.0 {
                    cycle_history(1, H_EDIT);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    return LRESULT(0);
                }
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub unsafe fn ensure_resources(hwnd: HWND) {
    if RENDER_TARGET.is_none() {
        let Some(factory) = &D2D_FACTORY else { return };

        let dpi = GetDpiForWindow(hwnd) as f32;
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: dpi,
            dpiY: dpi,
            ..Default::default()
        };
        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U {
                width: WIN_W as u32,
                height: WIN_H as u32,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };

        if let Ok(target) = factory.CreateHwndRenderTarget(&props, &hwnd_props) {
            RENDER_TARGET = Some(target);
        }
    }

    if BRUSHES.is_some() && FONTS.is_some() {
        return;
    }

    let Some(target) = &RENDER_TARGET else { return };
    let rt: ID2D1RenderTarget = target.cast().unwrap();

    let is_dark = is_dark_mode();
    let text_col_val = if is_dark {
        COLOR_DARK_TEXT
    } else {
        COLOR_LIGHT_TEXT
    };
    let white = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: text_col_val,
                g: text_col_val,
                b: text_col_val,
                a: 1.0,
            },
            None,
        )
        .unwrap();
    let gray_col = if is_dark {
        COLOR_DARK_TEXT_SEC
    } else {
        COLOR_LIGHT_TEXT_SEC
    };
    let gray = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: gray_col,
                g: gray_col,
                b: gray_col,
                a: 1.0,
            },
            None,
        )
        .unwrap();
    let input_bg_col = if is_dark {
        COLOR_INPUT_BG_DARK
    } else {
        COLOR_INPUT_BG_LIGHT
    };
    let input_bg = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: input_bg_col,
                g: input_bg_col,
                b: input_bg_col,
                a: COLOR_INPUT_BG_OPACITY,
            },
            None,
        )
        .unwrap();
    let btn_bg_col = if is_dark {
        COLOR_DARK_BTN
    } else {
        COLOR_LIGHT_BTN
    };
    let btn_bg = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: btn_bg_col,
                g: btn_bg_col,
                b: btn_bg_col,
                a: 0.9,
            },
            None,
        )
        .unwrap();
    let btn_hover_col = if is_dark {
        COLOR_DARK_BTN_HOVER
    } else {
        COLOR_LIGHT_BTN_HOVER
    };
    let btn_hover = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: btn_hover_col,
                g: btn_hover_col,
                b: btn_hover_col,
                a: 0.95,
            },
            None,
        )
        .unwrap();
    let close_hover = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: COLOR_DESTRUCTIVE_R,
                g: COLOR_DESTRUCTIVE_G,
                b: COLOR_DESTRUCTIVE_B,
                a: 1.0,
            },
            None,
        )
        .unwrap();

    let (ar, ag, ab) = get_accent_color_values();
    let accent = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: ar,
                g: ag,
                b: ab,
                a: COLOR_ACCENT_OPACITY,
            },
            None,
        )
        .unwrap();
    let accent_hover = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: (ar + COLOR_HOVER_BRIGHTEN).min(1.0),
                g: (ag + COLOR_HOVER_BRIGHTEN).min(1.0),
                b: (ab + COLOR_HOVER_BRIGHTEN).min(1.0),
                a: 1.0,
            },
            None,
        )
        .unwrap();

    BRUSHES = Some(Brushes {
        placeholder: rt
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: text_col_val,
                    g: text_col_val,
                    b: text_col_val,
                    a: COLOR_DISABLED_OPACITY,
                },
                None,
            )
            .unwrap(),
        white,
        gray,
        input_bg,
        btn_bg,
        btn_hover,
        close_hover,
        accent,
        accent_hover,
        selection: rt
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: ar,
                    g: ag,
                    b: ab,
                    a: 0.4,
                },
                None,
            )
            .unwrap(),
        btn_border: rt
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: if is_dark {
                        COLOR_DARK_BORDER
                    } else {
                        COLOR_LIGHT_BORDER
                    },
                    g: if is_dark {
                        COLOR_DARK_BORDER
                    } else {
                        COLOR_LIGHT_BORDER
                    },
                    b: if is_dark {
                        COLOR_DARK_BORDER
                    } else {
                        COLOR_LIGHT_BORDER
                    },
                    a: COLOR_BORDER_OPACITY,
                },
                None,
            )
            .unwrap(),
    });

    if WIC_FACTORY.is_none() {
        WIC_FACTORY = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok();
    }

    if APP_ICON_BITMAP.is_none() {
        if let Some(wic_factory) = &WIC_FACTORY {
            let icon_handle = LoadImageW(
                Some(GetModuleHandleW(None).unwrap().into()),
                w!("icon.ico"),
                IMAGE_ICON,
                32,
                32,
                LR_DEFAULTCOLOR,
            );
            if let Ok(icon_handle) = icon_handle {
                if let Ok(bmp) = wic_factory.CreateBitmapFromHICON(HICON(icon_handle.0 as _)) {
                    if let Ok(converter) = wic_factory.CreateFormatConverter() {
                        let _ = converter.Initialize(
                            &bmp,
                            &GUID_WICPixelFormat32bppPBGRA,
                            WICBitmapDitherTypeNone,
                            None,
                            0.0,
                            WICBitmapPaletteTypeMedianCut,
                        );
                        if let Ok(d2d_bmp) = rt.CreateBitmapFromWicBitmap(&converter, None) {
                            APP_ICON_BITMAP = Some(d2d_bmp);
                        }
                    }
                }
            }
        }
    }
    ensure_fonts();
}

pub unsafe fn ensure_fonts() {
    if FONTS.is_some() {
        return;
    }
    let Some(dwrite) = &DWRITE_FACTORY else {
        return;
    };

    let title = dwrite
        .CreateTextFormat(
            FONT_DISPLAY,
            None,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            FONT_SZ_TITLE,
            w!(""),
        )
        .unwrap();
    let _ = title.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
    let _ = title.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR);

    let label = dwrite
        .CreateTextFormat(
            FONT_TEXT,
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            FONT_SZ_LABEL,
            w!(""),
        )
        .unwrap();
    let _ = label.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
    let _ = label.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);

    let button = dwrite
        .CreateTextFormat(
            FONT_TEXT,
            None,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            FONT_SZ_BUTTON,
            w!(""),
        )
        .unwrap();
    let _ = button.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
    let _ = button.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);

    let tooltip = dwrite
        .CreateTextFormat(
            FONT_TEXT,
            None,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            FONT_SZ_TOOLTIP,
            w!(""),
        )
        .unwrap();
    let _ = tooltip.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
    let _ = tooltip.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR);
    let _ = tooltip.SetWordWrapping(DWRITE_WORD_WRAPPING_EMERGENCY_BREAK);

    let tooltip_bold = dwrite
        .CreateTextFormat(
            FONT_TEXT,
            None,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            FONT_SZ_TOOLTIP_BOLD,
            w!(""),
        )
        .unwrap();
    let _ = tooltip_bold.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
    let _ = tooltip_bold.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR);
    let _ = tooltip_bold.SetWordWrapping(DWRITE_WORD_WRAPPING_EMERGENCY_BREAK);

    let icon = dwrite
        .CreateTextFormat(
            FONT_TEXT,
            None,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            TOOLTIP_ICON_SIZE * 0.7,
            w!(""),
        )
        .unwrap();
    let _ = icon.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
    let _ = icon.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);

    FONTS = Some(Fonts {
        title,
        label,
        button,
        tooltip,
        tooltip_bold,
        icon,
    });
}

pub unsafe fn paint() {
    let Some(target) = &RENDER_TARGET else { return };
    let Some(b) = &BRUSHES else { return };
    let Some(f) = &FONTS else { return };
    let rt: ID2D1RenderTarget = target.cast().unwrap();

    rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
    target.BeginDraw();
    target.Clear(Some(&D2D1_COLOR_F {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    }));

    let size = target.GetSize();
    let (w, _h) = (size.width, size.height);

    let min_x = w - WIN_BTN_W * 2.0;
    if HOVER == HoverId::Min {
        rt.FillRectangle(
            &D2D_RECT_F {
                left: min_x,
                top: 0.0,
                right: min_x + WIN_BTN_W,
                bottom: TITLE_BAR_H,
            },
            &b.btn_hover,
        );
    }
    let cy = TITLE_BAR_H / 2.0;
    rt.DrawLine(
        D2D_POINT_2F {
            X: min_x + 18.0,
            Y: cy,
        },
        D2D_POINT_2F {
            X: min_x + 28.0,
            Y: cy,
        },
        &b.white,
        1.0,
        None,
    );

    let close_x = w - WIN_BTN_W;
    if HOVER == HoverId::Close {
        rt.FillRectangle(
            &D2D_RECT_F {
                left: close_x,
                top: 0.0,
                right: w,
                bottom: TITLE_BAR_H,
            },
            &b.close_hover,
        );
    }
    let cx = close_x + WIN_BTN_W / 2.2;
    rt.DrawLine(
        D2D_POINT_2F {
            X: cx - 5.0,
            Y: cy - 5.0,
        },
        D2D_POINT_2F {
            X: cx + 5.0,
            Y: cy + 5.0,
        },
        &b.white,
        0.8,
        None,
    );
    rt.DrawLine(
        D2D_POINT_2F {
            X: cx + 5.0,
            Y: cy - 5.0,
        },
        D2D_POINT_2F {
            X: cx - 5.0,
            Y: cy + 5.0,
        },
        &b.white,
        0.8,
        None,
    );

    let icon_size = 24.0;
    if let Some(bitmap) = &APP_ICON_BITMAP {
        rt.DrawBitmap(
            bitmap,
            Some(&D2D_RECT_F {
                left: MARGIN - 5.0,
                top: TITLE_Y - 2.0,
                right: MARGIN - 5.0 + icon_size,
                bottom: TITLE_Y - 2.0 + icon_size,
            }),
            1.0,
            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            None,
        );
    }

    rt.DrawText(
        get_str_title(),
        &f.title,
        &D2D_RECT_F {
            left: MARGIN - 5.0 + icon_size + 8.0,
            top: TITLE_Y,
            right: 200.0,
            bottom: TITLE_Y + 20.0,
        },
        &b.white,
        D2D1_DRAW_TEXT_OPTIONS_NONE,
        DWRITE_MEASURING_MODE_NATURAL,
    );

    let input_rect = D2D_RECT_F {
        left: MARGIN,
        top: INPUT_Y,
        right: w - MARGIN,
        bottom: INPUT_Y + INPUT_H,
    };
    rt.FillRoundedRectangle(
        &D2D1_ROUNDED_RECT {
            rect: input_rect,
            radiusX: CORNER_RADIUS,
            radiusY: CORNER_RADIUS,
        },
        &b.input_bg,
    );
    rt.DrawRoundedRectangle(
        &D2D1_ROUNDED_RECT {
            rect: input_rect,
            radiusX: CORNER_RADIUS,
            radiusY: CORNER_RADIUS,
        },
        &b.accent,
        1.0,
        None,
    );

    // Search Icon (Magnifying Glass)
    let search_icon_x = MARGIN + 12.0;
    let search_icon_y = INPUT_Y + (INPUT_H - 16.0) / 2.0;
    let search_brush = &b.accent;

    rt.DrawEllipse(
        &D2D1_ELLIPSE {
            point: D2D_POINT_2F {
                X: search_icon_x + 6.0,
                Y: search_icon_y + 6.0,
            },
            radiusX: 5.0,
            radiusY: 5.0,
        },
        search_brush,
        1.5,
        None,
    );
    rt.DrawLine(
        D2D_POINT_2F {
            X: search_icon_x + 10.0,
            Y: search_icon_y + 10.0,
        },
        D2D_POINT_2F {
            X: search_icon_x + 14.0,
            Y: search_icon_y + 14.0,
        },
        search_brush,
        1.5,
        None,
    );

    if let Ok(buf) = INPUT_BUFFER.lock() {
        let text_rect = D2D_RECT_F {
            left: MARGIN + 35.0,
            top: INPUT_Y + 8.0,
            right: w - MARGIN - 30.0,
            bottom: INPUT_Y + INPUT_H - 8.0,
        };
        if buf.is_empty() {
            let hint_u16: Vec<u16> = "Search or run a command...".encode_utf16().collect();
            if let Some(dwrite) = &DWRITE_FACTORY {
                if let Ok(layout) = dwrite.CreateTextLayout(
                    &hint_u16,
                    &f.button,
                    text_rect.right - text_rect.left,
                    text_rect.bottom - text_rect.top,
                ) {
                    let _ = layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
                    rt.DrawTextLayout(
                        D2D_POINT_2F {
                            X: text_rect.left,
                            Y: text_rect.top,
                        },
                        &layout,
                        &b.placeholder,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                    );
                }
            }
        } else {
            if let Some(dwrite) = &DWRITE_FACTORY {
                let text_u16: Vec<u16> = buf.encode_utf16().collect();
                if let Ok(layout) = dwrite.CreateTextLayout(
                    &text_u16,
                    &f.button,
                    text_rect.right - text_rect.left,
                    text_rect.bottom - text_rect.top,
                ) {
                    let _ = layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
                    let lresult = SendMessageW(H_EDIT, EM_GETSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                    let (start, end) = (
                        (lresult.0 & 0xFFFF) as u32,
                        ((lresult.0 >> 16) & 0xFFFF) as u32,
                    );
                    if start != end {
                        let (mut x1, mut y1, mut m1, mut x2, mut y2, mut m2) =
                            (0.0, 0.0, std::mem::zeroed(), 0.0, 0.0, std::mem::zeroed());
                        let _ = layout.HitTestTextPosition(start, false, &mut x1, &mut y1, &mut m1);
                        let _ = layout.HitTestTextPosition(end, false, &mut x2, &mut y2, &mut m2);
                        rt.FillRectangle(
                            &D2D_RECT_F {
                                left: text_rect.left + x1,
                                top: text_rect.top,
                                right: text_rect.left + x2,
                                bottom: text_rect.bottom,
                            },
                            &b.selection,
                        );
                    }
                    rt.DrawTextLayout(
                        D2D_POINT_2F {
                            X: text_rect.left,
                            Y: text_rect.top,
                        },
                        &layout,
                        &b.white,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                    );
                }
            }
        }

        let blink_time = GetCaretBlinkTime();
        let blink_time = if blink_time == 0 { 500 } else { blink_time };
        if (GetTickCount() / blink_time) % 2 == 0 {
            let cursor_x = if buf.is_empty() {
                text_rect.left
            } else {
                if let Some(dwrite) = &DWRITE_FACTORY {
                    let text_u16: Vec<u16> = buf.encode_utf16().collect();
                    if let Ok(layout) = dwrite.CreateTextLayout(
                        &text_u16,
                        &f.button,
                        text_rect.right - text_rect.left,
                        text_rect.bottom - text_rect.top,
                    ) {
                        let _ = layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
                        let (mut start, mut end) = (0, 0);
                        SendMessageW(
                            H_EDIT,
                            EM_GETSEL,
                            Some(WPARAM(&mut start as *mut _ as usize)),
                            Some(LPARAM(&mut end as *mut _ as isize)),
                        );
                        let (mut x, mut y, mut m) = (0.0, 0.0, std::mem::zeroed());
                        let _ =
                            layout.HitTestTextPosition(end as u32, false, &mut x, &mut y, &mut m);
                        text_rect.left + x
                    } else {
                        text_rect.left
                    }
                } else {
                    text_rect.left
                }
            };
            rt.DrawLine(
                D2D_POINT_2F {
                    X: cursor_x,
                    Y: text_rect.top,
                },
                D2D_POINT_2F {
                    X: cursor_x,
                    Y: text_rect.bottom,
                },
                &b.white,
                1.5,
                None,
            );
        }
    }

    let (cx, cy) = (w - MARGIN - 20.0, INPUT_Y + INPUT_H / 2.0);
    rt.DrawLine(
        D2D_POINT_2F {
            X: cx - 4.0,
            Y: cy - 2.0,
        },
        D2D_POINT_2F { X: cx, Y: cy + 2.0 },
        &b.white,
        1.0,
        None,
    );
    rt.DrawLine(
        D2D_POINT_2F { X: cx, Y: cy + 2.0 },
        D2D_POINT_2F {
            X: cx + 4.0,
            Y: cy - 2.0,
        },
        &b.white,
        1.0,
        None,
    );

    let ok_x = w - MARGIN - BTN_W * 2.0 - 8.0;
    draw_button(
        &rt,
        b,
        f,
        ok_x,
        get_str_run(),
        HoverId::Ok,
        is_input_empty(),
    );
    draw_button(
        &rt,
        b,
        f,
        w - MARGIN - BTN_W,
        get_str_cancel(),
        HoverId::Cancel,
        false,
    );

    let _ = target.EndDraw(None, None);
}

pub unsafe fn draw_button(
    rt: &ID2D1RenderTarget,
    b: &Brushes,
    f: &Fonts,
    x: f32,
    text: &[u16],
    id: HoverId,
    disabled: bool,
) {
    let rect = D2D_RECT_F {
        left: x,
        top: BTN_Y,
        right: x + BTN_W,
        bottom: BTN_Y + BTN_H,
    };
    let bg = if disabled {
        &b.input_bg
    } else if id == HoverId::Ok {
        if HOVER == id {
            &b.accent_hover
        } else {
            &b.accent
        }
    } else if HOVER == id {
        &b.btn_hover
    } else {
        &b.btn_bg
    };
    rt.FillRoundedRectangle(
        &D2D1_ROUNDED_RECT {
            rect,
            radiusX: CORNER_RADIUS,
            radiusY: CORNER_RADIUS,
        },
        bg,
    );
    rt.DrawRoundedRectangle(
        &D2D1_ROUNDED_RECT {
            rect,
            radiusX: CORNER_RADIUS,
            radiusY: CORNER_RADIUS,
        },
        &b.btn_border,
        1.0,
        None,
    );
    let brush = if disabled { &b.gray } else { &b.white };
    rt.DrawText(
        text,
        &f.button,
        &rect,
        brush,
        D2D1_DRAW_TEXT_OPTIONS_NONE,
        DWRITE_MEASURING_MODE_NATURAL,
    );
}
