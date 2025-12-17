#![allow(static_mut_refs)]
#![allow(non_snake_case)]

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::Instant;
use windows::{
    core::*,
    Foundation::Numerics::Matrix3x2,
    Win32::Foundation::*,
    Win32::Graphics::Direct2D::{
        Common::{
            D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_POINT_2F,
            D2D_RECT_F, D2D_SIZE_U,
        },
        D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1Layer,
        ID2D1RenderTarget, ID2D1SolidColorBrush, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_OPTIONS,
        D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
        D2D1_LAYER_PARAMETERS, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
        D2D1_ROUNDED_RECT,
    },
    Win32::Graphics::DirectWrite::*,
    Win32::Graphics::Dwm::*,
    Win32::Graphics::Dxgi::Common::*,
    Win32::Graphics::Gdi::*,
    Win32::Graphics::Imaging::*,
    Win32::System::Com::*,
    Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
    Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress},
    Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
    Win32::System::SystemInformation::GetTickCount,
    Win32::UI::Controls::{EM_GETSEL, EM_SETSEL, MARGINS},
    Win32::UI::HiDpi::{GetDpiForWindow, SetProcessDpiAwareness},
    Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, SetFocus, VK_BACK, VK_CONTROL, VK_DOWN, VK_RETURN, VK_SHIFT, VK_UP,
    },
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
static mut HISTORY_INDEX: isize = -1; // -1 = current input, 0 = latest history

// Error Dialog State
// Tooltip state
static mut TOOLTIP_TEXT: String = String::new();
static mut H_TOOLTIP: HWND = HWND(0);

static mut IS_CYCLING: bool = false;

#[derive(Debug, Clone, Copy, PartialEq)]
enum AnimType {
    None,
    Entering,
    Exiting,
}

static mut ANIM_START_TIME: Option<Instant> = None;
static mut ANIM_TYPE: AnimType = AnimType::None;
static mut FINAL_X: i32 = 0;
static mut FINAL_Y: i32 = 0;
static mut START_Y: i32 = 0;
const ANIM_ENTER_DURATION_MS: u128 = 750;
const ANIM_EXIT_DURATION_MS: u128 = 200;
const ANIM_DROPDOWN_DURATION_MS: u128 = 250;
const ANIM_TOOLTIP_DURATION_MS: u128 = 300;

static mut DROPDOWN_ANIM_START: Option<Instant> = None;
static mut DROPDOWN_ANIM_TYPE: AnimType = AnimType::None;

static mut TOOLTIP_ANIM_START: Option<Instant> = None;
static mut TOOLTIP_ANIM_TYPE: AnimType = AnimType::None;

fn ease_out_quad(t: f32) -> f32 {
    t * (2.0 - t)
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

// Static strings caching
static STR_TITLE: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
static STR_RUN: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
static STR_CANCEL: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();

fn get_str_title() -> &'static [u16] {
    STR_TITLE
        .get_or_init(|| "Swift Run".encode_utf16().collect())
        .as_slice()
}
fn get_str_run() -> &'static [u16] {
    STR_RUN
        .get_or_init(|| "Run \u{21B5}".encode_utf16().collect())
        .as_slice()
}
fn get_str_cancel() -> &'static [u16] {
    STR_CANCEL
        .get_or_init(|| "Cancel".encode_utf16().collect())
        .as_slice()
}

fn is_input_empty() -> bool {
    if let Ok(lock) = INPUT_BUFFER.lock() {
        lock.trim().is_empty()
    } else {
        true
    }
}

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
// const DESC_Y: f32 = 38.0; // Removed unused
const INPUT_Y: f32 = 47.0; //input text box vertical position in y axis
const INPUT_H: f32 = 32.0;
const BTN_Y: f32 = 103.0;
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
static mut APP_ICON_BITMAP: Option<ID2D1Bitmap> = None;
static mut WIC_FACTORY: Option<IWICImagingFactory> = None;

struct Brushes {
    white: ID2D1SolidColorBrush,
    gray: ID2D1SolidColorBrush,
    input_bg: ID2D1SolidColorBrush,
    btn_bg: ID2D1SolidColorBrush,
    btn_hover: ID2D1SolidColorBrush,
    close_hover: ID2D1SolidColorBrush,
    accent: ID2D1SolidColorBrush,
    accent_hover: ID2D1SolidColorBrush,
    selection: ID2D1SolidColorBrush,
    input_border: ID2D1SolidColorBrush,
    placeholder: ID2D1SolidColorBrush,
}

struct Fonts {
    title: IDWriteTextFormat,
    label: IDWriteTextFormat,
    button: IDWriteTextFormat,
    tooltip: IDWriteTextFormat,
}

#[repr(C)]
struct AccentPolicy {
    AccentState: u32,
    AccentFlags: u32,
    GradientColor: u32,
    AnimationId: u32,
}

