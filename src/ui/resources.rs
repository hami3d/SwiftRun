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

pub static mut H_MAIN: HWND = HWND(std::ptr::null_mut());
pub static mut H_EDIT: HWND = HWND(std::ptr::null_mut());
pub static mut H_DROPDOWN: HWND = HWND(std::ptr::null_mut());
pub static mut H_TOOLTIP: HWND = HWND(std::ptr::null_mut());

pub static INPUT_BUFFER: Mutex<String> = Mutex::new(String::new());

pub static mut CACHED_TEXT_LAYOUT: Option<IDWriteTextLayout> = None;
pub static mut CACHED_TEXT: String = String::new();

pub static mut CACHED_GHOST_LAYOUT: Option<IDWriteTextLayout> = None;
pub static mut CACHED_GHOST_TEXT: String = String::new();
pub static mut CACHED_GHOST_PREDICTION_SOURCE: String = String::new();
pub static mut CACHED_GHOST_INPUT_LEN: usize = 0;

pub static mut CACHED_PLACEHOLDER_LAYOUT: Option<IDWriteTextLayout> = None;

pub static mut CACHED_SEL_START: u32 = 0;
pub static mut CACHED_SEL_END: u32 = 0;
pub static mut CACHED_SEL_X1: f32 = 0.0;
pub static mut CACHED_SEL_X2: f32 = 0.0;

// Message constants
pub const WM_APP_RUN_COMMAND: u32 = 1025;
pub const WM_APP_CLOSE: u32 = 1026;
pub const WM_APP_ERROR: u32 = 1027;
pub const WM_APP_SHOW_UI: u32 = 1028;
