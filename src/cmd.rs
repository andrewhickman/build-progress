use std::ffi::OsString;
use std::fmt;
use std::io::{self, prelude::*, BufReader};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

use failure::ResultExt;
use futures::{Future, Poll, Stream};
use structopt::StructOpt;
use indicatif::{ProgressBar, ProgressDrawTarget};
use tokio_io::{try_nb, AsyncRead};
use tokio_process::CommandExt;

use crate::diff;
use crate::hash::hash;
use crate::Result;

pub fn run<E>(opts: &Opts, err: E) -> Result<i32>
where
    E: FnMut(Vec<u8>) -> Result<()>,
{
    let command = CommandOptions::new(opts)?;
    log::trace!("Command: {:#?}", command);

    let mut writer = diff::Writer::new(command.hash())?;
    let pb = writer.len().map(|len| ProgressBar::with_draw_target(len, ProgressDrawTarget::stdout()));
    let status = command
        .spawn(map_err(|line| {
            writer.write_line(line)?;
            if let Some(pb) = &pb {
                pb.set_position(writer.completed());
            }
            Ok(())
        }), map_err(err))?
        .wait()?;
    if let Some(pb) = &pb {
        pb.finish();
    }
    writer.finish()?;

    if status.success() {
        Ok(0)
    } else {
        log::error!("Process '{}' exited unsuccessfully ({})", command, status);
        Ok(status.code().unwrap_or(1))
    }
}

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
        parse(from_os_str)
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

    fn spawn<O, E>(
        &self,
        out: O,
        err: E,
    ) -> Result<impl Future<Item = ExitStatus, Error = io::Error>>
    where
        O: FnMut(Vec<u8>) -> io::Result<()>,
        E: FnMut(Vec<u8>) -> io::Result<()>,
    {
        let mut child = Command::new(&self.args[0])
            .args(&self.args[1..])
            .current_dir(&self.workdir)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn_async()
            .with_context(|_| format!("failed to execute process '{}'", self))?;
        let stdout = lines(child.stdout().take().unwrap()).for_each(out);
        let stderr = lines(child.stderr().take().unwrap()).for_each(err);
        Ok(child.join3(stdout, stderr).map(|(status, (), ())| status))
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

struct Lines<R> {
    rdr: R,
}

fn lines<R>(rdr: R) -> Lines<BufReader<R>>
where
    R: AsyncRead,
{
    Lines {
        rdr: BufReader::new(rdr),
    }
}

impl<R> Stream for Lines<R>
where
    R: AsyncRead + BufRead,
{
    type Item = Vec<u8>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Vec<u8>>, io::Error> {
        let mut line = Vec::new();
        let n = try_nb!(self.rdr.read_until(b'\n', &mut line));
        if n == 0 && line.len() == 0 {
            return Ok(None.into());
        }
        if line.ends_with(b"\n") {
            line.pop();
            if line.ends_with(b"\r") {
                line.pop();
            }
        }
        Ok(Some(line).into())
    }
}

fn map_err<A, R>(mut f: impl FnMut(A) -> Result<R>) -> impl FnMut(A) -> io::Result<R> {
    move |a| f(a).map_err(|err| io::Error::new(io::ErrorKind::Other, err))
}
