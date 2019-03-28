use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use failure::ResultExt;
use structopt::StructOpt;

use crate::hash::hash;
use crate::Result;

#[derive(Debug, StructOpt)]
pub struct Opts {
    /// The command to run
    #[structopt(name = "COMMAND", required = true, parse(from_os_str))]
    args: Vec<OsString>,
    /// The directory to run the command in
    #[structopt(
        name = "WORKDIR",
        long = "workdir",
        short = "w",
        default_value = ".",
        hide_default_value = true,
        parse(from_os_str),
    )]
    workdir: PathBuf,
}

#[derive(Debug, Hash)]
pub struct CommandOptions<'a> {
    args: &'a [OsString],
    workdir: PathBuf,
}

impl<'a> CommandOptions<'a> {
    fn new(opts: &'a Opts) -> Result<Self> {
        debug_assert!(!opts.args.is_empty());
        Ok(CommandOptions {
            args: &opts.args,
            workdir: opts.workdir.canonicalize().with_context(|_| {
                format!("failed to canonicalize path '{}'", opts.workdir.display())
            })?,
        })
    }

    pub fn hash(&self) -> String {
        hash(self)
    }

    fn status(&self) -> Result<ExitStatus> {
        Ok(Command::new(&self.args[0])
            .args(&self.args[1..])
            .current_dir(&self.workdir)
            .status()
            .with_context(|_| format!("failed to execute process '{}'", self))?)
    }
}

pub fn run(opts: &Opts) -> Result<i32> {
    let command = CommandOptions::new(opts)?;
    log::trace!("Command: {:#?}", command);
    let status = command.status()?;
    if status.success() {
        Ok(0)
    } else {
        log::error!("Process '{}' exited unsuccessfully ({})", command, status);
        Ok(status.code().unwrap_or(1))
    }
}

impl<'a> fmt::Display for CommandOptions<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.args[0].to_string_lossy())?;
        for arg in &self.args[1..] {
            write!(f, " {}", arg.to_string_lossy())?;
        }
        Ok(())
    }
}
