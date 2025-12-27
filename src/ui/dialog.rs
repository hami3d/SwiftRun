#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]
#![allow(non_snake_case)]

use windows::Win32::Foundation::*;
use windows::core::*;

use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::*;
use crate::ui::resources::*;
use crate::ui::*;

pub static mut DIALOG_TITLE: String = String::new();
pub static mut DIALOG_MESSAGE: String = String::new();
pub static mut DIALOG_HOVER_OK: bool = false;
pub static mut DIALOG_ACTIVE: bool = false;
static mut MOUSE_TRACKED: bool = false;

pub extern "system" fn dialog_wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let v: i32 = 1;
                let _ = DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(20), &v as *const _ as _, 4);
                set_acrylic_effect(hwnd);
                let v: i32 = 2; // Round corners
                let _ = DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(33), &v as *const _ as _, 4);
                let m = MARGINS {
                    cxLeftWidth: -1,
                    cxRightWidth: -1,
                    cyTopHeight: -1,
                    cyBottomHeight: -1,
                };
                let _ = DwmExtendFrameIntoClientArea(hwnd, &m);

                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);

                let backdrop: u32 = 3; // Acrylic
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWINDOWATTRIBUTE(38),
                    &backdrop as *const _ as _,
                    4,
                );
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut ps);

                let padding = DIALOG_PADDING;
                // let title_height = 15.0; // Removed title
                let button_w = DIALOG_BTN_W;
                let button_h = DIALOG_BTN_H;

                let scale = get_dpi_scale(hwnd);
                let mut cr = RECT::default();
                let _ = GetClientRect(hwnd, &mut cr);
                let w = (cr.right - cr.left) as f32 / scale;
                let h = (cr.bottom - cr.top) as f32 / scale;

                if let Some(factory) = &D2D_FACTORY {
                    let mut props = D2D1_RENDER_TARGET_PROPERTIES::default();
                    props.pixelFormat = D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    };

                    let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                        hwnd,
                        pixelSize: D2D_SIZE_U {
                            width: (cr.right - cr.left) as u32,
                            height: (cr.bottom - cr.top) as u32,
                        },
                        presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                    };
                    let rt = factory.CreateHwndRenderTarget(&props, &hwnd_props).unwrap();

                    rt.BeginDraw();
                    rt.Clear(Some(&D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }));

                    let is_dark = is_dark_mode();
                    let text_val = if is_dark {
                        COLOR_DARK_TEXT
                    } else {
                        COLOR_LIGHT_TEXT
                    };
                    let text_color = D2D1_COLOR_F {
                        r: text_val,
                        g: text_val,
                        b: text_val,
                        a: 1.0,
                    };
                    let brush = rt.CreateSolidColorBrush(&text_color, None).unwrap();

                    /*
                    let title_format = DWRITE_FACTORY
                        .as_ref()
                        .unwrap()
                        .CreateTextFormat(
                            FONT_DISPLAY,
                            None,
                            DWRITE_FONT_WEIGHT_BOLD,
                            DWRITE_FONT_STYLE_NORMAL,
                            DWRITE_FONT_STRETCH_NORMAL,
                            FONT_SZ_DIALOG_TITLE,
                            w!("en-us"),
                        )
                        .unwrap();
                    */

                    let ok_text_format = DWRITE_FACTORY
                        .as_ref()
                        .unwrap()
                        .CreateTextFormat(
                            FONT_DISPLAY,
                            None,
                            DWRITE_FONT_WEIGHT_REGULAR,
                            DWRITE_FONT_STYLE_NORMAL,
                            DWRITE_FONT_STRETCH_NORMAL,
                            FONT_SZ_DIALOG_BTN,
                            w!("en-us"),
                        )
                        .unwrap();
                    let _ = ok_text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                    let _ = ok_text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);

                    /*
                    let title_u16: Vec<u16> = DIALOG_TITLE.encode_utf16().collect();
                    rt.DrawText(
                        &title_u16,
                        &title_format,
                        &D2D_RECT_F {
                            left: padding,
                            top: 20.0,
                            right: w - padding,
                            bottom: padding + title_height,
                        },
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                    */

                    let msg_format = DWRITE_FACTORY
                        .as_ref()
                        .unwrap()
                        .CreateTextFormat(
                            FONT_SMALL,
                            None,
                            DWRITE_FONT_WEIGHT_NORMAL,
                            DWRITE_FONT_STYLE_NORMAL,
                            DWRITE_FONT_STRETCH_NORMAL,
                            FONT_SZ_DIALOG_MSG,
                            w!("en-us"),
                        )
                        .unwrap();

                    let msg_u16: Vec<u16> = DIALOG_MESSAGE.encode_utf16().collect();
                    rt.DrawText(
                        &msg_u16,
                        &msg_format,
                        &D2D_RECT_F {
                            left: padding,
                            top: padding + 5.0,
                            right: w - padding,
                            bottom: h - padding - button_h - 10.0,
                        },
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );

                    let btn_rect = D2D_RECT_F {
                        left: w - padding - button_w,
                        top: h - padding - button_h,
                        right: w - padding,
                        bottom: h - padding,
                    };
                    let btn_bg = if DIALOG_HOVER_OK {
                        let c = if is_dark {
                            COLOR_DARK_BTN_HOVER
                        } else {
                            COLOR_LIGHT_BTN_HOVER
                        };
                        D2D1_COLOR_F {
                            r: c,
                            g: c,
                            b: c,
                            a: 0.8,
                        }
                    } else {
                        let c = if is_dark {
                            COLOR_DARK_BTN
                        } else {
                            COLOR_LIGHT_BTN
                        };
                        D2D1_COLOR_F {
                            r: c,
                            g: c,
                            b: c,
                            a: 0.5,
                        }
                    };
                    let btn_brush = rt.CreateSolidColorBrush(&btn_bg, None).unwrap();
                    rt.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: btn_rect,
                            radiusX: CORNER_RADIUS,
                            radiusY: CORNER_RADIUS,
                        },
                        &btn_brush,
                    );

                    let border_val = if is_dark {
                        COLOR_DARK_BORDER
                    } else {
                        COLOR_LIGHT_BORDER
                    };
                    let border_brush = rt
                        .CreateSolidColorBrush(
                            &D2D1_COLOR_F {
                                r: border_val,
                                g: border_val,
                                b: border_val,
                                a: COLOR_BORDER_OPACITY,
                            },
                            None,
                        )
                        .unwrap();
                    rt.DrawRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: btn_rect,
                            radiusX: CORNER_RADIUS,
                            radiusY: CORNER_RADIUS,
                        },
                        &border_brush,
                        1.0,
                        None,
                    );

                    let ok_text = [b'O' as u16, b'K' as u16];
                    rt.DrawText(
                        &ok_text,
                        &ok_text_format,
                        &btn_rect,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                    let _ = rt.EndDraw(None, None);
                }
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_NCHITTEST => {
                let x = (lp.0 & 0xFFFF) as i16 as i32;
                let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;
                let mut p = POINT { x, y };
                let _ = ScreenToClient(hwnd, &mut p);

                let scale = get_dpi_scale(hwnd);
                let sx = p.x as f32 / scale;
                let sy = p.y as f32 / scale;

                let mut cr = RECT::default();
                let _ = GetClientRect(hwnd, &mut cr);
                let w = (cr.right - cr.left) as f32 / scale;
                let h = (cr.bottom - cr.top) as f32 / scale;

                let padding = DIALOG_PADDING;
                let button_w = DIALOG_BTN_W;
                let button_h = DIALOG_BTN_H;

                let btn_rect = D2D_RECT_F {
                    left: w - padding - button_w,
                    top: h - padding - button_h,
                    right: w - padding,
                    bottom: h - padding,
                };

                if sx >= btn_rect.left
                    && sx <= btn_rect.right
                    && sy >= btn_rect.top
                    && sy <= btn_rect.bottom
                {
                    LRESULT(HTCLIENT as isize)
                } else {
                    LRESULT(HTCAPTION as isize)
                }
            }
            WM_MOUSEMOVE => {
                let x = (lp.0 & 0xFFFF) as i16 as f32;
                let y = ((lp.0 >> 16) & 0xFFFF) as i16 as f32;
                let scale = get_dpi_scale(hwnd);
                let sx = x / scale;
                let sy = y / scale;

                let mut cr = RECT::default();
                let _ = GetClientRect(hwnd, &mut cr);
                let w = (cr.right - cr.left) as f32 / scale;
                let h = (cr.bottom - cr.top) as f32 / scale;

                let padding = DIALOG_PADDING;
                let button_w = DIALOG_BTN_W;
                let button_h = DIALOG_BTN_H;

                let btn_rect = D2D_RECT_F {
                    left: w - padding - button_w,
                    top: h - padding - button_h,
                    right: w - padding,
                    bottom: h - padding,
                };

                let inside = sx >= btn_rect.left
                    && sx <= btn_rect.right
                    && sy >= btn_rect.top
                    && sy <= btn_rect.bottom;

                if !MOUSE_TRACKED {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                    MOUSE_TRACKED = true;
                }

                if inside != DIALOG_HOVER_OK {
                    DIALOG_HOVER_OK = inside;
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                MOUSE_TRACKED = false;
                if DIALOG_HOVER_OK {
                    DIALOG_HOVER_OK = false;
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                if DIALOG_HOVER_OK {
                    DIALOG_ACTIVE = false;
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                let vk = wp.0 as u32;
                if vk == VK_RETURN.0 as u32 || vk == VK_ESCAPE.0 as u32 {
                    DIALOG_ACTIVE = false;
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                DIALOG_ACTIVE = false;
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

pub unsafe fn show_fluent_dialog(title: &str, message: &str) {
    DIALOG_TITLE = title.to_string();
    DIALOG_MESSAGE = message.to_string();
    DIALOG_ACTIVE = true;
    DIALOG_HOVER_OK = false;

    let instance = GetModuleHandleW(None).unwrap();
    let screen_w = GetSystemMetrics(SM_CXSCREEN);
    let screen_h = GetSystemMetrics(SM_CYSCREEN);

    let w = DIALOG_W;
    let h = DIALOG_H;
    let x = (screen_w - w) / 2;
    let y = (screen_h - h) / 2;

    let _hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
        w!("SwiftDialog"),
        w!(""),
        WS_POPUP | WS_VISIBLE,
        x,
        y,
        w,
        h,
        None,
        None,
        Some(instance.into()),
        None,
    )
    .unwrap_or(HWND(std::ptr::null_mut()));

    if !_hwnd.0.is_null() {
        let _ = SetForegroundWindow(_hwnd);
        let _ = SetFocus(Some(_hwnd));
    }

    let mut msg = MSG::default();
    while DIALOG_ACTIVE {
        if GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        } else {
            break;
        }
    }
}
