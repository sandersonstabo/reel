use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExePath(String);

impl ExePath {
    pub fn new(raw: &str) -> Self {
        Self(raw.trim().replace('/', "\\").to_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn stem(&self) -> &str {
        let file = self.0.rsplit('\\').next().unwrap_or(&self.0);
        file.strip_suffix(".exe").unwrap_or(file)
    }
}

pub type KnownGames = HashSet<ExePath>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub exe: ExePath,
    pub covers_monitor: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    KnownToWindows,
    FullscreenInLibrary,
    NotGame,
}

impl Verdict {
    pub fn is_game(self) -> bool {
        !matches!(self, Verdict::NotGame)
    }
}

const LIBRARIES: [&str; 9] = [
    "\\steamapps\\common\\",
    "\\epic games\\",
    "\\riot games\\",
    "\\gog galaxy\\games\\",
    "\\gog games\\",
    "\\xboxgames\\",
    "\\ea games\\",
    "\\ubisoft game launcher\\games\\",
    "\\battlestate games\\",
];

pub fn in_library(exe: &ExePath) -> bool {
    LIBRARIES.iter().any(|library| exe.as_str().contains(library))
}

pub fn classify(candidate: &Candidate, known: &KnownGames) -> Verdict {
    match (
        known.contains(&candidate.exe),
        candidate.covers_monitor && in_library(&candidate.exe),
    ) {
        (true, _) => Verdict::KnownToWindows,
        (false, true) => Verdict::FullscreenInLibrary,
        (false, false) => Verdict::NotGame,
    }
}
