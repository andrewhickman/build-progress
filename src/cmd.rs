use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, prelude::*, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use failure::{bail, ResultExt};
use futures::{Future, Poll, Stream};
use indicatif::HumanDuration;
use structopt::StructOpt;
use tokio_io::{try_nb, AsyncRead};
use tokio_process::CommandExt;

use crate::config::Config;
use crate::diff;
use crate::hash::hash;
use crate::logger;
use crate::Result;

pub fn run(opts: &Opts, config: Config) -> Result<i32> {
    let command = CommandOptions::new(opts, config)?;
    log::trace!("Command: {:#?}", command);

    let dir = if let Some(dir) = dirs::data_dir() {
        dir.join(env!("CARGO_PKG_NAME")).join(command.hash())
    } else {
        bail!("failed to get user's data directory");
    };

    fs::create_dir_all(&dir)
        .with_context(|_| format!("failed to create directory '{}'", dir.display()))?;

    let mut writer = diff::Writer::new(&dir)?;
    if let Some(len) = writer.len() {
        let msg = format!("{:#}", HumanDuration(len));
        logger::start_progress(len.as_millis() as u64, &msg);
    }

    let output_path = if let Some(output) = &opts.output {
        command.workdir.join(output)
    } else {
        dir.join("output").with_extension("log")
    };
    let output = File::create(&output_path)
        .with_context(|_| format!("failed to create file '{}'", output_path.display()))?;

    let handle_stdout = map_err(|line: Vec<u8>| {
        write_output(&line, &output, &output_path)?;
        logger::log_bytes(&line);
        writer.write_line(line)?;
        logger::set_progress_position(writer.completed().as_millis() as u64);
        Ok(())
    });

    let status = command.spawn(handle_stdout)?.wait()?;

    logger::finish_progress();
    writer.finish()?;

    if !status.success() {
        log::error!("process '{}' exited unsuccessfully ({})", command, status);
    }
    log::info!("Output log file is located at '{}'", output_path.display());

    Ok(status.code().unwrap_or(1))
}

#[derive(Debug, StructOpt)]
pub struct Opts {
    /// The command to run
    #[structopt(name = "COMMAND", required = true, parse(from_os_str))]
    args: Vec<OsString>,
    /// The file to pipe the command to, relative to workdir
    #[structopt(name = "OUTPUT", long = "output", short = "o", parse(from_os_str))]
    output: Option<PathBuf>,
}

#[derive(Debug, Hash)]
pub struct CommandOptions<'a> {
    args: &'a [OsString],
    workdir: PathBuf,
    env: BTreeMap<String, OsString>,
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
            args: &opts.args,
            workdir: env::current_dir().context("failed to get current directory")?,
            env,
        })
    }

    pub fn hash(&self) -> String {
        hash(self)
    }

    fn spawn<O>(
        &self,
        out: O,
    ) -> Result<impl Future<Item = ExitStatus, Error = io::Error>>
    where
        O: FnMut(Vec<u8>) -> io::Result<()>,
    {
        let mut child = Command::new(&self.args[0])
            .args(&self.args[1..])
            .stderr(Stdio::inherit())
            .stdout(Stdio::piped())
            .spawn_async()
            .with_context(|_| format!("failed to execute process '{}'", self))?;
        let stdout = lines(child.stdout().take().unwrap()).for_each(out);
        Ok(child.join(stdout).map(|(status, ())| status))
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

fn write_output(line: &[u8], mut file: &File, path: &Path) -> Result<()> {
    Ok(file
        .write_all(line)
        .with_context(|_| format!("failed to write to file '{}'", path.display()))?)
}

fn map_err<A, R>(mut f: impl FnMut(A) -> Result<R>) -> impl FnMut(A) -> io::Result<R> {
    move |a| f(a).map_err(|err| io::Error::new(io::ErrorKind::Other, err))
}
