use std::sync::OnceLock;

// Fixed window dimensions
pub const WIN_W: f32 = 500.0;
pub const WIN_H: f32 = 180.0;
pub const ITEM_H: f32 = 18.0; // Height of each history item

// UI Layout
pub const MARGIN: f32 = 16.0;
pub const CORNER_RADIUS: f32 = 5.0;
pub const TITLE_BAR_H: f32 = 32.0;
pub const WIN_BTN_W: f32 = 46.0;

// Element positions
pub const TITLE_Y: f32 = 8.0;
pub const INPUT_Y: f32 = 47.0;
pub const INPUT_H: f32 = 32.0;
pub const BTN_Y: f32 = 96.0;
pub const BTN_H: f32 = 30.0;
pub const BTN_W: f32 = 80.0;

pub const EDIT_ID: u32 = 101;

// Animation Durations
pub const ANIM_ENTER_DURATION_MS: u128 = 350;
pub const ANIM_EXIT_DURATION_MS: u128 = 150;
pub const ANIM_DROPDOWN_DURATION_MS: u128 = 200;
pub const ANIM_TOOLTIP_DURATION_MS: u128 = 300;

// Static strings caching
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
