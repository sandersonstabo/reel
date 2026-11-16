use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use crate::{log, session::Game};

pub struct Worker {
    child: Child,
    output: PathBuf,
}

impl Worker {
    pub fn spawn(game: &Game, output: &Path) -> io::Result<Self> {
        if let Some(directory) = output.parent() {
            fs::create_dir_all(directory)?;
        }
        let stderr = log::open().map(Stdio::from).unwrap_or_else(Stdio::null);
        let child = Command::new(env::current_exe()?)
            .arg("record")
            .arg(game.window.to_string())
            .arg(output)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()?;
        Ok(Self {
            child,
            output: output.to_path_buf(),
        })
    }

    pub fn output(&self) -> &Path {
        &self.output
    }

    pub fn exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    pub fn stop(mut self) -> Self {
        drop(self.child.stdin.take());
        self
    }
}