#[repr(C)]
struct WindowCompositionAttribData {
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
                *mut WindowCompositionAttribData,
            ) -> i32 = std::mem::transmute(func);

            let is_dark = is_dark_mode();
            let tint = if is_dark { 0xCC000000 } else { 0xCCF3F3F3 };

            let mut policy = AccentPolicy {
                AccentState: 4, // ACCENT_ENABLE_ACRYLICBLURBEHIND
                AccentFlags: 0,
                GradientColor: tint, // AABBGGRR
                AnimationId: 0,
            };

            let mut data = WindowCompositionAttribData {
                Attrib: 19, // WCA_ACCENT_POLICY
                pvData: &mut policy as *mut _ as _,
                cbData: std::mem::size_of::<AccentPolicy>(),
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

fn get_accent_color_values() -> (f32, f32, f32) {
    unsafe {
        let mut color: u32 = 0;
        let mut opaque: BOOL = BOOL(0);
        if DwmGetColorizationColor(&mut color, &mut opaque).is_ok() {
            let r = ((color >> 16) & 0xFF) as f32 / 255.0;
            let g = ((color >> 8) & 0xFF) as f32 / 255.0;
            let b = (color & 0xFF) as f32 / 255.0;
            (r, g, b)
        } else {
            // Default fallback (blue-ish)
            (0.0, 0.47, 0.84)
        }
    }
}

fn main() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok();
        load_history();

        // Set DPI awareness - Per Monitor V2
        let _ = SetProcessDpiAwareness(windows::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE);

        let instance = GetModuleHandleW(None)?;
        let class_name = w!("SwiftRunClass");

        // Load Icon early for both windows
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

        // Position at bottom-left of work area (like original Run dialog)
        let mut work_area = RECT::default();
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut work_area as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );

        let margin_px = 18; // Space from edge/taskbar

        let x = work_area.left + margin_px;
        let y = work_area.bottom - WIN_H as i32 - margin_px;

        // Register Dropdown Class
        let dropdown_class_name = w!("SwiftRunDropdown");
        // Reuse h_icon for dropdown class too, or just use it.
        // The original code was loading it again or failing to compile because h_icon was below.
        // We already moved h_icon definition up. We can just use `h_icon` here or copy it? HICON is Copy.

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

        // Register Tooltip Class
        let tooltip_class_name = w!("SwiftRunTooltip");
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

        FINAL_X = x;
        FINAL_Y = y;
        START_Y = work_area.bottom;

        // Create Main Window initially at bottom (hidden or off-screen)
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("SwiftRun"),
            WS_POPUP | WS_VISIBLE, // Removed WS_CLIPCHILDREN - D2D doesn't respect it
            x,
            START_Y, // Start at bottom edge
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
        H_DROPDOWN = h_dropdown;

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
            0, // Size handled in WM_SIZE
            hwnd,
            HMENU(EDIT_ID as _),
            instance,
            None,
        );

        // Set Font for Edit Control
        // We'll do this in WM_SIZE or ensuring resources, or just create a stock font/system font?
        // Ideally we use the same DirectWrite font, but Edit controls need GDI HFONT.
        // Let's create a GDI font.
        let hfont = CreateFontW(20, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"));
        SendMessageW(H_EDIT, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));

        // Init D2D
        D2D_FACTORY = D2D1CreateFactory(
            D2D1_FACTORY_TYPE_SINGLE_THREADED,
            Some(&D2D1_FACTORY_OPTIONS::default()),
        )
        .ok();
        DWRITE_FACTORY = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok();

        // Setup blink timer
        let blink_time = GetCaretBlinkTime();
        let blink_time = if blink_time == 0 { 500 } else { blink_time };
        SetTimer(hwnd, 1, blink_time, None);

        // Setup Animation Timer
        ANIM_TYPE = AnimType::Entering;
        ANIM_START_TIME = Some(Instant::now());
        SetTimer(hwnd, 3, 10, None);

        SetFocus(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            // Keyboard hooks for Edit control
            if (msg.message == WM_KEYDOWN || msg.message == WM_KEYUP) && msg.hwnd == H_EDIT {
                let vk = msg.wParam.0 as i32;

                if msg.message == WM_KEYDOWN {
                    if vk == VK_UP.0 as i32 {
                        cycle_history(-1); // Swapped: Up is now newer
                        InvalidateRect(hwnd, None, BOOL(0)); // Repaint to show new text
                        continue;
                    }
                    if vk == VK_DOWN.0 as i32 {
                        cycle_history(1); // Swapped: Down is now older
                        InvalidateRect(hwnd, None, BOOL(0)); // Repaint to show new text
                        continue;
                    }

                    // Ctrl+Shift+Backspace to clear history
                    let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
                    let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;
                    if ctrl && shift && vk == VK_BACK.0 as i32 {
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
                        InvalidateRect(hwnd, None, BOOL(0));
                        continue;
                    }

                    if vk == VK_RETURN.0 as i32 {
                        // Run command
                        let main_hwnd = FindWindowW(w!("SwiftRunClass"), w!("SwiftRun"));
                        PostMessageW(main_hwnd, WM_APP_RUN_COMMAND, WPARAM(0), LPARAM(0));
                        continue;
                    }
                }

                // For any other key (shortcuts like Ctrl+A, Shift+Arrows, etc), trigger repaint
                InvalidateRect(hwnd, None, BOOL(0));
            }

            // Allow standard processing
            if msg.message == WM_KEYDOWN {
                if msg.wParam.0 == 0x1B {
                    // VK_ESCAPE
                    start_exit_animation(hwnd);
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

fn hit_test(x: i32, y: i32, w: f32, _h: f32, input_empty: bool) -> HoverId {
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
        return if input_empty {
            HoverId::None
        } else {
            HoverId::Ok
        };
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

unsafe fn start_exit_animation(hwnd: HWND) {
    if ANIM_TYPE == AnimType::Exiting {
        return;
    }
    // Dismiss dropdown and tooltip immediately
    if SHOW_DROPDOWN {
        SHOW_DROPDOWN = false;
        ShowWindow(H_DROPDOWN, SW_HIDE);
    }
    if H_TOOLTIP.0 != 0 {
        DestroyWindow(H_TOOLTIP);
        H_TOOLTIP = HWND(0);
    }

    ANIM_TYPE = AnimType::Exiting;
    ANIM_START_TIME = Some(Instant::now());
    SetTimer(hwnd, 3, 10, None);
}

fn run_command() {
    unsafe {
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

        let main_hwnd = FindWindowW(w!("SwiftRunClass"), w!("SwiftRun"));

        // 1. Check for Protocol/URL (regex-like check)
        // If it looks like a URL, let ShellExecute handle it entirely.
        let is_url = input_str.starts_with("http")
            || input_str.starts_with("www")
            || input_str.contains("://");

        thread::spawn(move || {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let mut file_path = input_str.clone();
            let mut params = String::new();
            let mut verb = PCWSTR::null();
            let mut _admin_mode = false;

            if !is_url {
                if GetKeyState(VK_CONTROL.0 as i32) < 0 && GetKeyState(VK_SHIFT.0 as i32) < 0 {
                    // Admin mode
                    _admin_mode = true;
                    verb = w!("runas");
                }

                // Parse command vs args
                // (Simplified logic for thread: just run what we parsed)
                // Actually we need to parse inside thread or before.
                // Let's parse here for simplicity or just run basic split.
                // Re-using logic:
                if let Some(idx) = input_str.find(' ') {
                    file_path = input_str[..idx].to_string();
                    params = input_str[idx + 1..].to_string();
                }
            }

            // Secure Quote Handling
            if params.is_empty() && file_path.contains(' ') && !file_path.starts_with('"') {
                file_path = format!("\"{}\"", file_path);
            }

            // Execute
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
                PostMessageW(main_hwnd, WM_APP_CLOSE, WPARAM(0), LPARAM(0));
            } else {
                // Determine if we should show error
                // Just post error
                PostMessageW(main_hwnd, WM_APP_ERROR, WPARAM(0), LPARAM(0));
            }
        });
    }
}

unsafe fn cycle_history(delta: isize) {
    let mut history_len = 0;
    if let Some(h) = HISTORY.as_ref() {
        history_len = h.len() as isize;
    }

    if history_len == 0 {
        return;
    }

    // Update index
    let new_index = HISTORY_INDEX + delta;

    // Bounds check
    if new_index < -1 {
        // Wrap to end? Or stop? Standard is stop at -1
        HISTORY_INDEX = -1;
    } else if new_index >= history_len {
        HISTORY_INDEX = history_len - 1;
    } else {
        HISTORY_INDEX = new_index;
    }

    let text_to_set = if HISTORY_INDEX == -1 {
        // Restore empty? Or saved buffer?
        // Ideally we save current input before starting cycle.
        // For now just clear or keep.
        String::new()
    } else {
        if let Some(h) = HISTORY.as_ref() {
            // History uses insert(0), so index 0 = newest, index len-1 = oldest
            // HISTORY_INDEX 0 = most recent, so real_index = HISTORY_INDEX directly
            let real_index = HISTORY_INDEX;
            if real_index >= 0 && real_index < history_len {
                h[real_index as usize].clone()
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    };

    IS_CYCLING = true;
    // Set text
    SetWindowTextW(
        H_EDIT,
        PCWSTR(
            text_to_set
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>()
                .as_ptr(),
        ),
    );
    // Select all
    SendMessageW(H_EDIT, EM_SETSEL, WPARAM(0), LPARAM(-1 as isize));

    // EM_SETSEL 0,-1 highlights all.
    IS_CYCLING = false;
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
            WM_APP_RUN_COMMAND => {
                run_command();
                LRESULT(0)
            }
            WM_APP_CLOSE => {
                start_exit_animation(hwnd);
                LRESULT(0)
            }
            WM_APP_ERROR => {
                ShowWindow(hwnd, SW_SHOW);
                // We can't easily get the exact string back from thread without logic,
                // but we can just show a generic error or read from input buffer again.
                if let Ok(buf) = INPUT_BUFFER.lock() {
                    show_tooltip(&format!("This Command Doesn't Exist: '{}'!", buf));
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                start_exit_animation(hwnd);
                LRESULT(0)
            }
            WM_SIZE => {
                if let Some(target) = RENDER_TARGET.as_ref() {
                    let size = D2D_SIZE_U {
                        width: (lp.0 & 0xFFFF) as u32,
                        height: ((lp.0 >> 16) & 0xFFFF) as u32,
                    };
                    target.Resize(&size).ok();
                }

                // Resize Edit Control
                let w = (lp.0 & 0xFFFF) as i32;
                SetWindowPos(
                    H_EDIT,
                    None,
                    -10000, // Move off-screen - Edit control is hidden, only used for input handling
                    -10000,
                    w,
                    50,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );

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
                } else {
                    // Activated
                    SetFocus(H_EDIT);
                    SendMessageW(H_EDIT, EM_SETSEL, WPARAM(0), LPARAM(-1 as isize));
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

            WM_CTLCOLOREDIT => {
                let hdc = HDC(wp.0 as _);
                let is_dark = is_dark_mode();

                // Set text color
                let text_col = if is_dark { 0x00FFFFFF } else { 0x00000000 };
                SetTextColor(hdc, COLORREF(text_col));

                // Return generic background brush?
                // Actually if we want transparent background (acrylic), Edit controls don't support true transparency well.
                // We fake it by returning a brush that matches the acrylic tint?
                // Or we just use a solid dark/light background for the box.
                // User asked for "Highlight visible... like text editor".
                // Solid background is safer for Edit control readability.
                // Let's use a solid brush matching the theme.

                // For now, let's use GetStockObject(NULL_BRUSH) to see if we can get transparency?
                // Often produces artifacts.
                // Better: CreateSolidBrush. To avoid leaking, store it?
                // Creating leaky brush for now (optimization later) or use system colors.

                let _bg_col = if is_dark { 0x00202020 } else { 0x00F3F3F3 }; // BGR
                                                                             // SetBkColor(hdc, COLORREF(bg_col)); // Set text background

                // Actually, let's just let it use default system logic?
                // Or simple:
                // If Dark: Text White, BG Black.
                if is_dark {
                    SetTextColor(hdc, COLORREF(0x00FFFFFF));
                    SetBkColor(hdc, COLORREF(0x002C2C2C)); // Slightly lighter than pure black
                    static mut DARK_BRUSH: HBRUSH = HBRUSH(0);
                    if DARK_BRUSH.0 == 0 {
                        DARK_BRUSH = CreateSolidBrush(COLORREF(0x002C2C2C));
                    }
                    LRESULT(DARK_BRUSH.0 as isize)
                } else {
                    // Light mode defaults usually fine?
                    // Ensure text is black.
                    SetTextColor(hdc, COLORREF(0x00000000));
                    SetBkColor(hdc, COLORREF(0x00FFFFFF));
                    static mut LIGHT_BRUSH: HBRUSH = HBRUSH(0);
                    if LIGHT_BRUSH.0 == 0 {
                        LIGHT_BRUSH = CreateSolidBrush(COLORREF(0x00FFFFFF));
                    }
                    LRESULT(LIGHT_BRUSH.0 as isize)
                }
            }

            WM_TIMER => {
                if wp.0 == 1 {
                    InvalidateRect(hwnd, None, BOOL(0));
                } else if wp.0 == 3 {
                    let mut still_animating = false;

                    // Main Window Animation
                    if let Some(start) = ANIM_START_TIME {
                        let elapsed = start.elapsed().as_millis();
                        match ANIM_TYPE {
                            AnimType::Entering => {
                                let progress =
                                    (elapsed as f32 / ANIM_ENTER_DURATION_MS as f32).min(1.0);
                                let eased = ease_out_cubic(progress);
                                let current_y =
                                    START_Y - ((START_Y - FINAL_Y) as f32 * eased) as i32;
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
                                let progress =
                                    (elapsed as f32 / ANIM_EXIT_DURATION_MS as f32).min(1.0);
                                let eased = ease_out_quad(progress);
                                let current_y =
                                    FINAL_Y + ((START_Y - FINAL_Y) as f32 * eased) as i32;
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
                                    DestroyWindow(hwnd);
                                }
                            }
                            _ => {}
                        }
                    }

                    // Dropdown Animation
                    if let Some(start) = DROPDOWN_ANIM_START {
                        let elapsed = start.elapsed().as_millis();
                        let progress = (elapsed as f32 / ANIM_DROPDOWN_DURATION_MS as f32).min(1.0);
                        if progress < 1.0 {
                            still_animating = true;
                        } else if DROPDOWN_ANIM_TYPE == AnimType::Exiting {
                            ShowWindow(H_DROPDOWN, SW_HIDE);
                            DROPDOWN_ANIM_TYPE = AnimType::None;
                        } else {
                            DROPDOWN_ANIM_TYPE = AnimType::None;
                        }
                        InvalidateRect(H_DROPDOWN, None, BOOL(0));
                    }

                    // Tooltip Animation
                    if let Some(start) = TOOLTIP_ANIM_START {
                        let elapsed = start.elapsed().as_millis();
                        let progress = (elapsed as f32 / ANIM_TOOLTIP_DURATION_MS as f32).min(1.0);
                        if progress < 1.0 {
                            still_animating = true;
                        } else if TOOLTIP_ANIM_TYPE == AnimType::Exiting {
                            DestroyWindow(H_TOOLTIP);
                            H_TOOLTIP = HWND(0);
                            TOOLTIP_ANIM_TYPE = AnimType::None;
                        } else {
                            TOOLTIP_ANIM_TYPE = AnimType::None;
                        }
                        InvalidateRect(H_TOOLTIP, None, BOOL(0));
                    }

                    if !still_animating {
                        KillTimer(hwnd, 3);
                    }
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

                let new_hover = hit_test(sx as i32, sy as i32, w, h, is_input_empty());
                if new_hover != HOVER {
                    HOVER = new_hover;
                    InvalidateRect(hwnd, None, BOOL(0));
                }

                if wp.0 & 0x0001 != 0 {
                    // MK_LBUTTON is down, forward for drag selection
                    let lx_px = ((sx - (MARGIN + 10.0)) * scale) as i32;
                    let ly_px = (20.0 * scale) as i32;
                    let n_lp =
                        LPARAM(((lx_px & 0xFFFF) as isize) | (((ly_px & 0xFFFF) as isize) << 16));
                    SendMessageW(H_EDIT, WM_MOUSEMOVE, wp, n_lp);
                    InvalidateRect(hwnd, None, BOOL(0));
                }

                LRESULT(0)
            }

            WM_LBUTTONUP => {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture().ok();
                let x = (lp.0 & 0xFFFF) as i16 as i32;
                let scale = get_dpi_scale(hwnd);
                let sx = x as f32 / scale;
                let lx_px = ((sx - (MARGIN + 10.0)) * scale) as i32;
                let ly_px = (20.0 * scale) as i32;
                let n_lp =
                    LPARAM(((lx_px & 0xFFFF) as isize) | (((ly_px & 0xFFFF) as isize) << 16));
                SendMessageW(H_EDIT, WM_LBUTTONUP, wp, n_lp);
                InvalidateRect(hwnd, None, BOOL(0));
                LRESULT(0)
            }

            WM_LBUTTONDBLCLK => {
                let x = (lp.0 & 0xFFFF) as i16 as i32;
                let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;
                let mut cr = RECT::default();
                GetClientRect(hwnd, &mut cr);
                let scale = get_dpi_scale(hwnd);
                let w = (cr.right - cr.left) as f32 / scale;
                let h = (cr.bottom - cr.top) as f32 / scale;
                let sx = x as f32 / scale;
                let sy = y as f32 / scale;

                if hit_test(sx as i32, sy as i32, w, h, is_input_empty()) == HoverId::Input {
                    let lx_px = ((sx - (MARGIN + 10.0)) * scale) as i32;
                    let ly_px = (20.0 * scale) as i32;
                    let n_lp =
                        LPARAM(((lx_px & 0xFFFF) as isize) | (((ly_px & 0xFFFF) as isize) << 16));
                    SendMessageW(H_EDIT, WM_LBUTTONDBLCLK, wp, n_lp);
                    InvalidateRect(hwnd, None, BOOL(0));
                }
                LRESULT(0)
            }

            WM_CAPTURECHANGED => LRESULT(0),

            WM_LBUTTONDOWN => {
                // Dismiss tooltip if visible
                if H_TOOLTIP.0 != 0 {
                    DestroyWindow(H_TOOLTIP);
                    H_TOOLTIP = HWND(0);
                }

                let x = (lp.0 & 0xFFFF) as i16 as i32;
                let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;

                let mut cr = RECT::default();
                GetClientRect(hwnd, &mut cr);

                let scale = get_dpi_scale(hwnd);
                let w = (cr.right - cr.left) as f32 / scale;
                let h = (cr.bottom - cr.top) as f32 / scale;
                let sx = x as f32 / scale;
                let sy = y as f32 / scale;

                match hit_test(sx as i32, sy as i32, w, h, is_input_empty()) {
                    HoverId::Close => start_exit_animation(hwnd),
                    HoverId::Min => {
                        ShowWindow(hwnd, SW_MINIMIZE);
                    }
                    HoverId::Ok => run_command(),
                    HoverId::Cancel => start_exit_animation(hwnd),
                    HoverId::Input => {
                        windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(hwnd);
                        let lx_px = ((sx - (MARGIN + 10.0)) * scale) as i32;
                        let ly_px = (20.0 * scale) as i32;
                        let n_lp = LPARAM(
                            ((lx_px & 0xFFFF) as isize) | (((ly_px & 0xFFFF) as isize) << 16),
                        );
                        SendMessageW(H_EDIT, WM_LBUTTONDOWN, wp, n_lp);
                        SetFocus(H_EDIT);
                        InvalidateRect(hwnd, None, BOOL(0));
                    }
                    HoverId::None => {
                        SetFocus(H_EDIT);
                        if SHOW_DROPDOWN {
                            SHOW_DROPDOWN = false;
                            ShowWindow(H_DROPDOWN, SW_HIDE);
                            InvalidateRect(hwnd, None, BOOL(0));
                        }
                    }
                    HoverId::Dropdown => {
                        let is_empty = HISTORY.as_ref().map_or(true, |h| h.is_empty());

                        if is_empty {
                            show_tooltip("No Command History Found");
                        } else {
                            if !SHOW_DROPDOWN {
                                // Show
                                SHOW_DROPDOWN = true;
                                HOVER_DROPDOWN = Some(0);
                                SCROLL_OFFSET = 0;

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
                                    let count = history.len().min(5);
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
                                    DROPDOWN_ANIM_START = Some(Instant::now());
                                    DROPDOWN_ANIM_TYPE = AnimType::Entering;
                                    SetTimer(hwnd, 3, 16, None);
                                    UpdateWindow(H_DROPDOWN);
                                } else {
                                    SHOW_DROPDOWN = false;
                                }
                            } else {
                                // Hide
                                SHOW_DROPDOWN = false;
                                DROPDOWN_ANIM_START = Some(Instant::now());
                                DROPDOWN_ANIM_TYPE = AnimType::Exiting;
                                SetTimer(hwnd, 3, 16, None);
                                InvalidateRect(hwnd, None, BOOL(0));
                            }
                        }
                    } // HistoryItem click is now handled in dropdown_wndproc
                }
                LRESULT(0)
            }

            WM_COMMAND => {
                let id = wp.0 & 0xFFFF;
                let code = (wp.0 >> 16) & 0xFFFF;
                if id == EDIT_ID as usize && code == 0x0300 {
                    // EN_CHANGE
                    if !IS_CYCLING {
                        HISTORY_INDEX = -1;
                    }
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

            WM_NCLBUTTONDOWN => {
                if SHOW_DROPDOWN {
                    SHOW_DROPDOWN = false;
                    ShowWindow(H_DROPDOWN, SW_HIDE);
                    InvalidateRect(hwnd, None, BOOL(0));
                }
                DefWindowProcW(hwnd, msg, wp, lp)
            }

            WM_KEYDOWN => {
                let vk = wp.0 as u16;
                // Arrow keys for history navigation when dropdown is closed
                if !SHOW_DROPDOWN {
                    if vk == VK_UP.0 {
                        cycle_history(-1); // Swapped: Up is now newer
                        InvalidateRect(hwnd, None, BOOL(0));
                        return LRESULT(0);
                    } else if vk == VK_DOWN.0 {
                        cycle_history(1); // Swapped: Down is now older
                        InvalidateRect(hwnd, None, BOOL(0));
                        return LRESULT(0);
                    }
                }
                DefWindowProcW(hwnd, msg, wp, lp)
            }

            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_APP_RUN_COMMAND: u32 = 1025; // WM_USER + 1
const WM_APP_CLOSE: u32 = 1026;
const WM_APP_ERROR: u32 = 1027;

unsafe extern "system" fn dropdown_wndproc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_CREATE => {
            set_acrylic_effect(hwnd);

            // Round corners
            let v: i32 = 2; // DWMWCP_ROUND
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(33), // DWMWA_WINDOW_CORNER_PREFERENCE
                &v as *const _ as _,
                4,
            );
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_SIZE => {
            DROPDOWN_BRUSHES = None; // Force brush refresh on next paint
            InvalidateRect(hwnd, None, BOOL(0));
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_ERASEBKGND => LRESULT(1), // Prevent flicker
        windows::Win32::UI::WindowsAndMessaging::WM_SHOWWINDOW => {
            if wp.0 == 1 {
                InvalidateRect(hwnd, None, BOOL(0));
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
                        if let Ok(rt) = target.cast::<ID2D1RenderTarget>() {
                            rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                            target.BeginDraw();

                            // Draw background
                            let size = target.GetSize();
                            let w = size.width;
                            let h = size.height;

                            // Animation logic
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

                            // We can't easily set global alpha on brushes here without recreating them,
                            // but we can PushLayer specifically for opacity.
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
                                for (i, item) in
                                    history.iter().skip(SCROLL_OFFSET).take(5).enumerate()
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

                            target.PopLayer();
                            rt.SetTransform(&Matrix3x2::identity());

                            let result = target.EndDraw(None, None);
                            if let Err(e) = result {
                                if e.code() == HRESULT(0x8899000Cu32 as i32) {
                                    DROPDOWN_RENDER_TARGET = None;
                                    DROPDOWN_BRUSHES = None;
                                    InvalidateRect(hwnd, None, BOOL(0));
                                }
                            }
                        }
                    }
                }
            }

            EndPaint(hwnd, &ps);
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
                        unsafe {
                            let main_hwnd = FindWindowW(w!("SwiftRunClass"), w!("SwiftRun"));
                            DROPDOWN_ANIM_START = Some(Instant::now());
                            DROPDOWN_ANIM_TYPE = AnimType::Exiting;
                            SetTimer(main_hwnd, 3, 16, None);
                        }

                        // Set focus back to edit
                        SetFocus(H_EDIT);
                        SendMessageW(H_EDIT, EM_SETSEL, WPARAM(0), LPARAM(-1 as isize));
                    }
                }
            }
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
            } else if HOVER_DROPDOWN.is_some() {
                HOVER_DROPDOWN = None;
                InvalidateRect(hwnd, None, BOOL(0));
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

unsafe fn ensure_fonts() {
    if FONTS.is_some() {
        return;
    }

    let Some(dwrite) = &DWRITE_FACTORY else {
        return;
    };

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

    // Tooltip font - light weight for thin appearance
    let tooltip = dwrite
        .CreateTextFormat(
            w!("Segoe UI Variable Text"),
            None,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            12.0,
            w!(""),
        )
        .unwrap();
    tooltip.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING).ok();
    tooltip
        .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
        .ok();

    FONTS = Some(Fonts {
        title,
        label,
        button,
        tooltip,
    });
}

unsafe fn ensure_dropdown_resources(hwnd: HWND) {
    ensure_fonts();
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
                let res = target.Resize(&windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U {
                    width: w,
                    height: h,
                });
                if res.is_ok() {
                    DROPDOWN_BRUSHES = None; // Recreate brushes for new size
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

    let input_bg_col = 0.0; // Black base for both to ensure darkness
    let input_bg_alpha = if is_dark { 0.1 } else { 0.1 };
    let input_bg = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: input_bg_col,
                g: input_bg_col,
                b: input_bg_col,
                a: input_bg_alpha,
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

    let btn_hover_col = if is_dark { 0.35 } else { 0.8 };
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

    // Init accent brushes for dropdown (even if not used for buttons, kept for consistency or future use)
    // Actually dropdown uses Brushes struct too, so it needs these fields.
    // For dropdown we probably don't use accent strongly, but let's just use defaults or same logic.
    let (ar, ag, ab) = get_accent_color_values();
    let accent = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: ar,
                g: ag,
                b: ab,
                a: 0.9,
            },
            None,
        )
        .unwrap();
    let accent_hover = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: ar,
                g: ag,
                b: ab,
                a: 1.0,
            },
            None,
        )
        .unwrap();

    DROPDOWN_BRUSHES = Some(Brushes {
        placeholder: rt
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: text_col,
                    g: text_col,
                    b: text_col,
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
        input_border: rt
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: gray_col,
                    g: gray_col,
                    b: gray_col,
                    a: 0.1, // Extra subtle
                },
                None,
            )
            .unwrap(),
    });
}

