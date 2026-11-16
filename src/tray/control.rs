use std::{ffi::OsString, os::windows::ffi::OsStringExt};

use windows::{
    Win32::{
        Foundation::{CloseHandle, LPARAM, WPARAM},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{GetCurrentProcessId, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
        },
        UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_CLOSE},
    },
    core::w,
};

use super::autostart;

const EXIT_WAIT_MS: u32 = 10_000;

fn name(entry: &PROCESSENTRY32W) -> String {
    let length = entry.szExeFile.iter().position(|unit| *unit == 0).unwrap_or(entry.szExeFile.len());
    OsString::from_wide(&entry.szExeFile[..length]).to_string_lossy().to_lowercase()
}

fn siblings() -> Vec<u32> {
    let own = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|file| file.to_string_lossy().to_lowercase()))
        .unwrap_or_else(|| "reel.exe".to_string());
    let me = unsafe { GetCurrentProcessId() };
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = Vec::new();
    let mut more = unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok();
    while more {
        if entry.th32ProcessID != me && name(&entry) == own {
            found.push(entry.th32ProcessID);
        }
        more = unsafe { Process32NextW(snapshot, &mut entry) }.is_ok();
    }
    let _ = unsafe { CloseHandle(snapshot) };
    found
}

fn wait_for(pid: u32) {
    if let Ok(process) = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
        unsafe {
            WaitForSingleObject(process, EXIT_WAIT_MS);
            let _ = CloseHandle(process);
        }
    }
}

pub fn quit() -> i32 {
    if let Ok(window) = unsafe { FindWindowW(w!("Reel.Tray"), None) } {
        let _ = unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    }
    siblings().into_iter().for_each(wait_for);
    0
}

pub fn cleanup() -> i32 {
    autostart::set(false);
    0
}
