use anyhow::{Context, Result};
use serde_derive::Deserialize;
use std::{fs, path::Path};

use crate::commit_types::CommitType;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub commit_types: Vec<CommitType>,
}

pub fn load_config() -> Result<Option<Config>> {
    if Path::new("koji.toml").exists() {
        let file = fs::read_to_string("koji.toml").context("reading config file")?;
        let parsed: Config = toml::from_str(file.as_ref()).context("parsing config file")?;

        Ok(Some(parsed))
    } else {
        Ok(None)
    }
}