unsafe fn ensure_resources(hwnd: HWND) {
    if RENDER_TARGET.is_none() {
        let Some(factory) = &D2D_FACTORY else { return };
        let Some(_dwrite) = &DWRITE_FACTORY else {
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

    let btn_bg_col = if is_dark { 0.2 } else { 0.94 }; // Soft gray resting
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

    let btn_hover_col = if is_dark { 0.35 } else { 0.88 }; // Deeper gray hover
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
                a: 0.9,
            },
            None,
        )
        .unwrap();
    let accent_hover = rt
        .CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: ar,
                g: ag,
                b: ab,
                a: 1.0,
            },
            None,
        )
        .unwrap();

    BRUSHES = Some(Brushes {
        placeholder: rt
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: text_col,
                    g: text_col,
                    b: text_col,
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
        input_border: rt
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: gray_col,
                    g: gray_col,
                    b: gray_col,
                    a: 0.15, // Extra subtle
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
            // Load Icon
            let icon_handle = LoadImageW(
                GetModuleHandleW(None).unwrap(),
                w!("icon.ico"),
                IMAGE_ICON,
                32,
                32,
                LR_DEFAULTCOLOR,
            );

            if let Ok(icon_handle) = icon_handle {
                if let Ok(bmp) = wic_factory.CreateBitmapFromHICON(HICON(icon_handle.0 as _)) {
                    // Check if rt supports creation
                    if let Ok(converter) = wic_factory.CreateFormatConverter() {
                        converter
                            .Initialize(
                                &bmp,
                                &GUID_WICPixelFormat32bppPBGRA,
                                WICBitmapDitherTypeNone,
                                None,
                                0.0,
                                WICBitmapPaletteTypeMedianCut,
                            )
                            .ok();

                        if let Ok(d2d_bmp) = rt.CreateBitmapFromWicBitmap(&converter, None) {
                            APP_ICON_BITMAP = Some(d2d_bmp);
                        }
                    }
                }
            }
        }
    }

    ensure_fonts();
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
    let _h = size.height;

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
    let icon_size = 24.0;
    if let Some(bitmap) = &APP_ICON_BITMAP {
        rt.DrawBitmap(
            bitmap,
            Some(&D2D_RECT_F {
                left: MARGIN - 5.0,
                top: TITLE_Y - 2.0, // Center vertically with text (24px icon)
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

    // Input field
    let input_rect = D2D_RECT_F {
        left: MARGIN,
        top: INPUT_Y,
        right: w - MARGIN,
        bottom: INPUT_Y + INPUT_H,
    };
    // Note: Native Edit control handles its own background now
    // Only draw a subtle border
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
        &b.input_border,
        1.0,
        None,
    );

    // Draw input text with D2D
    if let Ok(buf) = INPUT_BUFFER.lock() {
        let text_rect = D2D_RECT_F {
            left: MARGIN + 10.0,
            top: INPUT_Y + 8.0,
            right: w - MARGIN - 30.0,
            bottom: INPUT_Y + INPUT_H - 8.0,
        };

        let brush = &b.white;

        if buf.is_empty() {
            // Draw placeholder hint
            let hint = "Search or run a command...";
            let hint_u16: Vec<u16> = hint.encode_utf16().collect();
            if let Some(dwrite) = &DWRITE_FACTORY {
                if let Ok(layout) = dwrite.CreateTextLayout(
                    &hint_u16,
                    &f.button,
                    text_rect.right - text_rect.left,
                    text_rect.bottom - text_rect.top,
                ) {
                    layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING).ok();
                    rt.DrawTextLayout(
                        D2D_POINT_2F {
                            x: text_rect.left,
                            y: text_rect.top,
                        },
                        &layout,
                        &b.placeholder,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                    );
                }
            }
        } else {
            // Use TextLayout for selection highlighting and text rendering
            if let Some(dwrite) = &DWRITE_FACTORY {
                let text_u16: Vec<u16> = buf.encode_utf16().collect();
                if let Ok(layout) = dwrite.CreateTextLayout(
                    &text_u16,
                    &f.button,
                    text_rect.right - text_rect.left,
                    text_rect.bottom - text_rect.top,
                ) {
                    layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING).ok();

                    // 1. Draw Selection Highlight
                    let lresult = unsafe { SendMessageW(H_EDIT, EM_GETSEL, WPARAM(0), LPARAM(0)) };
                    let start = (lresult.0 & 0xFFFF) as u32; // Low-order word
                    let end = ((lresult.0 >> 16) & 0xFFFF) as u32; // High-order word

                    if start != end {
                        let mut x1: f32 = 0.0;
                        let mut y1: f32 = 0.0;
                        let mut metrics1: DWRITE_HIT_TEST_METRICS = std::mem::zeroed();
                        layout
                            .HitTestTextPosition(start, false, &mut x1, &mut y1, &mut metrics1)
                            .ok();

                        let mut x2: f32 = 0.0;
                        let mut y2: f32 = 0.0;
                        let mut metrics2: DWRITE_HIT_TEST_METRICS = std::mem::zeroed();
                        layout
                            .HitTestTextPosition(end, false, &mut x2, &mut y2, &mut metrics2)
                            .ok();

                        let sel_rect = D2D_RECT_F {
                            left: text_rect.left + x1,
                            top: text_rect.top,
                            right: text_rect.left + x2,
                            bottom: text_rect.bottom,
                        };
                        rt.FillRectangle(&sel_rect, &b.selection);
                    }

                    // 2. Draw Text
                    rt.DrawTextLayout(
                        D2D_POINT_2F {
                            x: text_rect.left,
                            y: text_rect.top,
                        },
                        &layout,
                        brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                    );
                }
            }
        }

        // Draw blinking cursor
        let blink_time = GetCaretBlinkTime();
        let blink_time = if blink_time == 0 { 500 } else { blink_time };
        let tick = GetTickCount();
        if (tick / blink_time) % 2 == 0 {
            // Calculate cursor X position using TextLayout for accuracy
            let cursor_x = if buf.is_empty() {
                text_rect.left
            } else {
                // Use DirectWrite TextLayout to get actual text width
                if let Some(dwrite) = &DWRITE_FACTORY {
                    let text_u16: Vec<u16> = buf.encode_utf16().collect();
                    if let Ok(layout) = dwrite.CreateTextLayout(
                        &text_u16,
                        &f.button,
                        text_rect.right - text_rect.left,
                        text_rect.bottom - text_rect.top,
                    ) {
                        // Set left alignment for proper cursor positioning
                        layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING).ok();

                        // Query actual caret position from standard edit control
                        let mut start: u32 = 0;
                        let mut end: u32 = 0;
                        SendMessageW(
                            H_EDIT,
                            EM_GETSEL,
                            WPARAM(&mut start as *mut _ as usize),
                            LPARAM(&mut end as *mut _ as isize),
                        );

                        let mut x: f32 = 0.0;
                        let mut y: f32 = 0.0;
                        let mut metrics: DWRITE_HIT_TEST_METRICS = std::mem::zeroed();
                        // Get position at the caret (end of selection)
                        layout
                            .HitTestTextPosition(end, false, &mut x, &mut y, &mut metrics)
                            .ok();
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
                    x: cursor_x,
                    y: text_rect.top,
                },
                D2D_POINT_2F {
                    x: cursor_x,
                    y: text_rect.bottom,
                },
                &b.white,
                1.5,
                None,
            );
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
    let input_empty = is_input_empty();
    draw_button(&rt, b, f, ok_x, get_str_run(), HoverId::Ok, input_empty);

    // Cancel button
    let cancel_x = w - MARGIN - BTN_W;
    draw_button(
        &rt,
        b,
        f,
        cancel_x,
        get_str_cancel(),
        HoverId::Cancel,
        false,
    );

    // Dropdown List drawing moved to dropdown_wndproc

    target.EndDraw(None, None).ok();
}

unsafe fn draw_button(
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
    let rounded = D2D1_ROUNDED_RECT {
        rect,
        radiusX: CORNER_RADIUS,
        radiusY: CORNER_RADIUS,
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
    rt.FillRoundedRectangle(&rounded, bg);

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

unsafe fn show_tooltip(msg: &str) {
    if H_TOOLTIP.0 != 0 {
        DestroyWindow(H_TOOLTIP);
    }

    TOOLTIP_TEXT = msg.to_string();

    let mut main_rect = RECT::default();
    let main_hwnd = FindWindowW(w!("SwiftRunClass"), w!("SwiftRun"));
    GetWindowRect(main_hwnd, &mut main_rect);

    // Calculate dynamic width based on text length
    let text_len = msg.chars().count();
    let char_width: usize = 8; // Characters ~8px wide
                               //let padding = 1; // Left + right padding
    let min_width: usize = 1;
    let max_width = WIN_W as usize; // Cap at main window width
    let width = (text_len * char_width.max(min_width).min(max_width) - 25) as i32; //minus margins
    let height = 22; // Compact
    let dpi_scale = get_dpi_scale(main_hwnd);

    // Check if main window is visible/valid?
    // Assuming yes.

    let x = main_rect.left + (WIN_W as i32 - width) / 2;
    // Calculate Y based on Input Y
    let input_y_screen = main_rect.top + ((INPUT_Y * dpi_scale) as i32);
    let y = input_y_screen - height - 10; // 10px padding above input

    let instance = GetModuleHandleW(None).unwrap();
    H_TOOLTIP = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        w!("SwiftRunTooltip"),
        w!(""),
        WS_POPUP, // Removed WS_VISIBLE to prevent focus steal
        x,
        y,
        width,
        height,
        main_hwnd,
        None,
        instance,
        None,
    );

    // Rounded corners (Small) - Windows 11 rounded appearance
    let v: i32 = 3; // DWMWCP_ROUNDSMALL
    DwmSetWindowAttribute(H_TOOLTIP, DWMWINDOWATTRIBUTE(33), &v as *const _ as _, 4).ok();

    // Show without activating
    ShowWindow(H_TOOLTIP, SW_SHOWNOACTIVATE);

    TOOLTIP_ANIM_START = Some(Instant::now());
    TOOLTIP_ANIM_TYPE = AnimType::Entering;
    SetTimer(main_hwnd, 3, 16, None);

    // Auto-close timer
    SetTimer(H_TOOLTIP, 2, 8000, None);

    // Highlight text in input box
    SetFocus(H_EDIT);
    SendMessageW(H_EDIT, EM_SETSEL, WPARAM(0), LPARAM(-1 as isize));
}

unsafe extern "system" fn tooltip_wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
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
            // Click to dismiss
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

                    // Re-use target or create new (ephemeral is ok for tooltip)
                    if let Ok(target) = factory.CreateHwndRenderTarget(&props, &hwnd_props) {
                        target.BeginDraw();

                        // Animation Logic
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
                                    alpha = progress.min(1.0); // Quick fade
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

                        // Transformation for Pop effect
                        let mid_x = w as f32 / 2.0;
                        let mid_y = h as f32 / 2.0;
                        target.SetTransform(
                            &(Matrix3x2::translation(-mid_x, -mid_y)
                                * Matrix3x2 {
                                    M11: scale,
                                    M12: 0.0,
                                    M21: 0.0,
                                    M22: scale,
                                    M31: 0.0,
                                    M32: 0.0,
                                }
                                * Matrix3x2::translation(mid_x, mid_y + y_off)),
                        );

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

                        // Draw Background
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

                        // Text Color
                        let text_col = if is_dark { 1.0 } else { 0.2 }; // Dark gray #323130 is approx 0.2 in RGB
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
                                // Use TextLayout for proper left alignment
                                if let Some(dwrite) = &DWRITE_FACTORY {
                                    if let Ok(layout) = dwrite.CreateTextLayout(
                                        &msg_u16,
                                        &f.tooltip,      // Light weight tooltip font
                                        w as f32 - 12.0, // Width for text wrapping
                                        h as f32,
                                    ) {
                                        // Explicitly set LEFT alignment and VERTICAL center
                                        layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING).ok();
                                        layout
                                            .SetParagraphAlignment(
                                                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                                            )
                                            .ok();
                                        target.DrawTextLayout(
                                            D2D_POINT_2F {
                                                x: 6.0,  // Left padding
                                                y: -2.0, // Negative to move text UP for centering
                                            },
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

                        target.EndDraw(None, None).ok();
                    }
                }
            }
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
