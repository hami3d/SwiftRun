use std::sync::OnceLock;

// ==================================================================================
//  GLOBAL UI CONFIGURATION
//  This file contains all the design tokens, dimensions, colors, and assets settings
//  for the application. Change values here to theme the app.
// ==================================================================================

// ----------------------------------------------------------------------------------
//  WINDOW DIMENSIONS & LAYOUT
// ----------------------------------------------------------------------------------
pub const WIN_W: f32 = 450.0;
pub const WIN_H: f32 = 150.0;
pub const CORNER_RADIUS: f32 = 5.0; // Main window corner radius
pub const MARGIN: f32 = 16.0; // Global margin
pub const TITLE_BAR_H: f32 = 32.0;
pub const WIN_BTN_W: f32 = 46.0; // Width of window control buttons (min/close)

// ----------------------------------------------------------------------------------
//  ELEMENT POSITIONS (Main Window)
// ----------------------------------------------------------------------------------
pub const TITLE_Y: f32 = 8.0;
pub const INPUT_Y: f32 = 47.0;
pub const INPUT_H: f32 = 32.0;
pub const BTN_Y: f32 = 96.0;
pub const BTN_H: f32 = 30.0;
pub const BTN_W: f32 = 80.0;

// ----------------------------------------------------------------------------------
//  DROPDOWN CONFIGURATION
// ----------------------------------------------------------------------------------
pub const ITEM_H: f32 = 26.0; // Height of each history item row
pub const DROPDOWN_MAX_ITEMS: usize = 5;
pub const DROPDOWN_GAP: f32 = 5.0;

// ----------------------------------------------------------------------------------
//  TOOLTIP CONFIGURATION
// ----------------------------------------------------------------------------------
pub const TOOLTIP_W: f32 = 400.0;
pub const TOOLTIP_H: f32 = 100.0;
pub const TOOLTIP_TRI_W: f32 = 8.0; // Triangle width
pub const TOOLTIP_TRI_H: f32 = 8.0; // Triangle height
pub const TOOLTIP_GAP: f32 = 4.0; // Gap between input and tooltip
pub const TOOLTIP_ICON_SIZE: f32 = 25.0;
pub const TOOLTIP_PADDING: f32 = 15.0;
pub const TOOLTIP_CORNER_RADIUS: f32 = 8.0;

// ----------------------------------------------------------------------------------
//  DIALOG WINDOW CONFIGURATION
// ----------------------------------------------------------------------------------
pub const DIALOG_W: i32 = 620;
pub const DIALOG_H: i32 = 190;
pub const DIALOG_PADDING: f32 = 22.0;
pub const DIALOG_BTN_W: f32 = 160.0;
pub const DIALOG_BTN_H: f32 = 36.0;

// ----------------------------------------------------------------------------------
//  TYPOGRAPHY & FONTS
// ----------------------------------------------------------------------------------
// Font Families
pub const FONT_DISPLAY: windows::core::PCWSTR = windows::core::w!("Segoe UI Variable Display");
pub const FONT_TEXT: windows::core::PCWSTR = windows::core::w!("Segoe UI Variable Text");
pub const FONT_SMALL: windows::core::PCWSTR = windows::core::w!("Segoe UI Variable Small");
pub const FONT_STD: windows::core::PCWSTR = windows::core::w!("Segoe UI");

// Font Sizes
pub const FONT_SZ_TITLE: f32 = 13.0;
pub const FONT_SZ_LABEL: f32 = 12.0;
pub const FONT_SZ_BUTTON: f32 = 12.0;
pub const FONT_SZ_TOOLTIP: f32 = 11.5;
pub const FONT_SZ_TOOLTIP_BOLD: f32 = 12.5;
pub const FONT_SZ_DIALOG_MSG: f32 = 18.0;
pub const FONT_SZ_DIALOG_BTN: f32 = 15.0;
pub const FONT_SZ_INPUT: i32 = 20;

