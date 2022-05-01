use anyhow::{Context, Result};
use dirs::config_dir;
use serde::Deserialize;
use std::{env::current_dir, fs::read_to_string, path::Path};

use crate::commit_types::CommitType;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub autocomplete: Option<bool>,
    pub emoji: Option<bool>,
    #[serde(default)]
    pub commit_types: Vec<CommitType>,
}

/// Find a config and load it.
pub fn load_config(path: Option<String>) -> Result<Config> {
    // Get the default config
    let default_str = include_str!("../../meta/config/default.toml");
    let default_config: Config =
        toml::from_str(default_str).context("could not parse config file")?;

    let mut config: Option<Config> = None;

    // Try to get config from users config directory
    let config_dir_path = config_dir().unwrap().join("koji/config.toml");
    if Path::new(&config_dir_path).exists() {
        let contents = read_to_string(config_dir_path).context("could not read config")?;
        config = Some(toml::from_str(&contents).context("could not parse config")?);
    };

    // Try to get config from working directory
    let working_dir_path = current_dir()?.join(".koji.toml");
    if Path::new(&working_dir_path).exists() {
        let contents = read_to_string(working_dir_path).context("could not read config")?;
        config = Some(toml::from_str(&contents).context("could not parse config")?);
    };

    // Try to get config from passed directory
    if let Some(path) = path {
        if Path::new(&path).exists() {
            let contents = read_to_string(&path).context("could not read config")?;
            config = Some(toml::from_str(&contents).context("could not parse config")?);
        }
    }

    let config = match config {
        Some(mut config) => {
            if config.commit_types.is_empty() {
                config.commit_types = default_config.commit_types;
            }

            config
        }
        None => default_config,
    };

    Ok(config)
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
}
