use reel::session::{Action, Cue, Event, Game, Mode, step};

fn game(pid: u32) -> Game {
    Game {
        pid,
        window: 42,
        name: "VALORANT".to_string(),
    }
}

fn watching() -> Mode {
    Mode::Watching { skip: None }
}

#[test]
fn focusing_a_game_while_watching_starts_recording() {
    assert_eq!(
        step(watching(), Event::GameFocused(game(7))),
        (Mode::Recording(game(7)), vec![Action::Start(game(7))])
    );
}

#[test]
fn closing_the_game_stops_and_keeps_watching() {
    assert_eq!(
        step(Mode::Recording(game(7)), Event::GameClosed),
        (watching(), vec![Action::Stop])
    );
}

#[test]
fn toggle_while_recording_stops_and_turns_off() {
    assert_eq!(
        step(Mode::Recording(game(7)), Event::Toggle),
        (Mode::Off, vec![Action::Stop, Action::Cue(Cue::Off)])
    );
}

#[test]
fn toggle_cycles_between_off_and_watching() {
    assert_eq!(step(Mode::Off, Event::Toggle), (watching(), vec![Action::Cue(Cue::On)]));
    assert_eq!(step(watching(), Event::Toggle), (Mode::Off, vec![Action::Cue(Cue::Off)]));
}

#[test]
fn off_ignores_games() {
    assert_eq!(step(Mode::Off, Event::GameFocused(game(7))), (Mode::Off, vec![]));
}

#[test]
fn failed_recorder_is_not_retried_for_the_same_process() {
    let (mode, actions) = step(Mode::Recording(game(7)), Event::RecorderFailed);
    assert_eq!(actions, vec![Action::Stop]);
    assert_eq!(step(mode.clone(), Event::GameFocused(game(7))), (mode, vec![]));
}

#[test]
fn failed_recorder_still_records_a_different_process() {
    let (mode, _) = step(Mode::Recording(game(7)), Event::RecorderFailed);
    assert_eq!(
        step(mode, Event::GameFocused(game(8))),
        (Mode::Recording(game(8)), vec![Action::Start(game(8))])
    );
}

#[test]
fn toggling_clears_the_failed_process() {
    let (mode, _) = step(Mode::Recording(game(7)), Event::RecorderFailed);
    let (mode, _) = step(mode, Event::Toggle);
    let (mode, _) = step(mode, Event::Toggle);
    assert_eq!(mode, watching());
}

#[test]
fn second_game_does_not_interrupt_a_recording() {
    assert_eq!(
        step(Mode::Recording(game(7)), Event::GameFocused(game(8))),
        (Mode::Recording(game(7)), vec![])
    );
}

#[test]
fn saving_while_buffering_saves_the_current_game() {
    assert_eq!(
        step(Mode::Recording(game(7)), Event::SaveClip),
        (Mode::Recording(game(7)), vec![Action::Save(game(7)), Action::Cue(Cue::Saved)])
    );
}

#[test]
fn saving_without_a_game_only_signals_unavailable() {
    assert_eq!(step(watching(), Event::SaveClip), (watching(), vec![Action::Cue(Cue::Unavailable)]));
    assert_eq!(step(Mode::Off, Event::SaveClip), (Mode::Off, vec![Action::Cue(Cue::Unavailable)]));
}

#[test]
fn settings_change_restarts_the_buffer() {
    assert_eq!(
        step(Mode::Recording(game(7)), Event::SettingsChanged),
        (watching(), vec![Action::Stop])
    );
    assert_eq!(step(watching(), Event::SettingsChanged), (watching(), vec![]));
}
