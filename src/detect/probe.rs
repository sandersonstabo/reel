use std::ffi::c_void;

use windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, RECT},
        Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
        System::Threading::{
            GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
            IsIconic, IsWindow,
        },
    },
    core::PWSTR,
};

use super::classify::{Candidate, ExePath};
use crate::{naming, session::Game};

pub struct Foreground {
    pub pid: u32,
    pub window: HWND,
    pub exe: ExePath,
    pub title: String,
    pub covers_monitor: bool,
}

impl Foreground {
    pub fn candidate(&self) -> Candidate {
        Candidate {
            exe: self.exe.clone(),
            covers_monitor: self.covers_monitor,
        }
    }

    pub fn game(&self) -> Game {
        Game {
            pid: self.pid,
            window: self.window.0 as isize,
            name: naming::display_name(&self.title, self.exe.stem()),
        }
    }
}

fn owner(window: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    pid
}

fn exe_path(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buffer = [0u16; 1024];
    let mut length = buffer.len() as u32;
    let queried = unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut length)
    };
    let _ = unsafe { CloseHandle(process) };
    queried.ok()?;
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

fn title(window: HWND) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetWindowTextW(window, &mut buffer) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

fn covers_monitor(window: HWND) -> bool {
    let mut rect = RECT::default();
    let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let measured = unsafe { GetWindowRect(window, &mut rect) }.is_ok()
        && unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool();
    let screen = info.rcMonitor;
    measured
        && rect.left <= screen.left
        && rect.top <= screen.top
        && rect.right >= screen.right
        && rect.bottom >= screen.bottom
}

pub fn foreground() -> Option<Foreground> {
    let window = unsafe { GetForegroundWindow() };
    let pid = owner(window);
    let usable = !window.is_invalid()
        && pid != 0
        && pid != unsafe { GetCurrentProcessId() }
        && !unsafe { IsIconic(window) }.as_bool();
    match usable {
        false => None,
        true => Some(Foreground {
            pid,
            window,
            exe: ExePath::new(&exe_path(pid)?),
            title: title(window),
            covers_monitor: covers_monitor(window),
        }),
    }
}

pub fn alive(game: &Game) -> bool {
    let window = HWND(game.window as *mut c_void);
    unsafe { IsWindow(Some(window)) }.as_bool() && owner(window) == game.pid
}
