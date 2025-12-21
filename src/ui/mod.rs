use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Controls::*;
use windows::core::*;

use crate::config::*;

pub mod dialog;
pub mod dropdown;
pub mod main_win;
pub mod resources;
pub mod tooltip;

pub struct Brushes {
    pub placeholder: ID2D1SolidColorBrush,
    pub white: ID2D1SolidColorBrush,
    pub gray: ID2D1SolidColorBrush,
    pub input_bg: ID2D1SolidColorBrush,
    pub btn_bg: ID2D1SolidColorBrush,
    pub btn_hover: ID2D1SolidColorBrush,
    pub close_hover: ID2D1SolidColorBrush,
    pub accent: ID2D1SolidColorBrush,
    pub accent_hover: ID2D1SolidColorBrush,
    pub selection: ID2D1SolidColorBrush,
    pub btn_border: ID2D1SolidColorBrush,
}

pub struct Fonts {
    pub title: IDWriteTextFormat,
    pub label: IDWriteTextFormat,
    pub button: IDWriteTextFormat,
    pub tooltip: IDWriteTextFormat,
    pub tooltip_bold: IDWriteTextFormat,
    pub icon: IDWriteTextFormat,
}

#[repr(C)]
pub struct AccentPolicy {
    pub accent_state: u32,
    pub accent_flags: u32,
    pub gradient_color: u32,
    pub animation_id: u32,
}

#[repr(C)]
pub struct WindowCompositionAttribData {
    pub attribute: u32,
    pub data: *mut std::ffi::c_void,
    pub size: usize,
}

pub unsafe fn is_dark_mode() -> bool {
    let mut buffer = [0u8; 4];
    let mut cb_data = 4u32;
    let mut h_key = HKEY::default();
    if RegOpenKeyExW(
        HKEY_CURRENT_USER,
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
        Some(0),
        KEY_READ,
        &mut h_key,
    )
    .is_ok()
    {
        let _ = RegQueryValueExW(
            h_key,
            w!("AppsUseLightTheme"),
            None,
            None,
            Some(buffer.as_mut_ptr()),
            Some(&mut cb_data),
        );
        let _ = RegCloseKey(h_key);
    }
    buffer[0] == 0
}

pub unsafe fn get_accent_color_values() -> (f32, f32, f32) {
    let mut color: u32 = 0;
    let mut cb_data = 4u32;
    let mut h_key = HKEY::default();
    if RegOpenKeyExW(
        HKEY_CURRENT_USER,
        w!("Software\\Microsoft\\Windows\\DWM"),
        Some(0),
        KEY_READ,
        &mut h_key,
    )
    .is_ok()
    {
        let _ = RegQueryValueExW(
            h_key,
            w!("AccentColor"),
            None,
            None,
            Some(&mut color as *mut _ as *mut _),
            Some(&mut cb_data),
        );
        let _ = RegCloseKey(h_key);
    }
    if color == 0 {
        return (
            COLOR_ACCENT_DEFAULT_R,
            COLOR_ACCENT_DEFAULT_G,
            COLOR_ACCENT_DEFAULT_B,
        );
    }
    let r = (color & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = ((color >> 16) & 0xFF) as f32 / 255.0;
    (r, g, b)
}

pub unsafe fn set_acrylic_effect(hwnd: HWND) {
    let is_dark = is_dark_mode();
    let gradient_color = if is_dark {
        ACRYLIC_TINT_DARK
    } else {
        ACRYLIC_TINT_LIGHT
    };

    let policy = AccentPolicy {
        accent_state: 4, // ACCENT_ENABLE_ACRYLICBLURBEHIND
        accent_flags: 2,
        gradient_color,
        animation_id: 0,
    };

    let mut data = WindowCompositionAttribData {
        attribute: 19, // WCA_ACCENT_POLICY
        data: &policy as *const _ as *mut _,
        size: std::mem::size_of::<AccentPolicy>(),
    };

    type SetWindowCompositionAttributeFunc =
        unsafe extern "system" fn(HWND, *mut WindowCompositionAttribData) -> BOOL;

    if let Ok(user32) = windows::Win32::System::LibraryLoader::GetModuleHandleW(w!("user32.dll")) {
        if let Some(addr) = windows::Win32::System::LibraryLoader::GetProcAddress(
            user32,
            s!("SetWindowCompositionAttribute"),
        ) {
            let func: SetWindowCompositionAttributeFunc = std::mem::transmute(addr);
            func(hwnd, &mut data);
        }
    }

    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
}

pub unsafe fn get_dpi_scale(hwnd: HWND) -> f32 {
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    let dpi = GetDpiForWindow(hwnd);
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

// Re-exports
pub use dialog::*;
pub use dropdown::*;
pub use main_win::*;
pub use tooltip::*;
