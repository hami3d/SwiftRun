use std::time::Instant;
use windows::Win32::Foundation::*;
use windows::core::*;
use windows_numerics::Matrix3x2;

use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::animations::*;
use crate::config::*;
use crate::data::history::*;
use crate::ui::resources::*;
use crate::ui::*;

pub static mut SHOW_DROPDOWN: bool = false;
pub static mut SCROLL_OFFSET: usize = 0;
pub static mut DROPDOWN_RENDER_TARGET: Option<ID2D1HwndRenderTarget> = None;
pub static mut HOVER_DROPDOWN: Option<usize> = None;

pub unsafe fn ensure_dropdown_resources(hwnd: HWND) {
    if DROPDOWN_RENDER_TARGET.is_none() {
        let Some(factory) = &D2D_FACTORY else { return };

        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);
        let w = (rect.right - rect.left) as u32;
        let h = (rect.bottom - rect.top) as u32;

        if w == 0 || h == 0 {
            return;
        }

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
                width: w,
                height: h,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };

        if let Ok(target) = factory.CreateHwndRenderTarget(&props, &hwnd_props) {
            DROPDOWN_RENDER_TARGET = Some(target);
        }
    } else {
        if let Some(target) = &DROPDOWN_RENDER_TARGET {
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            let w = (rect.right - rect.left) as u32;
            let h = (rect.bottom - rect.top) as u32;

            let size = target.GetPixelSize();
            if size.width != w || size.height != h {
                let res = target.Resize(&D2D_SIZE_U {
                    width: w,
                    height: h,
                });
                if res.is_ok() {
                    DROPDOWN_BRUSHES = None;
                } else {
                    DROPDOWN_RENDER_TARGET = None;
                    DROPDOWN_BRUSHES = None;
                }
            }
        }
    }

    if DROPDOWN_BRUSHES.is_some() {
        return;
    }

    let Some(target) = &DROPDOWN_RENDER_TARGET else {
        return;
    };
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
        COLOR_DARK_BORDER
    } else {
        COLOR_LIGHT_BORDER
    };
    let gray = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: gray_col,
                g: gray_col,
                b: gray_col,
                a: 0.5,
            },
            None,
        )
        .unwrap();
    let input_bg = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.1,
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
                r: 0.769,
                g: 0.169,
                b: 0.11,
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

    DROPDOWN_BRUSHES = Some(Brushes {
        placeholder: rt
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: text_col_val,
                    g: text_col_val,
                    b: text_col_val,
                    a: 0.4,
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
                    r: gray_col,
                    g: gray_col,
                    b: gray_col,
                    a: COLOR_BORDER_OPACITY,
                },
                None,
            )
            .unwrap(),
    });
}

