use std::ffi::OsString;
use std::fmt;
use std::process::Command;

use failure::ResultExt;
use structopt::StructOpt;

use crate::Result;

#[derive(Debug, StructOpt)]
pub struct Opts {
    #[structopt(name = "COMMAND", required = true, parse(from_os_str))]
    args: Vec<OsString>,
}

pub fn run(opts: &Opts) -> Result<i32> {
    let status = Command::new(&opts.args[0])
        .args(&opts.args[1..])
        .status()
        .with_context(|_| format!("failed to execute process '{}'", opts))?;
    if status.success() {
        Ok(0)
    } else {
        log::error!("Process exited unsuccessfully: '{}' ({})", opts, status);
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
