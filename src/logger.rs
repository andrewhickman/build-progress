use std::fmt::Display;

use console::{style, Term};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use lazy_static::lazy_static;
use log::Log;
use structopt::StructOpt;

pub fn init(opts: Opts) {
    log::set_max_level(opts.level_filter());
    log::set_logger(&LOGGER as &Logger).unwrap();
}

pub fn log_bytes<B>(bytes: B)
where
    B: AsRef<[u8]>,
{
    if log::max_level() >= log::Level::Info {
        LOGGER.write_raw(String::from_utf8_lossy(bytes.as_ref()))
    }
}

pub fn start_progress(len: u64, msg: &str) {
    LOGGER
        .progress
        .set_draw_target(ProgressDrawTarget::to_term(LOGGER.term.clone(), None));
    LOGGER.progress.set_length(len);
    LOGGER.progress.set_message(msg);
}

pub fn set_progress_position(pos: u64) {
    LOGGER.progress.set_position(pos);
}

pub fn finish_progress() {
    LOGGER.progress.finish();
}

lazy_static! {
    static ref LOGGER: Logger = Logger::new();
}

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
    #[structopt(long, short, help = "Disable logging", global = true)]
    quiet: bool,
}

struct Logger {
    term: Term,
    progress: ProgressBar,
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
    fn new() -> Self {
        let progress = ProgressBar::hidden();
        progress.set_style(
            ProgressStyle::default_bar()
                .template("[{bar:64.white}] {elapsed}/{msg}")
                .progress_chars("=> "),
        );
        Logger {
            term: Term::stdout(),
            progress,
        }
    }

    fn write<D, S>(&self, prefix: D, msg: S)
    where
        D: Display,
        S: AsRef<str>,
    {
        const PAD: usize = 8;

        let mut lines = msg.as_ref().lines();
        if let Some(first) = lines.next() {
            self.write_raw(format!("{:>pad$.pad$}: {}", prefix, first, pad = PAD));
        }
        for line in lines {
            self.write_raw(format!("{:>pad$}  {}", "", line, pad = PAD));
        }
    }

    fn write_raw<S>(&self, msg: S) where S: Into<String> + AsRef<str> {
        if self.progress.is_hidden() {
            self.term.write_line(msg.as_ref()).ok();
        } else {
            self.progress.println(msg);
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
            }
            .bold();

            self.write(prefix, &record.args().to_string());
        }
    }

    fn flush(&self) {}
}
