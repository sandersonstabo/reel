use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, S_OK, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::{
                TASKDIALOG_NOTIFICATIONS, TASKDIALOGCONFIG, TASKDIALOGCONFIG_0, TDCBF_OK_BUTTON,
                TDF_ALLOW_DIALOG_CANCELLATION, TDN_CREATED, TaskDialogIndirect,
            },
            WindowsAndMessaging::SetForegroundWindow,
        },
    },
    core::{HRESULT, HSTRING, PCWSTR, w},
};

use crate::settings::Settings;

const APP_ICON: u16 = 1;

unsafe extern "system" fn raise(window: HWND, notification: TASKDIALOG_NOTIFICATIONS, _: WPARAM, _: LPARAM, _: isize) -> HRESULT {
    if notification == TDN_CREATED {
        let _ = unsafe { SetForegroundWindow(window) };
    }
    S_OK
}

pub fn installed(settings: &Settings) {
    let content = HSTRING::from(format!(
        "Reel is installed and running in your tray. You configure it from there.\n\nIn a game, press {} to save the last {}. F8 turns clipping on or off.",
        settings.clip_key.label(),
        settings.clip_length.label()
    ));
    let instance = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
    let config = TASKDIALOGCONFIG {
        cbSize: size_of::<TASKDIALOGCONFIG>() as u32,
        hInstance: instance.into(),
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION,
        dwCommonButtons: TDCBF_OK_BUTTON,
        pszWindowTitle: w!("Reel"),
        Anonymous1: TASKDIALOGCONFIG_0 {
            pszMainIcon: PCWSTR(APP_ICON as usize as *const u16),
        },
        pszMainInstruction: w!("Reel is installed"),
        pszContent: PCWSTR(content.as_ptr()),
        pfCallback: Some(raise),
        ..Default::default()
    };
    let _ = unsafe { TaskDialogIndirect(&config, None, None, None) };
}
