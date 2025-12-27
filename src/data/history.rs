#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]
#![allow(non_snake_case)]

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Controls::EM_SETSEL;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

pub static mut HISTORY: Option<Vec<String>> = None;
pub static mut FILTERED_HISTORY: Option<Vec<String>> = None;
pub static mut PREDICTION: String = String::new();
pub static mut HISTORY_INDEX: isize = -1;
pub static mut IS_CYCLING: bool = false;

pub fn get_history_path() -> Option<PathBuf> {
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

pub fn load_history() {
    unsafe {
        HISTORY = Some(Vec::new());
        FILTERED_HISTORY = None;
        PREDICTION = String::new();
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

pub fn save_history(cmd: &str) {
    unsafe {
        if let Some(history) = HISTORY.as_mut() {
            if let Some(pos) = history.iter().position(|x| x == cmd) {
                history.remove(pos);
            }
            history.insert(0, cmd.to_string());
            if history.len() > 50 {
                history.truncate(50);
            }

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

pub unsafe fn cycle_history(delta: isize, h_edit: HWND) {
    let history_to_use = if let Some(filtered) = FILTERED_HISTORY.as_ref() {
        filtered
    } else if let Some(history) = HISTORY.as_ref() {
        history
    } else {
        return;
    };

    let history_len = history_to_use.len() as isize;
    if history_len == 0 {
        return;
    }

    let new_index = HISTORY_INDEX + delta;

    if new_index < -1 {
        HISTORY_INDEX = -1;
    } else if new_index >= history_len {
        HISTORY_INDEX = history_len - 1;
    } else {
        HISTORY_INDEX = new_index;
    }

    let text_to_set = if HISTORY_INDEX == -1 {
        String::new()
    } else {
        let real_index = HISTORY_INDEX;
        if real_index >= 0 && real_index < history_len {
            history_to_use[real_index as usize].clone()
        } else {
            String::new()
        }
    };

    IS_CYCLING = true;
    PREDICTION = String::new();
    let text_u16: Vec<u16> = text_to_set
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let _ = SetWindowTextW(h_edit, PCWSTR(text_u16.as_ptr()));

    // Also select all
    SendMessageW(h_edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
    IS_CYCLING = false;
}
