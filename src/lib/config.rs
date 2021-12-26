use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::Path};

use crate::commit_types::CommitType;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub commit_types: Vec<CommitType>,
}

/// Loads the given config file if one is provided,
/// otherwise use the default.
pub fn load_config(path: Option<String>) -> Result<Config> {
    let path = path.unwrap_or_else(|| "koji.toml".into());

    let parsed = if Path::new(&path).exists() {
        let file = fs::read_to_string(path).context("could not read config file")?;
        toml::from_str(file.as_ref()).context("could not parse config file")?
    } else {
        let file = include_str!("../../meta/config/koji-default.toml");
        toml::from_str(file).context("could not parse default config file")?
    };

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config_default() {
        let config = load_config(None).unwrap();
        let first = config.commit_types.get(0).unwrap();

        assert_eq!(first.description, "A new feature");
    }

    #[test]
    fn test_load_config_with_arg() {
        let config = load_config(Some("./meta/config/koji-no-emoji.toml".into())).unwrap();
        let first = config.commit_types.get(0).unwrap();

        assert_eq!(first.emoji, None);
    }
}
