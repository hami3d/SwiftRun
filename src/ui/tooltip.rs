use std::time::Instant;
use windows::core::*;
use windows::Foundation::Numerics::Matrix3x2;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::animations::*;
use crate::config::*;
use crate::ui::resources::*;
use crate::ui::*;

pub static mut TOOLTIP_TEXT: String = String::new();

pub unsafe fn show_tooltip(msg: &str) {
    if H_TOOLTIP.0 != 0 {
        let _ = DestroyWindow(H_TOOLTIP);
    }

    TOOLTIP_TEXT = msg.to_string();

    let mut main_rect = RECT::default();
    let main_hwnd = FindWindowW(w!("SwiftRunClass"), w!("SwiftRun"));
    let _ = GetWindowRect(main_hwnd, &mut main_rect);

    let text_len = msg.chars().count();
    let char_width: usize = 8;
    let width = (text_len * char_width).max(100).min(WIN_W as usize) as i32;
    let height = 22;
    let dpi_scale = get_dpi_scale(main_hwnd);

    let x = main_rect.left + (WIN_W as i32 - width) / 2;
    let input_y_screen = main_rect.top + ((INPUT_Y * dpi_scale) as i32);
    let y = input_y_screen - height - 10;

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
        main_hwnd,
        None,
        instance,
        None,
    );

    let v: i32 = 3; // DWMWCP_ROUNDSMALL
    let _ = DwmSetWindowAttribute(H_TOOLTIP, DWMWINDOWATTRIBUTE(33), &v as *const _ as _, 4);

    ShowWindow(H_TOOLTIP, SW_SHOWNOACTIVATE);

    TOOLTIP_ANIM_START = Some(Instant::now());
    TOOLTIP_ANIM_TYPE = AnimType::Entering;
    SetTimer(main_hwnd, 3, 16, None);

    SetTimer(H_TOOLTIP, 2, 8000, None);

    SetFocus(H_EDIT);
    SendMessageW(
        H_EDIT,
        windows::Win32::UI::Controls::EM_SETSEL,
        WPARAM(0),
        LPARAM(-1),
    );
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
                let main_hwnd = FindWindowW(w!("SwiftRunClass"), w!("SwiftRun"));
                TOOLTIP_ANIM_START = Some(Instant::now());
                TOOLTIP_ANIM_TYPE = AnimType::Exiting;
                SetTimer(main_hwnd, 3, 16, None);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let main_hwnd = FindWindowW(w!("SwiftRunClass"), w!("SwiftRun"));
            TOOLTIP_ANIM_START = Some(Instant::now());
            TOOLTIP_ANIM_TYPE = AnimType::Exiting;
            SetTimer(main_hwnd, 3, 16, None);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);

            if let Some(factory) = &D2D_FACTORY {
                let mut rect = RECT::default();
                GetClientRect(hwnd, &mut rect);
                let w = (rect.right - rect.left) as u32;
                let h = (rect.bottom - rect.top) as u32;

                if w > 0 && h > 0 {
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
                        target.BeginDraw();

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

                        target.Clear(Some(&D2D1_COLOR_F {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }));

                        let mid_x = w as f32 / 2.0;
                        let mid_y = h as f32 / 2.0;

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

                        target.SetTransform(&(translate * scale_mat * translate_back));

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

                        let is_dark = is_dark_mode();
                        let bg_col = if is_dark { 0.1 } else { 1.0 };
                        let bg_alpha = if is_dark { 0.55 } else { 0.95 };

                        if let Ok(bg_brush) = target.CreateSolidColorBrush(
                            &D2D1_COLOR_F {
                                r: bg_col,
                                g: bg_col,
                                b: bg_col,
                                a: bg_alpha,
                            },
                            None,
                        ) {
                            target.FillRoundedRectangle(
                                &D2D1_ROUNDED_RECT {
                                    rect: D2D_RECT_F {
                                        left: 0.0,
                                        top: 0.0,
                                        right: w as f32,
                                        bottom: h as f32,
                                    },
                                    radiusX: 4.0,
                                    radiusY: 4.0,
                                },
                                &bg_brush,
                            );
                        }

                        let text_col = if is_dark { 1.0 } else { 0.2 };
                        if let Ok(text_brush) = target.CreateSolidColorBrush(
                            &D2D1_COLOR_F {
                                r: text_col,
                                g: text_col,
                                b: text_col,
                                a: 1.0,
                            },
                            None,
                        ) {
                            if let Some(f) = &FONTS {
                                let msg_u16 = TOOLTIP_TEXT.encode_utf16().collect::<Vec<u16>>();
                                if let Some(dwrite) = &DWRITE_FACTORY {
                                    if let Ok(layout) = dwrite.CreateTextLayout(
                                        &msg_u16,
                                        &f.tooltip,
                                        w as f32 - 12.0,
                                        h as f32,
                                    ) {
                                        let _ =
                                            layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
                                        let _ = layout.SetParagraphAlignment(
                                            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                                        );
                                        target.DrawTextLayout(
                                            D2D_POINT_2F { x: 6.0, y: -2.0 },
                                            &layout,
                                            &text_brush,
                                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                                        );
                                    }
                                }
                            }
                        }

                        target.PopLayer();
                        target.SetTransform(&Matrix3x2::identity());
                        let _ = target.EndDraw(None, None);
                    }
                }
            }
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
