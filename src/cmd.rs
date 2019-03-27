use std::ffi::OsString;

use structopt::StructOpt;

use crate::Result;

#[derive(Debug, StructOpt)]
pub struct Opts {
    #[structopt(name = "COMMAND", required = true, parse(from_os_str))]
    args: Vec<OsString>,
}

pub fn run(opts: &Opts) -> Result<i32> {
    unimplemented!()
}