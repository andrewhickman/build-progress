mod cmd;
mod config;
mod diff;
mod hash;
mod logger;

use std::process;

use structopt::StructOpt;

type Error = failure::Error;
type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, StructOpt)]
#[structopt(usage = "bp.exe [OPTIONS] <COMMAND>...")]
#[structopt(raw(setting = "structopt::clap::AppSettings::TrailingVarArg"))]
#[structopt(raw(setting = "structopt::clap::AppSettings::UnifiedHelpMessage"))]
#[structopt(raw(setting = "structopt::clap::AppSettings::DisableVersion"))]
struct Opts {
    #[structopt(flatten)]
    logger: logger::Opts,
    #[structopt(flatten)]
    config: config::Opts,
    #[structopt(flatten)]
    cmd: cmd::Opts,
}

fn main() {
    process::exit(match run() {
        Ok(code) => code,
        Err(err) => {
            log::error!("{}", fmt_error(&err));
            17
        }
    })
}

fn run() -> Result<i32> {
    let opts = Opts::from_args();
    logger::init(opts.logger);
    log::trace!("Options: {:#?}", opts);

    let config = config::read(&opts.config)?;
    log::trace!("Config: {:#?}", config);

    cmd::run(&opts.cmd, config)
}

fn fmt_error(err: &Error) -> String {
    let mut pretty = err.to_string();
    for cause in err.iter_causes() {
        pretty.push_str(&format!("\ncaused by: {}", cause));
    }
    pretty
}
