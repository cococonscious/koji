use anyhow::{Context, Error, Result};
use serde_derive::Deserialize;
use std::fs;

use crate::commit_types::CommitType;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub commit_types: Vec<CommitType>,
}

pub fn get_config() -> Result<Config, Error> {
    let file = fs::read_to_string("koji.toml").context("reading config file")?;
    let parsed: Config = toml::from_str(file.as_ref()).context("parsing config file")?;

    Ok(parsed)
}
