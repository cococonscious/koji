use anyhow::Result;
use config::FileFormat;
use dirs::config_dir;
use indexmap::IndexMap;
use serde::Deserialize;
use std::env::current_dir;
use std::path::PathBuf;
use xdg::BaseDirectories;

#[derive(Debug, Clone)]
pub struct Config {
    pub autocomplete: bool,
    pub breaking_changes: bool,
    pub commit_types: IndexMap<String, CommitType>,
    pub emoji: bool,
    pub issues: bool,
    pub sign: bool,
    pub workdir: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct CommitType {
    pub description: String,
    pub emoji: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ConfigTOML {
    pub autocomplete: bool,
    pub breaking_changes: bool,
    #[serde(default)]
    commit_types: Vec<CommitType>,
    pub emoji: bool,
    pub issues: bool,
    pub sign: bool,
}

#[derive(Default)]
pub struct ConfigArgs {
    pub path: Option<PathBuf>,
    pub autocomplete: Option<bool>,
    pub breaking_changes: Option<bool>,
    pub emoji: Option<bool>,
    pub issues: Option<bool>,
    pub sign: Option<bool>,
    pub _user_config_path: Option<PathBuf>,
    pub _current_dir: Option<PathBuf>,
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
            _user_config_path,
            _current_dir,
        } = args.unwrap_or_default();

        let mut settings = config::Config::builder();

        let workdir = _current_dir.unwrap_or(current_dir()?);

        // Get the default config
        let default_str = include_str!("../../meta/config/default.toml");
        settings = settings.add_source(config::File::from_str(default_str, FileFormat::Toml));

        // Try to get config from xdg based config directory
        let xdg_dirs = BaseDirectories::with_prefix("koji");
        let xdg_config_path = xdg_dirs.get_config_file("config.toml").unwrap();
        settings = settings.add_source(config::File::from(xdg_config_path).required(false));

        // Try to get config from users config directory
        let config_dir_path = _user_config_path
            .unwrap_or(config_dir().unwrap())
            .join("koji/config.toml");
        settings = settings.add_source(config::File::from(config_dir_path).required(false));

        // Try to get config from working directory
        let working_dir_path = workdir.join(".koji.toml");
        settings = settings.add_source(config::File::from(working_dir_path).required(false));

        // Try to get config from passed directory
        if let Some(path) = path {
            settings = settings.add_source(config::File::from(path).required(false));
        }

        let config: ConfigTOML = settings.build()?.try_deserialize()?;

        // Gather up commit types
        let mut commit_types = IndexMap::new();
        for commit_type in config.commit_types.iter() {
            commit_types.insert(commit_type.name.clone(), commit_type.to_owned());
        }

        Ok(Config {
            autocomplete: autocomplete.unwrap_or(config.autocomplete),
            breaking_changes: breaking_changes.unwrap_or(config.breaking_changes),
            commit_types,
            emoji: emoji.unwrap_or(config.emoji),
            issues: issues.unwrap_or(config.issues),
            sign: sign.unwrap_or(config.sign),
            workdir,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_from_path() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        std::fs::write(
            tempdir.path().join("my-koji.toml"),
            "[[commit_types]]\nname=\"1234\"\ndescription=\"test\"",
        )?;

        let config = Config::new(Some(ConfigArgs {
            path: Some(tempdir.path().join("my-koji.toml")),
            ..ConfigArgs::default()
        }));

        assert!(config.is_ok());
        assert!(config?.commit_types.get("1234").is_some());

        tempdir.close()?;

        Ok(())
    }

    #[test]
    fn test_local_config() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        std::fs::write(
            tempdir.path().join(".koji.toml"),
            "[[commit_types]]\nname=\"123\"\ndescription=\"test\"",
        )?;

        let config = Config::new(Some(ConfigArgs {
            _current_dir: Some(tempdir.path().to_path_buf()),
            ..Default::default()
        }));

        assert!(config.is_ok());
        assert!(config?.commit_types.get("123").is_some());

        tempdir.close()?;

        Ok(())
    }

    #[test]
    fn test_user_config_config() -> Result<(), Box<dyn Error>> {
        let tempdir_current = tempfile::tempdir()?;
        let tempdir_config = tempfile::tempdir()?;
        std::fs::create_dir(tempdir_config.path().join("koji"))?;
        std::fs::write(
            tempdir_config.path().join("koji").join("config.toml"),
            "[[commit_types]]\nname=\"12345\"\ndescription=\"test\"",
        )?;

        let config = Config::new(Some(ConfigArgs {
            _user_config_path: Some(tempdir_config.path().to_path_buf()),
            _current_dir: Some(tempdir_current.path().to_path_buf()),
            ..Default::default()
        }));

        assert!(config.is_ok());
        assert!(config?.commit_types.get("12345").is_some());

        tempdir_current.close()?;
        tempdir_config.close()?;

        Ok(())
    }

    #[test]
    fn test_all_config_sources() -> Result<(), Box<dyn Error>> {
        let tempdir_config = tempfile::tempdir()?;
        std::fs::create_dir(tempdir_config.path().join("koji"))?;
        std::fs::write(
            tempdir_config.path().join("koji").join("config.toml"),
            "[[commit_types]]\nname=\"12345\"\ndescription=\"test\"",
        )?;
        let tempdir_current = tempfile::tempdir()?;
        std::fs::write(tempdir_current.path().join(".koji.toml"), "emoji=\"true\"")?;
        let tempdir_path = tempfile::tempdir()?;
        std::fs::write(tempdir_path.path().join("custom.toml"), "autocomplete=true")?;

        let config = Config::new(Some(ConfigArgs {
            _user_config_path: Some(tempdir_config.path().to_path_buf()),
            _current_dir: Some(tempdir_current.path().to_path_buf()),
            path: Some(tempdir_path.path().join("custom.toml").to_path_buf()),
            emoji: Some(false),
            ..Default::default()
        }))?;

        // from user config dir
        assert!(config.commit_types.get("12345").is_some());
        assert!(config.commit_types.len() == 1);
        // set by current dir config and directly, which overwrites the former
        assert!(!config.emoji);
        // set by passed config path
        assert!(config.autocomplete);
        // a default
        assert!(!config.sign);

        tempdir_current.close()?;
        tempdir_config.close()?;
        tempdir_path.close()?;

        Ok(())
    }

    #[test]
    fn test_non_custom_use_defaults() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;

        let config = Config::new(Some(ConfigArgs {
            _current_dir: Some(tempdir.path().to_path_buf()),
            _user_config_path: Some(tempdir.path().to_path_buf()),
            ..Default::default()
        }));

        assert!(config.is_ok());
        assert!(!config?.commit_types.len() > 0);

        tempdir.close()?;

        Ok(())
    }

    #[test]
    fn test_breaking_changes() -> Result<(), Box<dyn Error>> {
        let config = Config::new(None)?;
        assert!(config.breaking_changes);

        let config = Config::new(Some(ConfigArgs {
            breaking_changes: Some(false),
            ..Default::default()
        }))?;
        assert!(!config.breaking_changes);

        Ok(())
    }

    #[test]
    fn test_issues() -> Result<(), Box<dyn Error>> {
        let config = Config::new(None)?;
        assert!(config.issues);

        let config = Config::new(Some(ConfigArgs {
            issues: Some(false),
            ..Default::default()
        }))?;
        assert!(!config.issues);

        Ok(())
    }

    #[test]
    fn test_commit_types() -> Result<(), Box<dyn Error>> {
        let config = Config::new(None)?;
        let commit_types = config.commit_types;

        assert_eq!(
            commit_types.get("feat"),
            Some(&CommitType {
                name: "feat".into(),
                emoji: Some("✨".into()),
                description: "A new feature".into()
            })
        );

        Ok(())
    }
}
