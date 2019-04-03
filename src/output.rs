use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use failure::{bail, ResultExt};

use crate::cmd::{self, CommandOptions};
use crate::{diff, logger, Result};

pub struct Writer {
    file: File,
    path: PathBuf,
    diff: Mutex<diff::Writer>,
}

impl Writer {
    pub fn new(opts: &cmd::Opts, cmd: &CommandOptions) -> Result<Self> {
        let dir = if let Some(dir) = dirs::data_dir() {
            dir.join(env!("CARGO_PKG_NAME")).join(cmd.hash())
        } else {
            bail!("failed to get user's data directory");
        };

        fs::create_dir_all(&dir)
            .with_context(|_| format!("failed to create directory '{}'", dir.display()))?;
        let path = if let Some(path) = &opts.output {
            cmd.workdir.join(path)
        } else {
            dir.join("output").with_extension("log")
        };
        let file = File::create(&path)
            .with_context(|_| format!("failed to create file '{}'", path.display()))?;
        let diff = Mutex::new(diff::Writer::new(&dir)?);

        Ok(Writer { file, path, diff })
    }

    pub fn diff(&mut self) -> &mut diff::Writer {
        self.diff.get_mut().unwrap()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write_stdout(&self, line: Vec<u8>) -> Result<()> {
        self.write(&line)?;
        logger::log_bytes(&line);

        let mut diff = self.diff.lock().unwrap();
        diff.write_line(line)?;
        logger::set_progress_position(diff.completed().as_millis() as u64);
        Ok(())
    }

    pub fn write_stderr(&self, line: Vec<u8>) -> Result<()> {
        self.write(&line)?;
        logger::log_bytes(&line);
        Ok(())
    }

    fn write(&self, line: &[u8]) -> Result<()> {
        Ok((&self.file)
            .write_all(line)
            .with_context(|_| format!("failed to write to file '{}'", self.path.display()))?)
    }

    pub fn finish(&self, success: bool) -> Result<()> {
        logger::finish_progress();
        self.diff.lock().unwrap().finish(success)
    }
}
