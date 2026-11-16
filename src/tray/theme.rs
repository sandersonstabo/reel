use windows::{
    Win32::{
        Foundation::HWND,
        System::LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
    },
    core::{PCSTR, w},
};

const ALLOW_DARK_MODE_FOR_WINDOW: usize = 133;
const SET_PREFERRED_APP_MODE: usize = 135;
const FLUSH_MENU_THEMES: usize = 136;
const FOLLOW_SYSTEM: i32 = 1;

type AllowDarkModeForWindow = unsafe extern "system" fn(HWND, bool) -> bool;
type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
type FlushMenuThemes = unsafe extern "system" fn();

fn export(ordinal: usize) -> Option<unsafe extern "system" fn() -> isize> {
    let library = unsafe { LoadLibraryExW(w!("uxtheme.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.ok()?;
    unsafe { GetProcAddress(library, PCSTR(ordinal as *const u8)) }
}

pub fn refresh() {
    if let Some(flush) = export(FLUSH_MENU_THEMES) {
        let flush: FlushMenuThemes = unsafe { std::mem::transmute(flush) };
        unsafe { flush() };
    }
}

pub fn follow_system(window: HWND) {
    if let Some(set_mode) = export(SET_PREFERRED_APP_MODE) {
        let set_mode: SetPreferredAppMode = unsafe { std::mem::transmute(set_mode) };
        unsafe { set_mode(FOLLOW_SYSTEM) };
    }
    if let Some(allow) = export(ALLOW_DARK_MODE_FOR_WINDOW) {
        let allow: AllowDarkModeForWindow = unsafe { std::mem::transmute(allow) };
        unsafe { allow(window, true) };
    }
    refresh();
}
