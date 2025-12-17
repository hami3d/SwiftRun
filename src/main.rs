#![allow(static_mut_refs)]
#![allow(non_snake_case)]

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Direct2D::{
        Common::{
            D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_POINT_2F,
            D2D_RECT_F, D2D_SIZE_U,
        },
        D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
        ID2D1SolidColorBrush, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DEBUG_LEVEL_NONE,
        D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_OPTIONS, D2D1_FACTORY_TYPE_SINGLE_THREADED,
        D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
        D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT,
    },
    Win32::Graphics::DirectWrite::*,
    Win32::Graphics::Dwm::*,
    Win32::Graphics::Dxgi::Common::*,
    Win32::Graphics::Gdi::*,
    Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress},
    Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
    Win32::System::SystemInformation::GetTickCount,
    Win32::UI::Controls::MARGINS,
    Win32::UI::HiDpi::{GetDpiForWindow, SetProcessDpiAwareness},
    Win32::UI::Input::KeyboardAndMouse::SetFocus,
    Win32::UI::Shell::ShellExecuteW,
    Win32::UI::WindowsAndMessaging::*,
};

const EDIT_ID: u32 = 101;
static mut H_EDIT: HWND = HWND(0);

static INPUT_BUFFER: Mutex<String> = Mutex::new(String::new());
static mut HISTORY: Option<Vec<String>> = None;
static mut SHOW_DROPDOWN: bool = false;
static mut SCROLL_OFFSET: usize = 0;
static mut H_DROPDOWN: HWND = HWND(0);
static mut DROPDOWN_RENDER_TARGET: Option<ID2D1HwndRenderTarget> = None;
static mut HOVER_DROPDOWN: Option<usize> = None;

// Fixed window dimensions
const WIN_W: f32 = 500.0;
const WIN_H: f32 = 180.0;
const ITEM_H: f32 = 18.0; // Height of each history item

// UI Layout
const MARGIN: f32 = 16.0;
const CORNER_RADIUS: f32 = 5.0;
const TITLE_BAR_H: f32 = 32.0;
const WIN_BTN_W: f32 = 46.0;

// Element positions (fixed layout)
const TITLE_Y: f32 = 8.0;
const DESC_Y: f32 = 38.0;
const INPUT_Y: f32 = 45.0;
const INPUT_H: f32 = 32.0;
const BTN_Y: f32 = 95.0;
const BTN_H: f32 = 30.0;
const BTN_W: f32 = 80.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum HoverId {
    None,
    Close,
    Min,
    Input,
    Ok,
    Cancel,
    Dropdown,
}

static mut HOVER: HoverId = HoverId::None;
static mut D2D_FACTORY: Option<ID2D1Factory> = None;
static mut DWRITE_FACTORY: Option<IDWriteFactory> = None;
static mut RENDER_TARGET: Option<ID2D1HwndRenderTarget> = None;
static mut BRUSHES: Option<Brushes> = None;
static mut DROPDOWN_BRUSHES: Option<Brushes> = None;
static mut FONTS: Option<Fonts> = None;

struct Brushes {
    white: ID2D1SolidColorBrush,
    gray: ID2D1SolidColorBrush,
    input_bg: ID2D1SolidColorBrush,
    btn_bg: ID2D1SolidColorBrush,
    btn_hover: ID2D1SolidColorBrush,
    close_hover: ID2D1SolidColorBrush,
}

struct Fonts {
    title: IDWriteTextFormat,
    label: IDWriteTextFormat,
    input: IDWriteTextFormat,
    button: IDWriteTextFormat,
}

#[repr(C)]
struct ACCENT_POLICY {
    AccentState: u32,
    AccentFlags: u32,
    GradientColor: u32,
    AnimationId: u32,
}

#[repr(C)]
struct WINDOWCOMPOSITIONATTRIBDATA {
    Attrib: u32,
    pvData: *mut std::ffi::c_void,
    cbData: usize,
}

