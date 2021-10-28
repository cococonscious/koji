use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::Path};

use crate::commit_types::CommitType;

const CONFIG_FILE: &str = "koji.toml";

#[derive(Clone, Deserialize)]
pub struct Config {
    pub commit_types: Vec<CommitType>,
}

/// Loads `koji.toml` if there is one, returning a `Config`.
pub fn load_config() -> Result<Option<Config>> {
    if Path::new(CONFIG_FILE).exists() {
        let file = fs::read_to_string(CONFIG_FILE).context("reading config file")?;
        let parsed: Config = toml::from_str(file.as_ref()).context("parsing config file")?;

        Ok(Some(parsed))
    } else {
        Ok(None)
    }
}
