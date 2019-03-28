use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;

use failure::ResultExt;
use structopt::StructOpt;

use crate::Result;

#[derive(Debug, StructOpt)]
pub struct Opts {
    /// The command to run
    #[structopt(name = "COMMAND", required = true, parse(from_os_str))]
    args: Vec<OsString>,
    /// The directory to run the command in
    #[structopt(name = "WORKDIR", long = "workdir", short = "w", parse(from_os_str))]
    workdir: Option<PathBuf>,
}

impl Opts {
    fn build(&self) -> Command {
        let mut cmd = Command::new(&self.args[0]);
        cmd.args(&self.args[1..]);
        if let Some(workdir) = &self.workdir {
            cmd.current_dir(workdir);
        }
        cmd
    }
}

pub fn run(opts: &Opts) -> Result<i32> {
    let status = opts.build().status()
        .with_context(|_| format!("failed to execute process '{}'", opts))?;
    if status.success() {
        Ok(0)
    } else {
        log::error!("Process '{}' exited unsuccessfully ({})", opts, status);
        Ok(status.code().unwrap_or(1))
    }
}

impl fmt::Display for Opts {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.args[0].to_string_lossy())?;
        for arg in &self.args[1..] {
            write!(f, " {}", arg.to_string_lossy())?;
        }
        Ok(())
    }
}
