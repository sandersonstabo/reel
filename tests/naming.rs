use reel::naming::{Stamp, display_name, file_name, sanitize};

#[test]
fn prefers_trimmed_window_title() {
    assert_eq!(display_name("VALORANT  ", "valorant-win64-shipping"), "VALORANT");
}

#[test]
fn falls_back_to_exe_stem_without_a_usable_title() {
    assert_eq!(display_name(" -- ", "bf6"), "bf6");
    assert_eq!(display_name("", ""), "Game");
}

#[test]
fn strips_characters_windows_forbids_in_paths() {
    assert_eq!(sanitize("Half-Life: Alyx <VR>?"), "Half-Life Alyx VR");
}

#[test]
fn strips_trailing_dots_and_caps_length() {
    assert_eq!(sanitize("Game..."), "Game");
    assert_eq!(sanitize(&"x".repeat(200)).len(), 60);
}

#[test]
fn file_name_is_sortable() {
    let stamp = Stamp {
        year: 2026,
        month: 11,
        day: 6,
        hour: 9,
        minute: 4,
        second: 5,
    };
    assert_eq!(file_name("VALORANT", stamp), "VALORANT 2026-11-06 09-04-05.mp4");
}
