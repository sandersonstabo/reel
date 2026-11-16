use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, POINT, WPARAM},
        UI::WindowsAndMessaging::{
            AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HMENU, MENU_ITEM_FLAGS,
            MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, PostMessageW,
            SetForegroundWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
            WM_NULL,
        },
    },
    core::HSTRING,
};

use crate::settings::{ClipKey, ClipLength, Codec, FrameRate, ResolutionCap, SizeTarget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Toggle,
    SaveClip,
    Length(ClipLength),
    Target(SizeTarget),
    Resolution(ResolutionCap),
    FrameRate(FrameRate),
    Codec(Codec),
    Key(ClipKey),
    Autostart,
    OpenClips,
    Quit,
}

pub enum Item {
    Note(String),
    Separator,
    Entry {
        label: String,
        checked: bool,
        enabled: bool,
        choice: Choice,
    },
    Submenu {
        label: String,
        items: Vec<Item>,
    },
}

pub fn entry(label: impl Into<String>, checked: bool, choice: Choice) -> Item {
    Item::Entry {
        label: label.into(),
        checked,
        enabled: true,
        choice,
    }
}

fn flags(checked: bool, enabled: bool) -> MENU_ITEM_FLAGS {
    let check = match checked {
        true => MF_CHECKED,
        false => MENU_ITEM_FLAGS(0),
    };
    let grey = match enabled {
        true => MENU_ITEM_FLAGS(0),
        false => MF_GRAYED,
    };
    MF_STRING | check | grey
}

fn append(menu: HMENU, flags: MENU_ITEM_FLAGS, id: usize, label: &str) {
    let _ = unsafe { AppendMenuW(menu, flags, id, &HSTRING::from(label)) };
}

fn render(menu: HMENU, items: &[Item], choices: &mut Vec<Choice>) {
    items.iter().for_each(|item| match item {
        Item::Note(label) => append(menu, MF_STRING | MF_GRAYED, 0, label),
        Item::Separator => append(menu, MF_SEPARATOR, 0, ""),
        Item::Entry {
            label,
            checked,
            enabled,
            choice,
        } => {
            choices.push(*choice);
            append(menu, flags(*checked, *enabled), choices.len(), label);
        }
        Item::Submenu { label, items } => {
            if let Ok(submenu) = unsafe { CreatePopupMenu() } {
                render(submenu, items, choices);
                append(menu, MF_STRING | MF_POPUP, submenu.0 as usize, label);
            }
        }
    });
}

pub fn show(window: HWND, items: &[Item]) -> Option<Choice> {
    let menu = unsafe { CreatePopupMenu() }.ok()?;
    let mut choices = Vec::new();
    render(menu, items, &mut choices);
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
    (picked.0 as usize).checked_sub(1).and_then(|index| choices.get(index).copied())
}
