use std::borrow::Cow;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, prelude::*, BufReader};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;
use structopt::StructOpt;

use failure::ResultExt;
use futures::future::Either;
use futures::prelude::*;
use indicatif::HumanDuration;
use tokio::runtime::Runtime;
use tokio::timer::Interval;
use tokio_io::{try_nb, AsyncRead};
use tokio_process::CommandExt;

use crate::config::Config;
use crate::hash::hash;
use crate::logger;
use crate::output;
use crate::Result;

pub fn run(opts: &Opts, config: Config) -> Result<i32> {
    let command = CommandOptions::new(opts, config)?;
    log::trace!("command: {:#?}", command);

    let mut output = output::Writer::new(opts, &command)?;
    let progress_ticker = if let Some(len) = output.diff().len() {
        let msg = format!("{:#}", HumanDuration(len));
        logger::start_progress(len.as_millis() as u64, &msg);
        Some(
            Interval::new_interval(Duration::from_millis(200))
                .for_each(|_| Ok(logger::tick_progress_bar())),
        )
    } else {
        None
    };

    let mut rt = Runtime::new()?;
    let output = Arc::new(output);
    let (output1, output2) = (output.clone(), output.clone());
    let status_fut = command.spawn(
        map_err(move |line| output1.write_stdout(line)),
        map_err(move |line| output2.write_stderr(line)),
    )?;
    let status = if let Some(ticker) = progress_ticker {
        match rt.block_on(status_fut.select2(ticker)) {
            Ok(Either::A((status, _))) => status,
            Ok(Either::B(_)) => unreachable!(),
            Err(Either::A((err, _))) => return Err(err.into()),
            Err(Either::B((err, _))) => return Err(err.into()),
        }
    } else {
        rt.block_on(status_fut)?
    };

    output.finish(status.success())?;

    if !status.success() {
        log::error!("process '{}' exited unsuccessfully ({})", command, status);
    }
    log::info!(
        "output log file is located at '{}'",
        output.path().display()
    );

    Ok(status.code().unwrap_or(1))
}

#[derive(Debug, StructOpt)]
pub struct Opts {
    /// The command to run
    #[structopt(name = "COMMAND", required = true, parse(from_os_str))]
    pub args: Vec<OsString>,
    /// The file to pipe the command to, relative to workdir
    #[structopt(name = "OUTPUT", long = "output", short = "o", parse(from_os_str))]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandOptions<'a> {
    pub args: Cow<'a, [OsString]>,
    pub workdir: PathBuf,
    pub env: BTreeMap<String, OsString>,
}

impl<'a> CommandOptions<'a> {
    fn new(opts: &'a Opts, config: Config) -> Result<Self> {
        debug_assert!(!opts.args.is_empty());

        let env = config
            .env
            .into_iter()
            .filter_map(|key| env::var_os(&key).map(|val| (key, val)))
            .collect();

        Ok(CommandOptions {
            args: Cow::Borrowed(&opts.args),
            workdir: env::current_dir().context("failed to get current directory")?,
            env,
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
        Ok(Some(line).into())
    }
}

fn map_err<A, R>(mut f: impl FnMut(A) -> Result<R>) -> impl FnMut(A) -> io::Result<R> {
    move |a| f(a).map_err(|err| io::Error::new(io::ErrorKind::Other, err))
}
