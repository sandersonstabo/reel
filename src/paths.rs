use std::path::PathBuf;

use windows::Win32::{
    System::{Com::CoTaskMemFree, SystemInformation::GetLocalTime},
    UI::Shell::{FOLDERID_Videos, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
};

use crate::naming::{self, Stamp};

fn videos() -> Option<PathBuf> {
    let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_Videos, KF_FLAG_DEFAULT, None) }.ok()?;
    let path = unsafe { raw.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(raw.0 as *const _)) };
    path.map(PathBuf::from)
}

pub fn recordings() -> PathBuf {
    videos()
        .or_else(|| std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join("Videos")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Reel")
}

pub fn local_stamp() -> Stamp {
    let time = unsafe { GetLocalTime() };
    Stamp {
        year: time.wYear,
        month: time.wMonth,
        day: time.wDay,
        hour: time.wHour,
        minute: time.wMinute,
        second: time.wSecond,
    }
}

pub fn output_for(name: &str) -> PathBuf {
    recordings()
        .join(name)
        .join(naming::file_name(name, local_stamp()))
}