fn set_acrylic_effect(hwnd: HWND) {
    unsafe {
        // Try Windows 11 DwmSetWindowAttribute first
        let dwmapi = GetModuleHandleW(w!("dwmapi.dll")).unwrap();
        let func_name = std::ffi::CString::new("DwmSetWindowAttribute").unwrap();
        let func_ptr = GetProcAddress(dwmapi, PCSTR(func_name.as_ptr() as _));

        if let Some(func) = func_ptr {
            let dwm_set_window_attribute: unsafe extern "system" fn(
                HWND,
                u32,
                *const std::ffi::c_void,
                u32,
            ) -> i32 = std::mem::transmute(func);

            // DWMWA_USE_IMMERSIVE_DARK_MODE = 20
            let is_dark = is_dark_mode();
            let dark_mode: i32 = if is_dark { 1 } else { 0 };
            dwm_set_window_attribute(
                hwnd,
                20,
                &dark_mode as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            // DWMWA_SYSTEMBACKDROP_TYPE = 38
            // DWMSBT_MAINWINDOW = 2 (Mica)
            // DWMSBT_TRANSIENTWITHACRYLIC = 3 (Acrylic)
            let backdrop_type: u32 = 3;
            dwm_set_window_attribute(
                hwnd,
                38,
                &backdrop_type as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // Also extend frame into client area to ensure transparency
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            let dwm_extend_frame: unsafe extern "system" fn(HWND, *const MARGINS) -> i32 =
                std::mem::transmute(
                    GetProcAddress(
                        dwmapi,
                        PCSTR(b"DwmExtendFrameIntoClientArea\0".as_ptr() as _),
                    )
                    .unwrap(),
                );
            dwm_extend_frame(hwnd, &margins);
        }

        // Fallback to SetWindowCompositionAttribute for older Windows 10
        let user32 = GetModuleHandleW(w!("user32.dll")).unwrap();
        let func_name = std::ffi::CString::new("SetWindowCompositionAttribute").unwrap();
        let func_ptr = GetProcAddress(user32, PCSTR(func_name.as_ptr() as _));

        if let Some(func) = func_ptr {
            let set_window_composition_attribute: unsafe extern "system" fn(
                HWND,
                *mut WINDOWCOMPOSITIONATTRIBDATA,
            ) -> i32 = std::mem::transmute(func);

            let is_dark = is_dark_mode();
            let tint = if is_dark { 0xCC000000 } else { 0xCCF3F3F3 };

            let mut policy = ACCENT_POLICY {
                AccentState: 4, // ACCENT_ENABLE_ACRYLICBLURBEHIND
                AccentFlags: 0,
                GradientColor: tint, // AABBGGRR
                AnimationId: 0,
            };

            let mut data = WINDOWCOMPOSITIONATTRIBDATA {
                Attrib: 19, // WCA_ACCENT_POLICY
                pvData: &mut policy as *mut _ as _,
                cbData: std::mem::size_of::<ACCENT_POLICY>(),
            };

            set_window_composition_attribute(hwnd, &mut data);
        }
    }
}

fn get_history_path() -> Option<PathBuf> {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let mut path = PathBuf::from(local_app_data);
        path.push("SwiftRun");
        if !path.exists() {
            let _ = fs::create_dir(&path);
        }
        path.push("history.txt");
        Some(path)
    } else {
        None
    }
}

fn load_history() {
    unsafe {
        HISTORY = Some(Vec::new());
        if let Some(path) = get_history_path() {
            if let Ok(file) = fs::File::open(path) {
                let reader = BufReader::new(file);
                let mut items = Vec::new();
                for line in reader.lines() {
                    if let Ok(l) = line {
                        if !l.trim().is_empty() {
                            items.push(l);
                        }
                    }
                }
                HISTORY = Some(items);
            }
        }
    }
}

fn save_history(cmd: &str) {
    unsafe {
        if let Some(history) = HISTORY.as_mut() {
            // Remove if exists (to move to top)
            if let Some(pos) = history.iter().position(|x| x == cmd) {
                history.remove(pos);
            }
            // Insert at top
            history.insert(0, cmd.to_string());
            // Trim to 50
            if history.len() > 50 {
                history.truncate(50);
            }

            // Save to file
            if let Some(path) = get_history_path() {
                if let Ok(mut file) = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(path)
                {
                    for item in history.iter() {
                        let _ = writeln!(file, "{}", item);
                    }
                }
            }
        }
    }
}

fn is_dark_mode() -> bool {
    unsafe {
        let mut data = [0u8; 4];
        let mut len = 4u32;
        let res = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(data.as_mut_ptr() as _),
            Some(&mut len),
        );
        if res.is_ok() {
            let val = u32::from_le_bytes(data);
            val == 0 // 0 = Dark, 1 = Light
        } else {
            true // Default to Dark
        }
    }
}

