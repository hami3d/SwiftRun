use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub unsafe fn register_hotkeys(hwnd: HWND) {
    // Win+R (ID 1)
    let _ = RegisterHotKey(hwnd, 1, MOD_WIN | MOD_NOREPEAT, VK_R.0 as u32);
}

pub unsafe fn unregister_hotkeys(hwnd: HWND) {
    let _ = UnregisterHotKey(hwnd, 1);
}
