use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub unsafe fn register_hotkeys(hwnd: HWND) -> bool {
    // Win+R (ID 1)
    let success = RegisterHotKey(hwnd, 1, MOD_WIN | MOD_NOREPEAT, VK_R.0 as u32).as_bool();
    if !success {
        eprintln!("Failed to register Win+R hotkey! It might be in use by another application.");
    }
    success
}

pub unsafe fn unregister_hotkeys(hwnd: HWND) {
    let _ = UnregisterHotKey(hwnd, 1);
}
