use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, POINT, WPARAM},
        UI::WindowsAndMessaging::{
            AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_CHECKED, MF_GRAYED,
            MF_SEPARATOR, MF_STRING, MF_UNCHECKED, PostMessageW, SetForegroundWindow,
            TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
        },
    },
    core::HSTRING,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Toggle,
    OpenRecordings,
    Autostart,
    Quit,
}

impl Choice {
    const ALL: [Choice; 4] = [Choice::Toggle, Choice::OpenRecordings, Choice::Autostart, Choice::Quit];

    fn id(self) -> usize {
        self as usize + 1
    }

    fn from_id(id: usize) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.id() == id)
    }
}

pub struct Snapshot {
    pub status: String,
    pub enabled: bool,
    pub autostart: bool,
}

fn checked(on: bool) -> windows::Win32::UI::WindowsAndMessaging::MENU_ITEM_FLAGS {
    match on {
        true => MF_CHECKED,
        false => MF_UNCHECKED,
    }
}

pub fn show(window: HWND, snapshot: &Snapshot) -> Option<Choice> {
    let menu = unsafe { CreatePopupMenu() }.ok()?;
    let items = [
        (MF_STRING | MF_GRAYED, 0, snapshot.status.clone()),
        (MF_SEPARATOR, 0, String::new()),
        (MF_STRING | checked(snapshot.enabled), Choice::Toggle.id(), "Record games\tF8".to_string()),
        (MF_STRING, Choice::OpenRecordings.id(), "Open recordings".to_string()),
        (MF_STRING | checked(snapshot.autostart), Choice::Autostart.id(), "Start with Windows".to_string()),
        (MF_SEPARATOR, 0, String::new()),
        (MF_STRING, Choice::Quit.id(), "Quit".to_string()),
    ];
    items.iter().for_each(|(flags, id, label)| {
        let _ = unsafe { AppendMenuW(menu, *flags, *id, &HSTRING::from(label.as_str())) };
    });
    let mut cursor = POINT::default();
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let _ = unsafe { SetForegroundWindow(window) };
    let picked = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            None,
            window,
            None,
        )
    };
    let _ = unsafe { PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0)) };
    let _ = unsafe { DestroyMenu(menu) };
    Choice::from_id(picked.0 as usize)
}
