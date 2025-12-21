use std::time::Instant;
use windows::Win32::Foundation::*;
use windows::core::*;
use windows_numerics::Matrix3x2;

use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows_numerics::Vector2 as D2D_POINT_2F;

use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::animations::*;
use crate::config::*;
use crate::ui::resources::*;
use crate::ui::*;

pub static mut TOOLTIP_TITLE: String = String::new();
pub static mut TOOLTIP_MESSAGE: String = String::new();

pub unsafe fn show_tooltip(title: &str, msg: &str) {
    if !H_TOOLTIP.0.is_null() {
        let _ = DestroyWindow(H_TOOLTIP);
    }
    H_TOOLTIP = HWND(std::ptr::null_mut());
    TOOLTIP_RENDER_TARGET = None;

    TOOLTIP_TITLE = title.to_string();
    TOOLTIP_MESSAGE = msg.to_string();

    let main_hwnd = H_MAIN;
    let mut main_rect = RECT::default();
    let _ = GetWindowRect(main_hwnd, &mut main_rect);

    let dpi_scale = get_dpi_scale(main_hwnd);
    let width = (TOOLTIP_W * dpi_scale) as i32;
    let height = (TOOLTIP_H * dpi_scale) as i32;

    let x = main_rect.left + (MARGIN * dpi_scale) as i32;
    let input_y_screen = main_rect.top + ((INPUT_Y * dpi_scale) as i32);
    let y = input_y_screen - height - (TOOLTIP_GAP * dpi_scale) as i32;

    let instance = GetModuleHandleW(None).unwrap();
    H_TOOLTIP = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        w!("SwiftRunTooltip"),
        w!(""),
        WS_POPUP,
        x,
        y,
        width,
        height,
        Some(main_hwnd),
        None,
        Some(instance.into()),
        None,
    )
    .unwrap_or(HWND(std::ptr::null_mut()));

    let v: i32 = 1; // DWMWCP_DONOTROUND
    let _ = DwmSetWindowAttribute(H_TOOLTIP, DWMWINDOWATTRIBUTE(33), &v as *const _ as _, 4);

    // Enable transparency without blur to avoid ghost window
    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    let _ = DwmExtendFrameIntoClientArea(H_TOOLTIP, &margins);

    let _ = ShowWindow(H_TOOLTIP, SW_SHOWNOACTIVATE);

    TOOLTIP_ANIM_START = Some(Instant::now());
    TOOLTIP_ANIM_TYPE = AnimType::Entering;
    SetTimer(Some(H_MAIN), 3, ANIM_TIMER_MS, None);

    SetTimer(Some(H_TOOLTIP), 2, 8000, None);

    if !H_EDIT.0.is_null() {
        let _ = SetFocus(Some(H_EDIT));
        SendMessageW(
            H_EDIT,
            windows::Win32::UI::Controls::EM_SETSEL,
            Some(WPARAM(0)),
            Some(LPARAM(-1)),
        );
    }
    let _ = InvalidateRect(Some(H_TOOLTIP), None, false);
}

