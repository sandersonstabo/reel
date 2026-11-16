use reel::detect::classify::{Candidate, ExePath, KnownGames, Verdict, classify};

const VALORANT: &str = r"C:\Riot Games\VALORANT\live\ShooterGame\Binaries\Win64\VALORANT-Win64-Shipping.exe";

fn candidate(exe: &str, covers_monitor: bool) -> Candidate {
    Candidate {
        exe: ExePath::new(exe),
        covers_monitor,
    }
}

fn known(paths: &[&str]) -> KnownGames {
    paths.iter().map(|path| ExePath::new(path)).collect()
}

#[test]
fn normalizes_case_and_separators() {
    assert_eq!(ExePath::new(" C:/Games/Foo.EXE "), ExePath::new(r"c:\games\foo.exe"));
}

#[test]
fn stem_drops_directory_and_extension() {
    assert_eq!(ExePath::new(VALORANT).stem(), "valorant-win64-shipping");
}

#[test]
fn game_known_to_windows_counts_even_when_windowed() {
    let verdict = classify(&candidate(VALORANT, false), &known(&[&VALORANT.to_uppercase()]));
    assert_eq!(verdict, Verdict::KnownToWindows);
}

#[test]
fn fullscreen_exe_in_a_launcher_library_counts() {
    let exe = r"D:\SteamLibrary\steamapps\common\Indie Game\indie.exe";
    assert_eq!(classify(&candidate(exe, true), &known(&[])), Verdict::FullscreenInLibrary);
}

#[test]
fn windowed_exe_in_a_launcher_library_is_ignored() {
    let exe = r"D:\SteamLibrary\steamapps\common\Indie Game\indie.exe";
    assert_eq!(classify(&candidate(exe, false), &known(&[])), Verdict::NotGame);
}

#[test]
fn fullscreen_browser_is_ignored() {
    let exe = r"C:\Program Files\Helium\helium.exe";
    assert_eq!(classify(&candidate(exe, true), &known(&[])), Verdict::NotGame);
}
