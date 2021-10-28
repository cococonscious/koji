use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::Path};

use crate::commit_types::CommitType;

const CONFIG_FILE: &str = "koji.toml";

#[derive(Clone, Deserialize)]
pub struct Config {
    pub commit_types: Vec<CommitType>,
}

/// Loads `koji.toml` if there is one, otherwise use the default.
pub fn load_config() -> Result<Config> {
    if Path::new(CONFIG_FILE).exists() {
        let file = fs::read_to_string(CONFIG_FILE).context("could not read config file")?;
        let parsed: Config =
            toml::from_str(file.as_ref()).context("could not parse config file")?;

        Ok(parsed)
    } else {
        let file = include_str!("../../meta/config/koji-default.toml");
        let parsed: Config = toml::from_str(file).context("could not parse default config file")?;

        Ok(parsed)
    }
}