pub unsafe extern "system" fn dropdown_wndproc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            set_acrylic_effect(hwnd);
            let v: i32 = 2; // DWMWCP_ROUND
            let _ = DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(33), &v as *const _ as _, 4);
            LRESULT(0)
        }
        WM_SIZE => {
            DROPDOWN_BRUSHES = None;
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_SHOWWINDOW => {
            if wp.0 == 1 {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_SETTINGCHANGE => {
            set_acrylic_effect(hwnd);
            DROPDOWN_BRUSHES = None;
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);
            ensure_dropdown_resources(hwnd);

            if let Some(target) = &DROPDOWN_RENDER_TARGET {
                if let Some(b) = &DROPDOWN_BRUSHES {
                    if let Some(f) = &FONTS {
                        if let Ok(rt) = target.cast::<ID2D1RenderTarget>() {
                            rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                            target.BeginDraw();

                            let size = target.GetSize();
                            let w = size.width;
                            let h = size.height;

                            let mut alpha = 1.0;
                            let mut y_off = 0.0;
                            if let Some(start) = DROPDOWN_ANIM_START {
                                let elapsed = start.elapsed().as_millis();
                                let progress =
                                    (elapsed as f32 / ANIM_DROPDOWN_DURATION_MS as f32).min(1.0);
                                match DROPDOWN_ANIM_TYPE {
                                    AnimType::Entering => {
                                        alpha = ease_out_cubic(progress);
                                        y_off = -10.0 * (1.0 - alpha);
                                    }
                                    AnimType::Exiting => {
                                        alpha = 1.0 - ease_out_cubic(progress);
                                        y_off = -5.0 * (1.0 - alpha);
                                    }
                                    _ => {}
                                }
                            }

                            target.Clear(Some(&D2D1_COLOR_F {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.0,
                            }));

                            let layer_params = D2D1_LAYER_PARAMETERS {
                                contentBounds: D2D_RECT_F {
                                    left: 0.0,
                                    top: 0.0,
                                    right: w as f32,
                                    bottom: h as f32,
                                },
                                opacity: alpha,
                                ..Default::default()
                            };

                            if let Ok(layer) = target.CreateLayer(None) {
                                target.PushLayer(&layer_params, &layer);
                            }

                            rt.SetTransform(&Matrix3x2::translation(0.0, y_off));

                            rt.FillRoundedRectangle(
                                &D2D1_ROUNDED_RECT {
                                    rect: D2D_RECT_F {
                                        left: 0.0,
                                        top: 0.0,
                                        right: w,
                                        bottom: h,
                                    },
                                    radiusX: CORNER_RADIUS,
                                    radiusY: CORNER_RADIUS,
                                },
                                &b.input_bg,
                            );

                            if let Some(history) = HISTORY.as_ref() {
                                for (i, item) in history
                                    .iter()
                                    .skip(SCROLL_OFFSET)
                                    .take(DROPDOWN_MAX_ITEMS)
                                    .enumerate()
                                {
                                    let item_y = i as f32 * ITEM_H;
                                    let total_items = history.len();
                                    let scroll_width = if total_items > DROPDOWN_MAX_ITEMS {
                                        8.0
                                    } else {
                                        0.0
                                    };

                                    let rect = D2D_RECT_F {
                                        left: 0.0,
                                        top: item_y,
                                        right: w - scroll_width,
                                        bottom: item_y + ITEM_H,
                                    };

                                    if HOVER_DROPDOWN == Some(i) {
                                        rt.FillRoundedRectangle(
                                            &D2D1_ROUNDED_RECT {
                                                rect,
                                                radiusX: CORNER_RADIUS,
                                                radiusY: CORNER_RADIUS,
                                            },
                                            &b.btn_hover,
                                        );
                                    }

                                    let txt: Vec<u16> = item.encode_utf16().collect();
                                    rt.DrawText(
                                        &txt,
                                        &f.label,
                                        &D2D_RECT_F {
                                            left: rect.left + 10.0,
                                            top: rect.top,
                                            right: rect.right - 10.0,
                                            bottom: rect.bottom,
                                        },
                                        &b.white,
                                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                                        DWRITE_MEASURING_MODE_NATURAL,
                                    );
                                }

                                let total_items = history.len();
                                if total_items > DROPDOWN_MAX_ITEMS {
                                    let ratio = DROPDOWN_MAX_ITEMS as f32 / total_items as f32;
                                    let thumb_h = h * ratio;
                                    let thumb_y = (SCROLL_OFFSET as f32 / total_items as f32) * h;
                                    let scroll_rect = D2D_RECT_F {
                                        left: w - 6.0,
                                        top: thumb_y + 2.0,
                                        right: w - 2.0,
                                        bottom: thumb_y + thumb_h - 2.0,
                                    };
                                    rt.FillRoundedRectangle(
                                        &D2D1_ROUNDED_RECT {
                                            rect: scroll_rect,
                                            radiusX: 2.0,
                                            radiusY: 2.0,
                                        },
                                        &b.gray,
                                    );
                                }
                            }

                            target.PopLayer();
                            rt.SetTransform(&Matrix3x2::identity());

                            let result = target.EndDraw(None, None);
                            if let Err(e) = result {
                                if e.code() == HRESULT(0x8899000Cu32 as i32) {
                                    DROPDOWN_RENDER_TARGET = None;
                                    DROPDOWN_BRUSHES = None;
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                }
                            }
                        }
                    }
                }
            }
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            if let Some(idx) = HOVER_DROPDOWN {
                if let Some(history) = HISTORY.as_ref() {
                    if let Some(cmd) = history.get(SCROLL_OFFSET + idx) {
                        if let Ok(mut lock) = INPUT_BUFFER.lock() {
                            *lock = cmd.clone();
                        }
                        let u16_vec: Vec<u16> =
                            cmd.encode_utf16().chain(std::iter::once(0)).collect();
                        let _ = SetWindowTextW(H_EDIT, PCWSTR(u16_vec.as_ptr()));

                        SHOW_DROPDOWN = false;
                        if let Ok(main_hwnd) = FindWindowW(w!("SwiftRunClass"), w!("SwiftRun")) {
                            DROPDOWN_ANIM_START = Some(Instant::now());
                            DROPDOWN_ANIM_TYPE = AnimType::Exiting;
                            SetTimer(Some(main_hwnd), 3, 16, None);
                        }
                        let _ = SetFocus(Some(H_EDIT));
                        let _ = SendMessageW(
                            H_EDIT,
                            windows::Win32::UI::Controls::EM_SETSEL,
                            Some(WPARAM(0)),
                            Some(LPARAM(-1)),
                        );
                    }
                }
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let y = (lp.0 >> 16) as i16 as f32;
            let idx = (y / ITEM_H) as usize;
            if idx < DROPDOWN_MAX_ITEMS {
                if HOVER_DROPDOWN != Some(idx) {
                    HOVER_DROPDOWN = Some(idx);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            } else if HOVER_DROPDOWN.is_some() {
                HOVER_DROPDOWN = None;
                let _ = InvalidateRect(Some(hwnd), None, false);
            }

            let mut tme = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            let _ = TrackMouseEvent(&mut tme);
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            HOVER_DROPDOWN = None;
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = (wp.0 >> 16) as i16;
            if let Some(history) = HISTORY.as_ref() {
                if history.len() > DROPDOWN_MAX_ITEMS {
                    if delta > 0 {
                        if SCROLL_OFFSET > 0 {
                            SCROLL_OFFSET -= 1;
                        }
                    } else {
                        if SCROLL_OFFSET < history.len() - DROPDOWN_MAX_ITEMS {
                            SCROLL_OFFSET += 1;
                        }
                    }
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
