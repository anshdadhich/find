use windows::{
    Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_WIN, VK_SPACE},
    Win32::Foundation::HWND,
};

pub const HOTKEY_ID: i32 = 1;

pub fn register(hwnd: HWND) -> bool {
    unsafe {
        RegisterHotKey(hwnd, HOTKEY_ID, MOD_WIN, VK_SPACE.0 as u32).is_ok()
    }
}