use std::{fs, path::PathBuf};

use super::{Settings, codec};

fn path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join("Reel").join("settings.ini"))
}

pub fn load() -> Settings {
    path()
        .and_then(|path| fs::read_to_string(path).ok())
        .map_or_else(Settings::default, |text| codec::parse(&text))
}

pub fn save(settings: &Settings) {
    if let Some(path) = path() {
        let _ = path.parent().map(fs::create_dir_all);
        let _ = fs::write(path, codec::serialize(settings));
    }
}
