use std::{env, ffi::c_void};

use windows::{
    Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW,
        RegSetKeyValueW,
    },
    core::{HSTRING, w},
};

const RUN: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const NAME: windows::core::PCWSTR = w!("Reel");

pub fn enabled() -> bool {
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN, NAME, RRF_RT_REG_SZ, None, None, None) }
        .is_ok()
}

pub fn set(on: bool) {
    match (on, env::current_exe()) {
        (true, Ok(exe)) => {
            let command = HSTRING::from(format!("\"{}\"", exe.display()));
            let bytes = (command.len() + 1) * 2;
            let _ = unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    RUN,
                    NAME,
                    REG_SZ.0,
                    Some(command.as_ptr() as *const c_void),
                    bytes as u32,
                )
            };
        }
        (true, Err(_)) => {}
        (false, _) => {
            let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, NAME) };
        }
    }
}
