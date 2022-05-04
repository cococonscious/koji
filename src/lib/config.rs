use anyhow::{Context, Result};
use dirs::config_dir;
use indexmap::IndexMap;
use serde::Deserialize;
use std::{env::current_dir, fs::read_to_string, path::Path};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CommitType {
    pub name: String,
    pub emoji: Option<String>,
    pub description: String,
}

#[derive(Clone, Deserialize)]
pub struct Config {
    pub autocomplete: Option<bool>,
    pub emoji: Option<bool>,
    #[serde(default)]
    commit_types: Vec<CommitType>,
}

impl Config {
    /// Find a config and load it
    pub fn new(path: Option<String>) -> Result<Self> {
        // Get the default config
        let default_str = include_str!("../../meta/config/default.toml");
        let default_config: Config =
            toml::from_str(default_str).context("could not parse config file")?;

        let mut config: Option<Self> = None;

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

        Ok(match config {
            Some(mut config) => {
                if config.commit_types.is_empty() {
                    config.commit_types = default_config.commit_types;
                }

                config
            }
            None => default_config,
        })
    }

    pub fn commit_types(&self) -> IndexMap<String, CommitType> {
        let mut map = IndexMap::new();

        for commit_type in self.commit_types.iter() {
            map.insert(commit_type.name.to_owned(), commit_type.to_owned());
        }

        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        let config = Config::new(None).unwrap();
        let first = config.commit_types.get(0).unwrap();

        assert_eq!(first.description, "A new feature");
    }

    #[test]
    fn test_commit_types() {
        let config = Config::new(None).unwrap();
        let commit_types = config.commit_types();

        assert_eq!(
            commit_types.get("feat"),
            Some(&CommitType {
                name: "feat".into(),
                emoji: Some("✨".into()),
                description: "A new feature".into()
            })
        )
    }
}
