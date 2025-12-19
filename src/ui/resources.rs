use crate::config::HoverId;
use crate::ui::{Brushes, Fonts};
use std::sync::Mutex;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Imaging::*;

pub static mut D2D_FACTORY: Option<ID2D1Factory> = None;
pub static mut DWRITE_FACTORY: Option<IDWriteFactory> = None;
pub static mut WIC_FACTORY: Option<IWICImagingFactory> = None;
pub static mut RENDER_TARGET: Option<ID2D1HwndRenderTarget> = None;
pub static mut TOOLTIP_RENDER_TARGET: Option<ID2D1HwndRenderTarget> = None;
pub static mut BRUSHES: Option<Brushes> = None;
pub static mut DROPDOWN_BRUSHES: Option<Brushes> = None;
pub static mut FONTS: Option<Fonts> = None;

pub static mut HOVER: HoverId = HoverId::None;
pub static mut APP_ICON_BITMAP: Option<ID2D1Bitmap> = None;

pub static mut H_MAIN: HWND = HWND(0);
pub static mut H_EDIT: HWND = HWND(0);
pub static mut H_DROPDOWN: HWND = HWND(0);
pub static mut H_TOOLTIP: HWND = HWND(0);

pub static INPUT_BUFFER: Mutex<String> = Mutex::new(String::new());

// Message constants
pub const WM_APP_RUN_COMMAND: u32 = 1025;
pub const WM_APP_CLOSE: u32 = 1026;
pub const WM_APP_ERROR: u32 = 1027;
