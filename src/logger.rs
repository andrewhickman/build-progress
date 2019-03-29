use std::fmt::Display;

use console::style;
use log::Log;
use structopt::StructOpt;

use encoding::all::UTF_8;
use encoding::{decode, DecoderTrap};

pub fn init(opts: Opts) {
    log::set_max_level(opts.level_filter());
    log::set_logger(&Logger).unwrap();
}

pub fn log_proc_stderr(line: &[u8]) {
    // Logger.write(style("stderr").magenta().bold(), decode_utf8(&line));
}

pub fn log_proc_stdout(line: &[u8]) {
    Logger.write(style("stdout").magenta().bold(), decode_utf8(&line));
}

struct Logger;

#[derive(Copy, Clone, Debug, StructOpt)]
pub struct Opts {
    #[structopt(
        long,
        help = "Enables debug logging",
        conflicts_with = "quiet",
        global = true
    )]
    debug: bool,
    #[structopt(
        long,
        help = "Enables trace logging",
        conflicts_with = "quiet",
        global = true
    )]
    trace: bool,
    #[structopt(long, short, help = "Disable all logging to stderr", global = true)]
    quiet: bool,
}

impl Opts {
    fn level_filter(self) -> log::LevelFilter {
        if self.quiet {
            log::LevelFilter::Off
        } else if self.trace {
            log::LevelFilter::Trace
        } else if self.debug {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        }
    }
}

impl Logger {
    fn write<D>(&self, prefix: D, msg: impl AsRef<str>) 
    where
        D: Display
    {
        const PAD: usize = 8;

        let mut lines = msg.as_ref().lines();
        if let Some(first) = lines.next() {
            eprint!("{:>pad$.pad$}: ", prefix, pad = PAD);
            eprintln!("{}", first);
        }
        for line in lines {
            eprintln!("{:>pad$}  {}", "", line, pad = PAD);
        }
    }
}

impl Log for Logger {
    fn enabled(&self, meta: &log::Metadata) -> bool {
        meta.target() == "bp" || meta.target().starts_with("bp::")
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(&record.metadata()) {
            let prefix = match record.level() {
                log::Level::Trace => style("trace").white(),
                log::Level::Debug => style("debug").cyan(),
                log::Level::Info => style("info").blue(),
                log::Level::Warn => style("warning").yellow(),
                log::Level::Error => style("error").red(),
            }.bold();

            self.write(prefix, &record.args().to_string());
        }
    }

    fn flush(&self) {}
}

fn decode_utf8(bytes: &[u8]) -> String {
    // decoding cannot fail since we use `DecoderTrap::Replace`.
    decode(&bytes, DecoderTrap::Replace, UTF_8).0.unwrap()
}