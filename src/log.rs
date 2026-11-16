use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
};

use crate::paths;

const MAX_BYTES: u64 = 1_000_000;

pub fn directory() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join("Reel"))
}

pub fn open() -> Option<File> {
    let directory = directory()?;
    fs::create_dir_all(&directory).ok()?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("reel.log"))
        .ok()?;
    match file.metadata().map(|meta| meta.len() > MAX_BYTES) {
        Ok(true) => file.set_len(0).ok().map(|_| file),
        _ => Some(file),
    }
}

pub fn line(message: &str) {
    let stamp = paths::local_stamp();
    if let Some(mut file) = open() {
        let _ = writeln!(
            file,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {message}",
            stamp.year, stamp.month, stamp.day, stamp.hour, stamp.minute, stamp.second
        );
    }
}
