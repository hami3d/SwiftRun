use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimType {
    None,
    Entering,
    Exiting,
}

pub static mut ANIM_START_TIME: Option<Instant> = None;
pub static mut ANIM_TYPE: AnimType = AnimType::None;
pub static mut EXIT_KILL_PROCESS: bool = false;
pub static mut FINAL_X: i32 = 0;
pub static mut FINAL_Y: i32 = 0;
pub static mut START_Y: i32 = 0;

pub static mut DROPDOWN_ANIM_START: Option<Instant> = None;
pub static mut DROPDOWN_ANIM_TYPE: AnimType = AnimType::None;

pub static mut TOOLTIP_ANIM_START: Option<Instant> = None;
pub static mut TOOLTIP_ANIM_TYPE: AnimType = AnimType::None;

pub fn ease_out_quad(t: f32) -> f32 {
    t * (2.0 - t)
}

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

pub fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}
