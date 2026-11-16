#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub pid: u32,
    pub window: isize,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    Watching { skip: Option<u32> },
    Recording(Game),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Toggle,
    GameFocused(Game),
    GameClosed,
    RecorderFailed,
    SaveClip,
    SettingsChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    On,
    Off,
    Saved,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Start(Game),
    Stop,
    Save(Game),
    Cue(Cue),
}

pub fn step(mode: Mode, event: Event) -> (Mode, Vec<Action>) {
    match (mode, event) {
        (Mode::Off, Event::Toggle) => (Mode::Watching { skip: None }, vec![Action::Cue(Cue::On)]),
        (Mode::Off, Event::SaveClip) => (Mode::Off, vec![Action::Cue(Cue::Unavailable)]),
        (Mode::Off, _) => (Mode::Off, vec![]),
        (Mode::Watching { .. }, Event::Toggle) => (Mode::Off, vec![Action::Cue(Cue::Off)]),
        (Mode::Watching { skip }, Event::GameFocused(game)) if skip != Some(game.pid) => {
            (Mode::Recording(game.clone()), vec![Action::Start(game)])
        }
        (watching @ Mode::Watching { .. }, Event::SaveClip) => {
            (watching, vec![Action::Cue(Cue::Unavailable)])
        }
        (watching @ Mode::Watching { .. }, _) => (watching, vec![]),
        (Mode::Recording(_), Event::Toggle) => {
            (Mode::Off, vec![Action::Stop, Action::Cue(Cue::Off)])
        }
        (Mode::Recording(_), Event::GameClosed) => {
            (Mode::Watching { skip: None }, vec![Action::Stop])
        }
        (Mode::Recording(game), Event::RecorderFailed) => (
            Mode::Watching {
                skip: Some(game.pid),
            },
            vec![Action::Stop],
        ),
        (Mode::Recording(_), Event::SettingsChanged) => {
            (Mode::Watching { skip: None }, vec![Action::Stop])
        }
        (Mode::Recording(game), Event::SaveClip) => (
            Mode::Recording(game.clone()),
            vec![Action::Save(game), Action::Cue(Cue::Saved)],
        ),
        (recording @ Mode::Recording(_), Event::GameFocused(_)) => (recording, vec![]),
    }
}
