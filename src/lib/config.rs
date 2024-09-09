use anyhow::{Context, Result};
use dirs::config_dir;
use indexmap::IndexMap;
use serde::Deserialize;
use std::{env::current_dir, fs::read_to_string, path::Path};

pub struct Config {
    pub autocomplete: bool,
    pub breaking_changes: bool,
    pub commit_types: IndexMap<String, CommitType>,
    pub emoji: bool,
    pub issues: bool,
    pub sign: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct CommitType {
    pub description: String,
    pub emoji: Option<String>,
    pub name: String,
}

#[derive(Clone, Deserialize)]
struct ConfigTOML {
    pub autocomplete: Option<bool>,
    pub breaking_changes: Option<bool>,
    #[serde(default)]
    commit_types: Vec<CommitType>,
    pub emoji: Option<bool>,
    pub issues: Option<bool>,
    pub sign: Option<bool>,
}

#[derive(Default)]
pub struct ConfigArgs {
    pub path: Option<String>,
    pub autocomplete: Option<bool>,
    pub breaking_changes: Option<bool>,
    pub emoji: Option<bool>,
    pub issues: Option<bool>,
    pub sign: Option<bool>,
}

impl Config {
    /// Find a config and load it
    pub fn new(args: Option<ConfigArgs>) -> Result<Self> {
        let ConfigArgs {
            path,
            autocomplete,
            breaking_changes,
            emoji,
            issues,
            sign,
        } = args.unwrap_or_default();

        // Get the default config
        let default_str = include_str!("../../meta/config/default.toml");
        let default_config: ConfigTOML =
            toml::from_str(default_str).context("could not parse config file")?;

        let mut parsed: Option<ConfigTOML> = None;

        // Try to get config from users config directory
        let config_dir_path = config_dir().unwrap().join("koji/config.toml");
        if Path::new(&config_dir_path).exists() {
            let contents = read_to_string(config_dir_path).context("could not read config")?;
            parsed = Some(toml::from_str(&contents).context("could not parse config")?);
        };

        // Try to get config from working directory
        let working_dir_path = current_dir()?.join(".koji.toml");
        if Path::new(&working_dir_path).exists() {
            let contents = read_to_string(working_dir_path).context("could not read config")?;
            parsed = Some(toml::from_str(&contents).context("could not parse config")?);
        };

        // Try to get config from passed directory
        if let Some(path) = path {
            if Path::new(&path).exists() {
                let contents = read_to_string(&path).context("could not read config")?;
                parsed = Some(toml::from_str(&contents).context("could not parse config")?);
            }
        }

        // If the users' config doesn't have any commit types,
        // merge in the defaults
        let config = match parsed {
            Some(mut config) => {
                if config.commit_types.is_empty() {
                    config.commit_types = default_config.commit_types;
                }

                config
            }
            None => default_config,
        };

        // Gather up commit types
        let mut commit_types = IndexMap::new();
        for commit_type in config.commit_types.iter() {
            commit_types.insert(commit_type.name.to_owned(), commit_type.to_owned());
        }

        Ok(Config {
            autocomplete: autocomplete.unwrap_or(config.autocomplete.unwrap_or(false)),
            breaking_changes: breaking_changes.unwrap_or(config.breaking_changes.unwrap_or(true)),
            commit_types,
            emoji: emoji.unwrap_or(config.emoji.unwrap_or(false)),
            issues: issues.unwrap_or(config.issues.unwrap_or(true)),
            sign: sign.unwrap_or(config.sign.unwrap_or(false)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breaking_changes() {
        let config = Config::new(None).unwrap();
        assert!(config.breaking_changes);

        let config = Config::new(Some(ConfigArgs {
            breaking_changes: Some(false),
            ..Default::default()
        }))
        .unwrap();
        assert!(!config.breaking_changes);
    }

    #[test]
    fn test_issues() {
        let config = Config::new(None).unwrap();
        assert!(config.issues);

        let config = Config::new(Some(ConfigArgs {
            issues: Some(false),
            ..Default::default()
        }))
        .unwrap();
        assert!(!config.issues);
    }

    #[test]
    fn test_commit_types() {
        let config = Config::new(None).unwrap();
        let commit_types = config.commit_types;

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
