use std::ffi::c_void;

use windows::{
    Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW,
        RegGetValueW, RegOpenKeyExW,
    },
    core::{PCWSTR, PWSTR, w},
};

use super::classify::{ExePath, KnownGames};

const MAX_KEY: usize = 256;

fn subkey(key: HKEY, index: u32) -> Option<Vec<u16>> {
    let mut name = [0u16; MAX_KEY];
    let mut length = MAX_KEY as u32;
    let result = unsafe {
        RegEnumKeyExW(
            key,
            index,
            Some(PWSTR(name.as_mut_ptr())),
            &mut length,
            None,
            None,
            None,
            None,
        )
    };
    result.ok().ok()?;
    Some(name[..length as usize].iter().copied().chain([0]).collect())
}

fn matched_path(key: HKEY, subkey: &[u16]) -> Option<String> {
    let mut size = 0u32;
    let value = w!("MatchedExeFullPath");
    unsafe {
        RegGetValueW(key, PCWSTR(subkey.as_ptr()), value, RRF_RT_REG_SZ, None, None, Some(&mut size))
    }
    .ok()
    .ok()?;
    let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
    unsafe {
        RegGetValueW(
            key,
            PCWSTR(subkey.as_ptr()),
            value,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    let text = String::from_utf16_lossy(&buffer);
    let trimmed = text.trim_end_matches('\0');
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub fn load() -> KnownGames {
    let mut key = HKEY::default();
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("System\\GameConfigStore\\Children"),
            Some(0),
            KEY_READ,
            &mut key,
        )
    };
    match opened.ok() {
        Err(_) => KnownGames::new(),
        Ok(()) => {
            let games = (0..)
                .map_while(|index| subkey(key, index))
                .filter_map(|name| matched_path(key, &name))
                .map(|path| ExePath::new(&path))
                .collect();
            let _ = unsafe { RegCloseKey(key) };
            games
        }
    }
}