pub unsafe extern "system" fn tooltip_wndproc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            if wp.0 == 2 {
                TOOLTIP_ANIM_START = Some(Instant::now());
                TOOLTIP_ANIM_TYPE = AnimType::Exiting;
                SetTimer(Some(H_MAIN), 3, ANIM_TIMER_MS, None);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            TOOLTIP_ANIM_START = Some(Instant::now());
            TOOLTIP_ANIM_TYPE = AnimType::Exiting;
            SetTimer(Some(H_MAIN), 3, ANIM_TIMER_MS, None);
            LRESULT(0)
        }
        WM_DESTROY => {
            TOOLTIP_RENDER_TARGET = None;
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            let factory = D2D_FACTORY.as_ref().unwrap();

            if !H_TOOLTIP.0.is_null() {
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let w = (rect.right - rect.left) as u32;
                let h = (rect.bottom - rect.top) as u32;

                if w > 0 && h > 0 {
                    if TOOLTIP_RENDER_TARGET.is_none() {
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
                            TOOLTIP_RENDER_TARGET = Some(target);
                        }
                    }

                    if let Some(target) = &TOOLTIP_RENDER_TARGET {
                        target.BeginDraw();
                        let rt: ID2D1RenderTarget = target.cast().unwrap();

                        let size = target.GetSize();
                        let (w_dip, h_dip) = (size.width, size.height);

                        let mut alpha = 1.0;
                        let mut scale = 1.0;
                        let mut y_off = 0.0;
                        if let Some(start) = TOOLTIP_ANIM_START {
                            let elapsed = start.elapsed().as_millis();
                            let progress =
                                (elapsed as f32 / ANIM_TOOLTIP_DURATION_MS as f32).min(1.0);
                            match TOOLTIP_ANIM_TYPE {
                                AnimType::Entering => {
                                    let eased = ease_out_back(progress);
                                    alpha = progress.min(1.0);
                                    scale = 0.8 + 0.2 * eased;
                                    y_off = 5.0 * (1.0 - eased);
                                }
                                AnimType::Exiting => {
                                    alpha = 1.0 - progress;
                                    scale = 1.0 - 0.1 * progress;
                                    y_off = -5.0 * progress;
                                }
                                _ => {}
                            }
                        }

                        rt.Clear(Some(&D2D1_COLOR_F {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }));

                        let mid_x = w_dip / 2.0;
                        let mid_y = h_dip / 2.0;

                        let translate = Matrix3x2::translation(-mid_x, -mid_y);
                        let scale_mat = Matrix3x2 {
                            M11: scale,
                            M12: 0.0,
                            M21: 0.0,
                            M22: scale,
                            M31: 0.0,
                            M32: 0.0,
                        };
                        let translate_back = Matrix3x2::translation(mid_x, mid_y + y_off);

                        rt.SetTransform(&(translate * scale_mat * translate_back));

                        let layer_params = D2D1_LAYER_PARAMETERS {
                            contentBounds: D2D_RECT_F {
                                left: 0.0,
                                top: 0.0,
                                right: w_dip,
                                bottom: h_dip,
                            },
                            opacity: alpha,
                            ..Default::default()
                        };
                        let mut pushed_layer = false;
                        if let Ok(layer) = rt.CreateLayer(None) {
                            rt.PushLayer(&layer_params, &layer);
                            pushed_layer = true;
                        }

                        let is_dark = is_dark_mode();
                        let bg_col = if is_dark {
                            COLOR_DARK_BG
                        } else {
                            COLOR_LIGHT_BG
                        };
                        let bg_alpha = if is_dark { 0.95 } else { 0.98 };

                        let (ar, ag, ab) = get_accent_color_values();
                        let accent_col = D2D1_COLOR_F {
                            r: ar,
                            g: ag,
                            b: ab,
                            a: 1.0,
                        };
                        let accent_brush = rt.CreateSolidColorBrush(&accent_col, None).unwrap();
                        let white_brush = rt
                            .CreateSolidColorBrush(
                                &D2D1_COLOR_F {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 1.0,
                                },
                                None,
                            )
                            .unwrap();

                        let tri_h = TOOLTIP_TRI_H;
                        let main_rect = D2D_RECT_F {
                            left: 0.0,
                            top: 0.0,
                            right: w_dip,
                            bottom: h_dip - tri_h,
                        };

                        if let Ok(bg_brush) = rt.CreateSolidColorBrush(
                            &D2D1_COLOR_F {
                                r: bg_col,
                                g: bg_col,
                                b: bg_col,
                                a: bg_alpha,
                            },
                            None,
                        ) {
                            rt.FillRoundedRectangle(
                                &D2D1_ROUNDED_RECT {
                                    rect: main_rect,
                                    radiusX: TOOLTIP_CORNER_RADIUS,
                                    radiusY: TOOLTIP_CORNER_RADIUS,
                                },
                                &bg_brush,
                            );

                            // Triangle at bottom left
                            if let Ok(path) = factory.CreatePathGeometry() {
                                if let Ok(sink) = path.Open() {
                                    sink.BeginFigure(
                                        D2D_POINT_2F {
                                            X: 20.0,
                                            Y: h_dip - tri_h,
                                        },
                                        D2D1_FIGURE_BEGIN_FILLED,
                                    );
                                    sink.AddLine(D2D_POINT_2F {
                                        X: 20.0 + TOOLTIP_TRI_W,
                                        Y: h_dip - tri_h,
                                    });
                                    sink.AddLine(D2D_POINT_2F { X: 20.0, Y: h_dip });
                                    sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                                    let _ = sink.Close();
                                    rt.FillGeometry(&path, &bg_brush, None);
                                    rt.DrawGeometry(&path, &accent_brush, 1.5, None);
                                }
                            }
                        }

                        rt.DrawRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: main_rect,
                                radiusX: TOOLTIP_CORNER_RADIUS,
                                radiusY: TOOLTIP_CORNER_RADIUS,
                            },
                            &accent_brush,
                            1.5,
                            None,
                        );

                        // Icon: Red Circle with '!'
                        let icon_center = D2D_POINT_2F {
                            X: TOOLTIP_PADDING + TOOLTIP_ICON_SIZE / 2.0,
                            Y: TOOLTIP_PADDING + TOOLTIP_ICON_SIZE / 2.0,
                        };
                        if let Ok(red_brush) = rt.CreateSolidColorBrush(
                            &D2D1_COLOR_F {
                                r: COLOR_WARNING_R,
                                g: COLOR_WARNING_G,
                                b: COLOR_WARNING_B,
                                a: 1.0,
                            },
                            None,
                        ) {
                            rt.FillEllipse(
                                &D2D1_ELLIPSE {
                                    point: icon_center,
                                    radiusX: TOOLTIP_ICON_SIZE / 2.0,
                                    radiusY: TOOLTIP_ICON_SIZE / 2.0,
                                },
                                &red_brush,
                            );
                        }

                        if let Some(f) = &FONTS {
                            let excl = [b'!' as u16];
                            let excl_rect = D2D_RECT_F {
                                left: TOOLTIP_PADDING,
                                top: TOOLTIP_PADDING,
                                right: TOOLTIP_PADDING + TOOLTIP_ICON_SIZE,
                                bottom: TOOLTIP_PADDING + TOOLTIP_ICON_SIZE,
                            };
                            rt.DrawText(
                                &excl,
                                &f.icon,
                                &excl_rect,
                                &white_brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                                DWRITE_MEASURING_MODE_NATURAL,
                            );
                        }

                        let text_col = if is_dark {
                            COLOR_DARK_TEXT
                        } else {
                            COLOR_LIGHT_TEXT
                        };
                        if let Ok(text_brush) = rt.CreateSolidColorBrush(
                            &D2D1_COLOR_F {
                                r: text_col,
                                g: text_col,
                                b: text_col,
                                a: 1.0,
                            },
                            None,
                        ) {
                            if let Some(f) = &FONTS {
                                let title_u16 = TOOLTIP_TITLE.encode_utf16().collect::<Vec<u16>>();
                                let title_rect = D2D_RECT_F {
                                    left: TOOLTIP_PADDING + TOOLTIP_ICON_SIZE + 10.0,
                                    top: TOOLTIP_PADDING,
                                    right: w_dip - 25.0, // Keeping extra right padding for safety
                                    bottom: 35.0,
                                };
                                rt.DrawText(
                                    &title_u16,
                                    &f.tooltip_bold,
                                    &title_rect,
                                    &text_brush,
                                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                                    DWRITE_MEASURING_MODE_NATURAL,
                                );

                                let msg_u16 = TOOLTIP_MESSAGE.encode_utf16().collect::<Vec<u16>>();
                                let msg_rect = D2D_RECT_F {
                                    left: TOOLTIP_PADDING + TOOLTIP_ICON_SIZE + 10.0,
                                    top: 38.0,
                                    right: w_dip - 25.0,
                                    bottom: h_dip - tri_h - 5.0,
                                };
                                rt.DrawText(
                                    &msg_u16,
                                    &f.tooltip,
                                    &msg_rect,
                                    &text_brush,
                                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                                    DWRITE_MEASURING_MODE_NATURAL,
                                );
                            }
                        }

                        if pushed_layer {
                            rt.PopLayer();
                        }
                        rt.SetTransform(&Matrix3x2::identity());
                        if let Err(_) = rt.EndDraw(None, None) {
                            TOOLTIP_RENDER_TARGET = None;
                        }
                    }
                }
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