fn main() -> Result<()> {
    unsafe {
        load_history();

        // Set DPI awareness - Per Monitor V2
        let _ = SetProcessDpiAwareness(windows::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE);

        let instance = GetModuleHandleW(None)?;
        let class_name = w!("SwiftRunClass");

        let wc = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hInstance: instance.into(),
            lpszClassName: class_name,
            lpfnWndProc: Some(wndproc),
            hbrBackground: HBRUSH::default(),
            ..Default::default()
        };
        RegisterClassW(&wc);

        // Center on screen
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);

        // Register Dropdown Class
        let dropdown_class_name = w!("SwiftRunDropdown");
        let wc_dropdown = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hInstance: instance,
            lpszClassName: dropdown_class_name,
            lpfnWndProc: Some(dropdown_wndproc),
            hbrBackground: HBRUSH::default(),
            ..Default::default()
        };
        RegisterClassW(&wc_dropdown);

        // Create Main Window
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("SwiftRun"),
            WS_POPUP | WS_VISIBLE,
            (screen_w / 2) - (WIN_W as i32 / 2),
            (screen_h / 2) - (WIN_H as i32 / 2),
            WIN_W as i32,
            WIN_H as i32,
            None,
            None,
            instance,
            None,
        );

        // Create Dropdown Window (Hidden initially)
        let h_dropdown = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            dropdown_class_name,
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            hwnd, // Parent is main window
            None,
            instance,
            None,
        );
        unsafe { H_DROPDOWN = h_dropdown };

        // Mica + rounded corners
        let v: i32 = 1;
        DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(20), &v as *const _ as _, 4).ok();

        // Enable Acrylic Blur
        set_acrylic_effect(hwnd);

        let v: i32 = 2; // Round corners
        DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(33), &v as *const _ as _, 4).ok();
        let m = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        DwmExtendFrameIntoClientArea(hwnd, &m).ok();

        // Create hidden Edit control for input handling
        H_EDIT = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            None,
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(0x0080), // ES_AUTOHSCROLL = 0x0080
            0,
            0,
            0,
            0, // Hidden (size 0)
            hwnd,
            HMENU(EDIT_ID as _),
            instance,
            None,
        );

        // Init D2D
        D2D_FACTORY = D2D1CreateFactory(
            D2D1_FACTORY_TYPE_SINGLE_THREADED,
            Some(&D2D1_FACTORY_OPTIONS::default()),
        )
        .ok();
        DWRITE_FACTORY = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok();

        // Setup blink timer
        let blink_time = GetCaretBlinkTime();
        SetTimer(hwnd, 1, blink_time, None);

        SetFocus(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_KEYDOWN {
                if msg.wParam.0 == 0x0D {
                    // VK_RETURN
                    run_command();
                    continue;
                }
                if msg.wParam.0 == 0x1B {
                    // VK_ESCAPE
                    PostQuitMessage(0);
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

fn hit_test(x: i32, y: i32, w: f32, _h: f32) -> HoverId {
    let fx = x as f32;
    let fy = y as f32;

    // Close button (top right)
    if fx >= w - WIN_BTN_W && fy < TITLE_BAR_H {
        return HoverId::Close;
    }
    // Minimize button
    if fx >= w - WIN_BTN_W * 2.0 && fx < w - WIN_BTN_W && fy < TITLE_BAR_H {
        return HoverId::Min;
    }
    // OK button
    let ok_x = w - MARGIN - BTN_W * 2.0 - 8.0;
    if fx >= ok_x && fx < ok_x + BTN_W && fy >= BTN_Y && fy < BTN_Y + BTN_H {
        return HoverId::Ok;
    }
    // Cancel button
    let cancel_x = w - MARGIN - BTN_W;
    if fx >= cancel_x && fx < cancel_x + BTN_W && fy >= BTN_Y && fy < BTN_Y + BTN_H {
        return HoverId::Cancel;
    }

    // Dropdown Chevron
    let chevron_x = w - MARGIN - 20.0;
    let chevron_y = INPUT_Y + INPUT_H / 2.0;
    if fx >= chevron_x - 10.0
        && fx < chevron_x + 10.0
        && fy >= chevron_y - 10.0
        && fy < chevron_y + 10.0
    {
        return HoverId::Dropdown;
    }

    // Input Box
    if fx >= MARGIN && fx < w - MARGIN && fy >= INPUT_Y && fy < INPUT_Y + INPUT_H {
        return HoverId::Input;
    }

    // History Items logic removed (handled by dropdown window)

    HoverId::None
}

fn run_command() {
    unsafe {
        if let Ok(buf) = INPUT_BUFFER.lock() {
            if !buf.is_empty() {
                let cmd_str = buf.clone();
                save_history(&cmd_str);

                let cmd: Vec<u16> = buf.encode_utf16().chain(std::iter::once(0)).collect();
                ShellExecuteW(
                    None,
                    w!("open"),
                    PCWSTR(cmd.as_ptr()),
                    None,
                    None,
                    SW_SHOWNORMAL,
                );
            }
        }
        PostQuitMessage(0);
    }
}

unsafe fn get_dpi_scale(hwnd: HWND) -> f32 {
    let dpi = GetDpiForWindow(hwnd);
    if dpi == 0 {
        1.0
    } else {
        dpi as f32 / 96.0
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_SIZE => {
                if let Some(target) = RENDER_TARGET.as_ref() {
                    let size = D2D_SIZE_U {
                        width: (lp.0 & 0xFFFF) as u32,
                        height: ((lp.0 >> 16) & 0xFFFF) as u32,
                    };
                    target.Resize(&size).ok();
                }
                InvalidateRect(hwnd, None, BOOL(0));
                LRESULT(0)
            }

            WM_MOVE => {
                if SHOW_DROPDOWN {
                    SHOW_DROPDOWN = false;
                    ShowWindow(H_DROPDOWN, SW_HIDE);
                    InvalidateRect(hwnd, None, BOOL(0));
                }
                LRESULT(0)
            }

            WM_NCCALCSIZE => {
                if wp.0 == 1 {
                    LRESULT(0)
                } else {
                    DefWindowProcW(hwnd, msg, wp, lp)
                }
            }
            WM_NCACTIVATE => LRESULT(1),
            WM_ACTIVATE => {
                if wp.0 == 0 {
                    // Deactivated
                    if SHOW_DROPDOWN {
                        SHOW_DROPDOWN = false;
                        ShowWindow(H_DROPDOWN, SW_HIDE);
                        InvalidateRect(hwnd, None, BOOL(0));
                    }
                }
                LRESULT(0)
            }

            WM_NCHITTEST => {
                let x = (lp.0 & 0xFFFF) as i16 as i32;
                let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;
                let mut wr = RECT::default();
                GetWindowRect(hwnd, &mut wr);
                let lx = x - wr.left;
                let ly = y - wr.top;
                let width = (wr.right - wr.left) as f32;

                let scale = get_dpi_scale(hwnd);
                let slx = lx as f32 / scale;
                let sly = ly as f32 / scale;

                // Draggable title bar (excluding buttons)
                if sly < TITLE_BAR_H && slx < (width / scale) - WIN_BTN_W * 2.0 {
                    return LRESULT(HTCAPTION as isize);
                }

                LRESULT(HTCLIENT as isize)
            }

            WM_SETCURSOR => {
                if HOVER == HoverId::Input {
                    SetCursor(LoadCursorW(None, IDC_IBEAM).unwrap());
                    LRESULT(1)
                } else {
                    DefWindowProcW(hwnd, msg, wp, lp)
                }
            }

            WM_TIMER => {
                if wp.0 == 1 {
                    InvalidateRect(hwnd, None, BOOL(0));
                }
                LRESULT(0)
            }

            // WM_MOUSEWHEEL moved to dropdown_wndproc
            WM_SETTINGCHANGE => {
                set_acrylic_effect(hwnd);
                BRUSHES = None; // Force recreate brushes
                InvalidateRect(hwnd, None, BOOL(0));
                LRESULT(0)
            }

            WM_MOUSEMOVE => {
                let x = (lp.0 & 0xFFFF) as i16 as i32;
                let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;

                let mut cr = RECT::default();
                GetClientRect(hwnd, &mut cr);

                let scale = get_dpi_scale(hwnd);
                let w = (cr.right - cr.left) as f32 / scale;
                let h = (cr.bottom - cr.top) as f32 / scale;
                let sx = x as f32 / scale;
                let sy = y as f32 / scale;

                let new_hover = hit_test(sx as i32, sy as i32, w, h);
                if new_hover != HOVER {
                    HOVER = new_hover;
                    InvalidateRect(hwnd, None, BOOL(0));
                }
                LRESULT(0)
            }

            WM_LBUTTONDOWN => {
                let x = (lp.0 & 0xFFFF) as i16 as i32;
                let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;

                let mut cr = RECT::default();
                GetClientRect(hwnd, &mut cr);

                let scale = get_dpi_scale(hwnd);
                let w = (cr.right - cr.left) as f32 / scale;
                let h = (cr.bottom - cr.top) as f32 / scale;
                let sx = x as f32 / scale;
                let sy = y as f32 / scale;

                match hit_test(sx as i32, sy as i32, w, h) {
                    HoverId::Close => PostQuitMessage(0),
                    HoverId::Min => {
                        ShowWindow(hwnd, SW_MINIMIZE);
                    }
                    HoverId::Ok => run_command(),
                    HoverId::Cancel => PostQuitMessage(0),
                    HoverId::None | HoverId::Input => {
                        SetFocus(H_EDIT);
                    }
                    HoverId::Dropdown => {
                        SHOW_DROPDOWN = !SHOW_DROPDOWN;
                        SCROLL_OFFSET = 0;

                        if SHOW_DROPDOWN {
                            // Show dropdown
                            let mut rect = RECT::default();
                            GetWindowRect(hwnd, &mut rect);
                            let scale = get_dpi_scale(hwnd);

                            let margin_px = (MARGIN * scale) as i32;
                            let input_y_px = ((INPUT_Y + INPUT_H) * scale) as i32;
                            let spacing_px = (5.0 * scale) as i32;

                            let x = rect.left + margin_px;
                            let y = rect.top + input_y_px + spacing_px;
                            let w = (rect.right - rect.left) - (margin_px * 2);

                            let mut h = 0;
                            if let Some(history) = HISTORY.as_ref() {
                                let count = history.len().min(3);
                                if count > 0 {
                                    h = (count as f32 * ITEM_H * scale) as i32;
                                }
                            }

                            if h > 0 {
                                SetWindowPos(
                                    H_DROPDOWN,
                                    HWND_TOPMOST,
                                    x,
                                    y,
                                    w,
                                    h,
                                    SWP_SHOWWINDOW | SWP_NOACTIVATE,
                                );
                            } else {
                                SHOW_DROPDOWN = false;
                            }
                        } else {
                            // Hide dropdown
                            ShowWindow(H_DROPDOWN, SW_HIDE);
                        }

                        InvalidateRect(hwnd, None, BOOL(0));
                    } // HistoryItem click is now handled in dropdown_wndproc
                }
                LRESULT(0)
            }

            WM_COMMAND => {
                let id = wp.0 & 0xFFFF;
                let code = (wp.0 >> 16) & 0xFFFF;
                if id == EDIT_ID as usize && code == 0x0300 {
                    // EN_CHANGE
                    let len = GetWindowTextLengthW(H_EDIT);
                    let mut buf = vec![0u16; (len + 1) as usize];
                    GetWindowTextW(H_EDIT, &mut buf);
                    if let Ok(mut lock) = INPUT_BUFFER.lock() {
                        *lock = String::from_utf16_lossy(&buf[..len as usize]);
                    }
                    InvalidateRect(hwnd, None, BOOL(0));
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
                SetFocus(H_EDIT);
                LRESULT(0)
            }

            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

const WM_MOUSELEAVE: u32 = 0x02A3;

unsafe extern "system" fn dropdown_wndproc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_CREATE => {
            set_acrylic_effect(hwnd);
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_SHOWWINDOW => {
            if wp.0 == 1 {
                set_acrylic_effect(hwnd);
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_SETTINGCHANGE => {
            set_acrylic_effect(hwnd);
            DROPDOWN_BRUSHES = None;
            InvalidateRect(hwnd, None, BOOL(0));
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);
            ensure_dropdown_resources(hwnd);

            if let Some(target) = &DROPDOWN_RENDER_TARGET {
                if let Some(b) = &DROPDOWN_BRUSHES {
                    if let Some(f) = &FONTS {
                        let rt: ID2D1RenderTarget = target.cast().unwrap();
                        rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                        target.BeginDraw();
                        target.Clear(Some(&D2D1_COLOR_F {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }));

                        // Draw background
                        let size = target.GetSize();
                        let w = size.width;
                        let h = size.height;

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
                            for (i, item) in history.iter().skip(SCROLL_OFFSET).take(5).enumerate()
                            {
                                let item_y = i as f32 * ITEM_H;

                                // Adjust highlight width if scrollbar is visible
                                let total_items = history.len();
                                let scroll_width = if total_items > 5 { 8.0 } else { 0.0 };

                                let rect = D2D_RECT_F {
                                    left: 0.0,
                                    top: item_y,
                                    right: w - scroll_width,
                                    bottom: item_y + ITEM_H,
                                };

                                // Hover highlight
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

                                // Text
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

                            // Scroll Indicator
                            let total_items = history.len();
                            if total_items > 5 {
                                let visible_items = 5.0;
                                let ratio = visible_items / total_items as f32;
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

                        target.EndDraw(None, None).ok();
                    }
                }
            }

            EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        windows::Win32::UI::WindowsAndMessaging::WM_MOUSEMOVE => {
            let y = (lp.0 >> 16) as i16 as f32;
            let idx = (y / ITEM_H) as usize;
            if idx < 5 {
                if HOVER_DROPDOWN != Some(idx) {
                    HOVER_DROPDOWN = Some(idx);
                    InvalidateRect(hwnd, None, BOOL(0));
                }
            } else {
                if HOVER_DROPDOWN.is_some() {
                    HOVER_DROPDOWN = None;
                    InvalidateRect(hwnd, None, BOOL(0));
                }
            }

            // Track mouse leave
            let mut tme = windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<
                    windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT,
                >() as u32,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            windows::Win32::UI::Input::KeyboardAndMouse::TrackMouseEvent(&mut tme);
            LRESULT(0)
        }

        WM_MOUSELEAVE => {
            HOVER_DROPDOWN = None;
            InvalidateRect(hwnd, None, BOOL(0));
            LRESULT(0)
        }

        windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONDOWN => {
            if let Some(idx) = HOVER_DROPDOWN {
                if let Some(history) = HISTORY.as_ref() {
                    if let Some(cmd) = history.get(SCROLL_OFFSET + idx) {
                        if let Ok(mut lock) = INPUT_BUFFER.lock() {
                            *lock = cmd.clone();
                        }
                        SetWindowTextW(
                            H_EDIT,
                            PCWSTR(
                                cmd.encode_utf16()
                                    .chain(std::iter::once(0))
                                    .collect::<Vec<_>>()
                                    .as_ptr(),
                            ),
                        );

                        // Close dropdown
                        SHOW_DROPDOWN = false;
                        ShowWindow(hwnd, SW_HIDE);
                    }
                }
            }
            LRESULT(0)
        }

        windows::Win32::UI::WindowsAndMessaging::WM_MOUSEWHEEL => {
            let delta = (wp.0 >> 16) as i16;
            if let Some(history) = HISTORY.as_ref() {
                if history.len() > 5 {
                    if delta > 0 {
                        if SCROLL_OFFSET > 0 {
                            SCROLL_OFFSET -= 1;
                        }
                    } else {
                        if SCROLL_OFFSET < history.len() - 5 {
                            SCROLL_OFFSET += 1;
                        }
                    }
                    InvalidateRect(hwnd, None, BOOL(0));
                }
            }
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn ensure_dropdown_resources(hwnd: HWND) {
    if DROPDOWN_RENDER_TARGET.is_none() {
        let Some(factory) = &D2D_FACTORY else { return };

        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect);
        let w = (rect.right - rect.left) as u32;
        let h = (rect.bottom - rect.top) as u32;

        // If size is 0, don't create target yet
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
        // Check if resize is needed
        if let Some(target) = &DROPDOWN_RENDER_TARGET {
            let mut rect = RECT::default();
            GetWindowRect(hwnd, &mut rect);
            let w = (rect.right - rect.left) as u32;
            let h = (rect.bottom - rect.top) as u32;

            let size = target.GetPixelSize();
            if size.width != w || size.height != h {
                target
                    .Resize(&windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U {
                        width: w,
                        height: h,
                    })
                    .ok();
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

    // Create brushes
    let is_dark = is_dark_mode();

    let text_col = if is_dark { 1.0 } else { 0.0 };
    let white = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: text_col,
                g: text_col,
                b: text_col,
                a: 1.0,
            },
            None,
        )
        .unwrap();

    let gray_col = if is_dark { 0.6 } else { 0.4 };
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

    let input_bg_col = if is_dark { 0.1 } else { 0.9 };
    let input_bg = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: input_bg_col,
                g: input_bg_col,
                b: input_bg_col,
                a: 0.15,
            },
            None,
        )
        .unwrap();

    let btn_bg_col = if is_dark { 0.2 } else { 0.95 };
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

    let btn_hover_col = if is_dark { 0.35 } else { 0.85 };
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

    DROPDOWN_BRUSHES = Some(Brushes {
        white,
        gray,
        input_bg,
        btn_bg,
        btn_hover,
        close_hover,
    });
}

unsafe fn ensure_resources(hwnd: HWND) {
    if RENDER_TARGET.is_none() {
        let Some(factory) = &D2D_FACTORY else { return };
        let Some(dwrite) = &DWRITE_FACTORY else {
            return;
        };

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
    let Some(dwrite) = &DWRITE_FACTORY else {
        return;
    };
    let rt: ID2D1RenderTarget = target.cast().unwrap();

    // Create brushes
    let is_dark = is_dark_mode();

    let text_col = if is_dark { 1.0 } else { 0.0 };
    let white = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: text_col,
                g: text_col,
                b: text_col,
                a: 1.0,
            },
            None,
        )
        .unwrap();

    let gray_col = if is_dark { 0.6 } else { 0.4 };
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

    let input_bg_col = if is_dark { 0.1 } else { 0.9 };
    let input_bg = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: input_bg_col,
                g: input_bg_col,
                b: input_bg_col,
                a: 0.15,
            },
            None,
        )
        .unwrap();

    let btn_bg_col = if is_dark { 0.2 } else { 0.95 };
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

    let btn_hover_col = if is_dark { 0.35 } else { 0.85 };
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

    BRUSHES = Some(Brushes {
        white,
        gray,
        input_bg,
        btn_bg,
        btn_hover,
        close_hover,
    });

    // Create fonts - each with specific alignment
    let title = dwrite
        .CreateTextFormat(
            w!("Segoe UI Variable Display"),
            None,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            13.0,
            w!(""),
        )
        .unwrap();
    title.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING).ok();
    title
        .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)
        .ok();

    let label = dwrite
        .CreateTextFormat(
            w!("Segoe UI Variable Text"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            12.0,
            w!(""),
        )
        .unwrap();
    label.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING).ok();
    label
        .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
        .ok();

    let input = dwrite
        .CreateTextFormat(
            w!("Segoe UI Variable Text"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            13.0,
            w!(""),
        )
        .unwrap();
    input.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING).ok();
    input
        .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
        .ok();

    let button = dwrite
        .CreateTextFormat(
            w!("Segoe UI Variable Text"),
            None,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            12.0,
            w!(""),
        )
        .unwrap();
    button.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER).ok();
    button
        .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
        .ok();

    FONTS = Some(Fonts {
        title,
        label,
        input,
        button,
    });
    RENDER_TARGET = Some(target.clone());
}

unsafe fn paint() {
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
    let w = size.width;
    let h = size.height;

    // Window buttons
    // Minimize
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
            x: min_x + 18.0, // increasing this will make the line smaller from the right side
            y: cy,
        },
        D2D_POINT_2F {
            x: min_x + 28.0, // left side of the line
            y: cy,
        },
        &b.white,
        1.0,
        None,
    );

    // Close
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
            x: cx - 5.0,
            y: cy - 5.0,
        },
        D2D_POINT_2F {
            x: cx + 5.0,
            y: cy + 5.0,
        },
        &b.white,
        0.8,
        None,
    );
    rt.DrawLine(
        D2D_POINT_2F {
            x: cx + 5.0,
            y: cy - 5.0,
        },
        D2D_POINT_2F {
            x: cx - 5.0,
            y: cy + 5.0,
        },
        &b.white,
        0.8,
        None,
    );

    // Title
    let title_text: Vec<u16> = "Swift Run".encode_utf16().collect();
    rt.DrawText(
        &title_text,
        &f.title,
        &D2D_RECT_F {
            left: MARGIN,
            top: TITLE_Y,
            right: 100.0,
            bottom: TITLE_Y + 20.0,
        },
        &b.white,
        D2D1_DRAW_TEXT_OPTIONS_NONE,
        DWRITE_MEASURING_MODE_NATURAL,
    );

    // Input field
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

    // Input text
    if let Ok(buf) = INPUT_BUFFER.lock() {
        let text_rect = D2D_RECT_F {
            left: MARGIN + 10.0,
            top: INPUT_Y,
            right: w - MARGIN - 10.0,
            bottom: INPUT_Y + INPUT_H,
        };

        let text_str: Vec<u16> = if buf.is_empty() {
            "Type the name of a command to run".encode_utf16().collect()
        } else {
            buf.encode_utf16().collect()
        };

        let format = &f.input; // Use same format for now
        let brush = if buf.is_empty() { &b.gray } else { &b.white };

        if let Some(dwrite) = DWRITE_FACTORY.as_ref() {
            if let Ok(layout) = dwrite.CreateTextLayout(
                &text_str,
                format,
                text_rect.right - text_rect.left,
                text_rect.bottom - text_rect.top,
            ) {
                rt.DrawTextLayout(
                    D2D_POINT_2F {
                        x: text_rect.left,
                        y: text_rect.top,
                    },
                    &layout,
                    brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );

                // Draw Caret
                if windows::Win32::UI::Input::KeyboardAndMouse::GetFocus() == H_EDIT {
                    // Blink logic
                    let blink_time = GetCaretBlinkTime();
                    let tick = GetTickCount();
                    if (tick / blink_time) % 2 == 0 {
                        let mut start: u32 = 0;
                        let mut end: u32 = 0;
                        SendMessageW(
                            H_EDIT,
                            0x00B0,
                            WPARAM(&mut start as *mut _ as usize),
                            LPARAM(&mut end as *mut _ as isize),
                        );

                        let mut caret_x: f32 = 0.0;
                        let mut caret_y: f32 = 0.0;
                        let mut metrics: DWRITE_HIT_TEST_METRICS = std::mem::zeroed();

                        let caret_pos = if buf.is_empty() { 0 } else { end };

                        if buf.is_empty() {
                            rt.DrawLine(
                                D2D_POINT_2F {
                                    x: text_rect.left,
                                    y: text_rect.top + 8.0,
                                },
                                D2D_POINT_2F {
                                    x: text_rect.left,
                                    y: text_rect.top + 25.0,
                                },
                                &b.white,
                                1.0,
                                None,
                            );
                        } else {
                            layout
                                .HitTestTextPosition(
                                    caret_pos,
                                    false, // isTrailingHit
                                    &mut caret_x,
                                    &mut caret_y,
                                    &mut metrics,
                                )
                                .ok();

                            let abs_x = text_rect.left + caret_x;
                            let abs_y = text_rect.top + caret_y;
                            rt.DrawLine(
                                D2D_POINT_2F {
                                    x: abs_x,
                                    y: abs_y + 3.0,
                                },
                                D2D_POINT_2F {
                                    x: abs_x,
                                    y: abs_y + 18.0,
                                },
                                &b.white,
                                1.0,
                                None,
                            );
                        }
                    }
                }
            }
        }
    }

    // Dropdown Chevron
    let chevron_x = w - MARGIN - 20.0;
    let chevron_y = INPUT_Y + INPUT_H / 2.0;

    // Draw Chevron (V shape)
    rt.DrawLine(
        D2D_POINT_2F {
            x: chevron_x - 4.0,
            y: chevron_y - 2.0,
        },
        D2D_POINT_2F {
            x: chevron_x,
            y: chevron_y + 2.0,
        },
        &b.white,
        1.0,
        None,
    );
    rt.DrawLine(
        D2D_POINT_2F {
            x: chevron_x,
            y: chevron_y + 2.0,
        },
        D2D_POINT_2F {
            x: chevron_x + 4.0,
            y: chevron_y - 2.0,
        },
        &b.white,
        1.0,
        None,
    );

    // OK button
    let ok_x = w - MARGIN - BTN_W * 2.0 - 8.0;
    draw_button(&rt, b, f, ok_x, "Run \u{21B5}", HoverId::Ok);

    // Cancel button
    let cancel_x = w - MARGIN - BTN_W;
    draw_button(&rt, b, f, cancel_x, "Cancel", HoverId::Cancel);

    // Dropdown List drawing moved to dropdown_wndproc

    target.EndDraw(None, None).ok();
}

unsafe fn draw_button(
    rt: &ID2D1RenderTarget,
    b: &Brushes,
    f: &Fonts,
    x: f32,
    text: &str,
    id: HoverId,
) {
    let rect = D2D_RECT_F {
        left: x,
        top: BTN_Y,
        right: x + BTN_W,
        bottom: BTN_Y + BTN_H,
    };
    let rounded = D2D1_ROUNDED_RECT {
        rect,
        radiusX: CORNER_RADIUS,
        radiusY: CORNER_RADIUS,
    };

    let bg = if HOVER == id { &b.btn_hover } else { &b.btn_bg };
    rt.FillRoundedRectangle(&rounded, bg);

    let txt: Vec<u16> = text.encode_utf16().collect();
    rt.DrawText(
        &txt,
        &f.button,
        &rect,
        &b.white,
        D2D1_DRAW_TEXT_OPTIONS_NONE,
        DWRITE_MEASURING_MODE_NATURAL,
    );
}
