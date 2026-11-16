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

fn command() -> Option<String> {
    let mut size = 0u32;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN, NAME, RRF_RT_REG_SZ, None, None, Some(&mut size)) }
        .ok()
        .ok()?;
    let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN,
            NAME,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    Some(String::from_utf16_lossy(&buffer).trim_end_matches('\0').to_string())
}

fn expected() -> Option<String> {
    env::current_exe().ok().map(|exe| format!("\"{}\"", exe.display()))
}

pub fn enabled() -> bool {
    command().is_some()
}

pub fn follow_current_exe() {
    match (command(), expected()) {
        (Some(current), Some(wanted)) if !current.eq_ignore_ascii_case(&wanted) => set(true),
        _ => {}
    }
}

pub fn set(on: bool) {
    match (on, expected()) {
        (true, Some(wanted)) => {
            let command = HSTRING::from(wanted);
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
        (true, None) => {}
        (false, _) => {
            let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, NAME) };
        }
    }
}
