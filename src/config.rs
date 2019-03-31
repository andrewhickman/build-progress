use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::{fs, io};

use failure::{Fail, ResultExt};
use serde::Deserialize;
use structopt::StructOpt;

use crate::Result;

#[derive(Debug, StructOpt)]
pub struct Opts {
    /// The location of the config file
    #[structopt(
        name = "CONFIG",
        long = "config",
        short = "c",
        raw(env_os = r#"OsStr::new("BUILD_PROGRESS_CONFIG_PATH")"#),
        hide_env_values = true,
        parse(from_os_str)
    )]
    config: Option<PathBuf>,
}

#[derive(Deserialize, Default, Debug)]
pub struct Config {
    pub env: Vec<String>,
}

pub fn read(opts: &Opts) -> Result<Config> {
    let (config_path, required) = if let Some(path) = &opts.config {
        (Cow::Borrowed(&**path), true)
    } else if let Some(dir) = dirs::config_dir() {
        let path = dir
            .join(env!("CARGO_PKG_NAME"))
            .join("config")
            .with_extension("toml");
        (Cow::Owned(path), false)
    } else {
        log::debug!("Unable to resolve config path");
        return Ok(Config::default());
    };

    let config_file = match fs::read_to_string(&config_path) {
        Ok(file) => file,
        Err(ref err) if !required && err.kind() == io::ErrorKind::NotFound => {
            log::debug!(
                "Failed to open config file '{}': {}",
                config_path.display(),
                err
            );
            return Ok(Config::default());
        }
        Err(err) => {
            return Err(err
                .context(format!(
                    "failed to open config file '{}'",
                    config_path.display()
                ))
                .into());
        }
    };

    log::debug!("Reading config from file '{}'", config_path.display());
    let config = toml::from_str(&config_file)
        .with_context(|_| format!("failed to read TOML file '{}'", config_path.display()))?;
    Ok(config)
}
