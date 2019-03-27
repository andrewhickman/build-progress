mod cmd;
mod logger;

use std::process;

use structopt::StructOpt;

type Error = failure::Error;
type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, StructOpt)]
#[structopt(raw(setting = "structopt::clap::AppSettings::TrailingVarArg"))]
struct Opts {
    #[structopt(flatten)]
    logger: logger::Opts,
    #[structopt(flatten)]
    cmd: cmd::Opts,
}

fn main() {
    let opts = Opts::from_args();
    logger::init(opts.logger).unwrap();

    log::trace!("Options: {:#?}", opts);

    let code = match cmd::run(&opts.cmd) {
        Ok(code) => code,
        Err(err) => {
            log::error!("Error: {}", fmt_error(&err));
            3
        }
    };

    process::exit(code)
}

fn fmt_error(err: &Error) -> String {
    let mut pretty = err.to_string();
    for cause in err.iter_causes() {
        pretty.push_str(&format!("\ncaused by: {}", cause));
    }
    pretty
}
