use windows::core::*;
use windows::Win32::Foundation::*;
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

use crate::ui::resources::*;
use crate::ui::*;

pub static mut DIALOG_TITLE: String = String::new();
pub static mut DIALOG_MESSAGE: String = String::new();
pub static mut DIALOG_HOVER_OK: bool = false;
pub static mut DIALOG_ACTIVE: bool = false;

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

                let padding = 22.0;
                let title_height = 15.0;
                let button_w = 160.0;
                let button_h = 38.0;

                let scale = get_dpi_scale(hwnd);
                let mut cr = RECT::default();
                GetClientRect(hwnd, &mut cr);
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
                    let text_color = if is_dark {
                        D2D1_COLOR_F {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }
                    } else {
                        D2D1_COLOR_F {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }
                    };
                    let brush = rt.CreateSolidColorBrush(&text_color, None).unwrap();

                    let title_format = DWRITE_FACTORY
                        .as_ref()
                        .unwrap()
                        .CreateTextFormat(
                            w!("Segoe UI Variable Display"),
                            None,
                            DWRITE_FONT_WEIGHT_BOLD,
                            DWRITE_FONT_STYLE_NORMAL,
                            DWRITE_FONT_STRETCH_NORMAL,
                            20.0,
                            w!("en-us"),
                        )
                        .unwrap();

                    let ok_text_format = DWRITE_FACTORY
                        .as_ref()
                        .unwrap()
                        .CreateTextFormat(
                            w!("Segoe UI Variable Display"),
                            None,
                            DWRITE_FONT_WEIGHT_REGULAR,
                            DWRITE_FONT_STYLE_NORMAL,
                            DWRITE_FONT_STRETCH_NORMAL,
                            15.0,
                            w!("en-us"),
                        )
                        .unwrap();
                    let _ = ok_text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                    let _ = ok_text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);

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

                    let msg_format = DWRITE_FACTORY
                        .as_ref()
                        .unwrap()
                        .CreateTextFormat(
                            w!("Segoe UI Variable Small"),
                            None,
                            DWRITE_FONT_WEIGHT_NORMAL,
                            DWRITE_FONT_STYLE_NORMAL,
                            DWRITE_FONT_STRETCH_NORMAL,
                            15.0,
                            w!("en-us"),
                        )
                        .unwrap();

                    let msg_u16: Vec<u16> = DIALOG_MESSAGE.encode_utf16().collect();
                    rt.DrawText(
                        &msg_u16,
                        &msg_format,
                        &D2D_RECT_F {
                            left: padding,
                            top: padding + title_height + 10.0,
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
                        if is_dark {
                            D2D1_COLOR_F {
                                r: 0.3,
                                g: 0.3,
                                b: 0.3,
                                a: 0.8,
                            }
                        } else {
                            D2D1_COLOR_F {
                                r: 0.9,
                                g: 0.9,
                                b: 0.9,
                                a: 0.8,
                            }
                        }
                    } else {
                        if is_dark {
                            D2D1_COLOR_F {
                                r: 0.2,
                                g: 0.2,
                                b: 0.2,
                                a: 0.5,
                            }
                        } else {
                            D2D1_COLOR_F {
                                r: 1.0,
                                g: 1.0,
                                b: 1.0,
                                a: 0.5,
                            }
                        }
                    };
                    let btn_brush = rt.CreateSolidColorBrush(&btn_bg, None).unwrap();
                    rt.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: btn_rect,
                            radiusX: 4.0,
                            radiusY: 4.0,
                        },
                        &btn_brush,
                    );

                    let border_val = if is_dark { 0.6 } else { 0.4 };
                    let border_brush = rt
                        .CreateSolidColorBrush(
                            &D2D1_COLOR_F {
                                r: border_val,
                                g: border_val,
                                b: border_val,
                                a: 0.1,
                            },
                            None,
                        )
                        .unwrap();
                    rt.DrawRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: btn_rect,
                            radiusX: 4.0,
                            radiusY: 4.0,
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
                EndPaint(hwnd, &ps);
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
                GetClientRect(hwnd, &mut cr);
                let w = (cr.right - cr.left) as f32 / scale;
                let h = (cr.bottom - cr.top) as f32 / scale;

                let padding = 22.0;
                let button_w = 160.0;
                let button_h = 38.0;

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
                GetClientRect(hwnd, &mut cr);
                let w = (cr.right - cr.left) as f32 / scale;
                let h = (cr.bottom - cr.top) as f32 / scale;

                let padding = 22.0;
                let button_w = 160.0;
                let button_h = 38.0;

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
                if inside != DIALOG_HOVER_OK {
                    DIALOG_HOVER_OK = inside;
                    let _ = InvalidateRect(hwnd, None, BOOL(0));
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

    let w = 620;
    let h = 210;
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
        instance,
        None,
    );

    if _hwnd.0 != 0 {
        let _ = SetForegroundWindow(_hwnd);
        let _ = SetFocus(_hwnd);
    }

    let mut msg = MSG::default();
    while DIALOG_ACTIVE {
        if GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        } else {
            break;
        }
    }
}
