use console::style;
use log::Log;
use structopt::StructOpt;

use encoding::all::UTF_8;
use encoding::{decode, DecoderTrap};

use crate::Result;

pub fn init(opts: Opts) {
    log::set_max_level(opts.level_filter());
    log::set_logger(&Logger).unwrap();
}

pub fn log_line(line: Vec<u8>) -> Result<()> {
    let message = decode(&line, DecoderTrap::Replace, UTF_8).0.unwrap();
    log::info!("{}", message);
    Ok(())
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
    fn write(&self, lvl: log::Level, msg: impl AsRef<str>) {
        const PAD: usize = 8;

        let prefix = match lvl {
            log::Level::Trace => style("trace").white(),
            log::Level::Debug => style("debug").cyan(),
            log::Level::Info => style("info").magenta(),
            log::Level::Warn => style("warning").yellow(),
            log::Level::Error => style("error").red(),
        };

        let mut lines = msg.as_ref().lines();
        if let Some(first) = lines.next() {
            eprint!("{:>pad$}: ", prefix, pad = PAD);
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
            self.write(record.level(), &record.args().to_string());
        }
    }

    fn flush(&self) {}
}