// ----------------------------------------------------------------------------------
//  ANIMATIONS
// ----------------------------------------------------------------------------------
pub const ANIM_ENTER_DURATION_MS: u128 = 250;
pub const ANIM_EXIT_DURATION_MS: u128 = 150;
pub const ANIM_DROPDOWN_DURATION_MS: u128 = 200;
pub const ANIM_TOOLTIP_DURATION_MS: u128 = 300;
pub const ANIM_TIMER_MS: u32 = 1; // High-frequency timer for maximum smoothness (limited by monitor refresh)

// ----------------------------------------------------------------------------------
//  COLORS (THEME)
// ----------------------------------------------------------------------------------
// General
pub const COLOR_ACCENT_OPACITY: f32 = 0.9;
pub const COLOR_HOVER_BRIGHTEN: f32 = 0.15;
pub const COLOR_BORDER_OPACITY: f32 = 0.12;
pub const COLOR_DISABLED_OPACITY: f32 = 0.4;

// Dark Mode Palette
pub const COLOR_DARK_BG: f32 = 0.12;
pub const COLOR_DARK_BTN: f32 = 0.25;
pub const COLOR_DARK_BTN_HOVER: f32 = 0.35;
pub const COLOR_DARK_BORDER: f32 = 0.6;
pub const COLOR_DARK_TEXT: f32 = 1.0;
pub const COLOR_DARK_TEXT_SEC: f32 = 0.7;

// Light Mode Palette
pub const COLOR_LIGHT_BG: f32 = 0.98;
pub const COLOR_LIGHT_BTN: f32 = 0.95;
pub const COLOR_LIGHT_BTN_HOVER: f32 = 0.98;
pub const COLOR_LIGHT_BORDER: f32 = 0.4;
pub const COLOR_LIGHT_TEXT: f32 = 0.1;
pub const COLOR_LIGHT_TEXT_SEC: f32 = 0.4;

// Status Colors
pub const COLOR_WARNING_R: f32 = 0.9;
pub const COLOR_WARNING_G: f32 = 0.1;
pub const COLOR_WARNING_B: f32 = 0.1;

pub const COLOR_DESTRUCTIVE_R: f32 = 0.769;
pub const COLOR_DESTRUCTIVE_G: f32 = 0.169;
pub const COLOR_DESTRUCTIVE_B: f32 = 0.11;

pub const COLOR_INPUT_BG_DARK: f32 = 0.1;
pub const COLOR_INPUT_BG_LIGHT: f32 = 0.9;
pub const COLOR_INPUT_BG_OPACITY: f32 = 0.15;

pub const ACRYLIC_TINT_DARK: u32 = 0x00202020;
pub const ACRYLIC_TINT_LIGHT: u32 = 0x00F3F3F3;

pub const COLOR_ACCENT_DEFAULT_R: f32 = 0.0;
pub const COLOR_ACCENT_DEFAULT_G: f32 = 0.47;
pub const COLOR_ACCENT_DEFAULT_B: f32 = 0.83;

// ----------------------------------------------------------------------------------
//  MISC
// ----------------------------------------------------------------------------------
pub const EDIT_ID: u32 = 101;

// Static string caching (Performance)
pub static STR_TITLE: OnceLock<Vec<u16>> = OnceLock::new();
pub static STR_RUN: OnceLock<Vec<u16>> = OnceLock::new();
pub static STR_CANCEL: OnceLock<Vec<u16>> = OnceLock::new();

pub fn get_str_title() -> &'static [u16] {
    STR_TITLE
        .get_or_init(|| "Swift Run".encode_utf16().collect())
        .as_slice()
}
pub fn get_str_run() -> &'static [u16] {
    STR_RUN
        .get_or_init(|| "Run \u{21B5}".encode_utf16().collect())
        .as_slice()
}
pub fn get_str_cancel() -> &'static [u16] {
    STR_CANCEL
        .get_or_init(|| "Cancel".encode_utf16().collect())
        .as_slice()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoverId {
    None,
    Close,
    Min,
    Input,
    Ok,
    Cancel,
    Dropdown,
}
