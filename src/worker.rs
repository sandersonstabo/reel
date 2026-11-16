use std::{
    env,
    io::{self, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
};

use crate::{log, session::Game};

pub struct Worker {
    child: Child,
    commands: Option<ChildStdin>,
}

impl Worker {
    pub fn spawn(game: &Game) -> io::Result<Self> {
        let stderr = log::open().map(Stdio::from).unwrap_or_else(Stdio::null);
        let mut child = Command::new(env::current_exe()?)
            .arg("record")
            .arg(game.window.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()?;
        let commands = child.stdin.take();
        Ok(Self { child, commands })
    }

    pub fn save(&mut self, path: &Path) -> io::Result<()> {
        match self.commands.as_mut() {
            Some(commands) => {
                writeln!(commands, "save {}", path.display())?;
                commands.flush()
            }
            None => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }

    pub fn exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    pub fn stop(mut self) -> Self {
        drop(self.commands.take());
        self
    }
}
