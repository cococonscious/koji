use anyhow::Result;
use config::FileFormat;
use dirs::config_dir;
use indexmap::IndexMap;
use serde::Deserialize;
use std::env::current_dir;
use std::fmt;
use std::path::PathBuf;
#[cfg(any(unix, target_os = "redox"))]
use xdg::BaseDirectories;

#[derive(Debug, Clone)]
pub struct Config {
    pub autocomplete: bool,
    pub breaking_changes: bool,
    pub commit_types: IndexMap<String, CommitType>,
    pub commit_scopes: IndexMap<String, CommitScope>,
    pub emoji: bool,
    pub issues: bool,
    pub sign: bool,
    pub force_config_scopes: bool,
    pub allow_empty_scope: bool,
    pub workdir: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct CommitType {
    pub description: String,
    pub emoji: Option<String>,
    pub name: String,
}

/// A configured commit scope, optionally with path patterns and/or an AST-grep rule
/// for automatic scope detection from staged changes.
#[derive(Clone, Deserialize)]
pub struct CommitScope {
    pub name: String,
    pub description: Option<String>,

    /// Regex patterns matched against staged file paths (prefixed with `/`).
    #[serde(default)]
    pub patterns: Option<ScopePatternValue>,

    /// AST-grep rule that pre-assigns this scope when it matches staged file content.
    #[cfg(feature = "ast-grep")]
    #[serde(default)]
    pub ast_grep: Option<ast_grep_config::SerializableRuleConfig<ast_grep_language::SupportLang>>,
}

impl PartialEq for CommitScope {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.description == other.description
            && self.patterns == other.patterns
    }
}

impl fmt::Debug for CommitScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommitScope")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("patterns", &self.patterns)
            .finish_non_exhaustive()
    }
}

/// A wrapper around the ast-grep rule config, with a manual Debug impl.
#[cfg(feature = "ast-grep")]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ScopePatternValue {
    One(String),
    Many(Vec<String>),
}

impl ScopePatternValue {
    pub fn iter(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self {
            Self::One(pattern) => Box::new(std::iter::once(pattern.as_str())),
            Self::Many(patterns) => Box::new(patterns.iter().map(String::as_str)),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct ConfigTOML {
    pub autocomplete: bool,
    pub breaking_changes: bool,
    #[serde(default)]
    commit_types: Vec<CommitType>,
    #[serde(default)]
    commit_scopes: Vec<CommitScope>,
    pub emoji: bool,
    pub issues: bool,
    pub sign: bool,
    pub force_config_scopes: bool,
    pub allow_empty_scope: bool,
}

#[derive(Default)]
pub struct ConfigArgs {
    pub path: Option<PathBuf>,
    pub autocomplete: Option<bool>,
    pub breaking_changes: Option<bool>,
    pub emoji: Option<bool>,
    pub issues: Option<bool>,
    pub sign: Option<bool>,
    pub force_scope: Option<bool>,
    pub allow_empty_scope: Option<bool>,
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
            force_scope,
            allow_empty_scope,
            _user_config_path,
            _current_dir,
        } = args.unwrap_or_default();

        let mut settings = config::Config::builder();

        let workdir = _current_dir.unwrap_or(current_dir()?);

        // Get the default config
        let default_str = include_str!("../../meta/config/default.toml");
        settings = settings.add_source(config::File::from_str(default_str, FileFormat::Toml));

        // Define the order in which configuration directories will be loaded
        let mut config_dirs = vec![config_dir()];
        #[cfg(any(unix, target_os = "redox"))]
        config_dirs.push(BaseDirectories::new().get_config_home());
        config_dirs.push(_user_config_path);

        settings = config_dirs
            .into_iter()
            .flatten()
            .map(|d| d.join("koji/config.toml"))
            .map(|d| config::File::from(d).required(false))
            .fold(settings, |prev, cfg| prev.add_source(cfg));

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

        // Gather up commit scopes (patterns and ast_grep are inline on each scope)
        let mut commit_scopes = IndexMap::new();
        for commit_scope in config.commit_scopes.iter() {
            commit_scopes.insert(commit_scope.name.clone(), commit_scope.to_owned());
        }

        let config = Config {
            autocomplete: autocomplete.unwrap_or(config.autocomplete),
            breaking_changes: breaking_changes.unwrap_or(config.breaking_changes),
            commit_types,
            commit_scopes,
            emoji: emoji.unwrap_or(config.emoji),
            issues: issues.unwrap_or(config.issues),
            sign: sign.unwrap_or(config.sign),
            force_config_scopes: force_scope.unwrap_or(config.force_config_scopes),
            allow_empty_scope: allow_empty_scope.unwrap_or(config.allow_empty_scope),
            workdir,
        };

        config.validate_scope_patterns()?;
        #[cfg(feature = "ast-grep")]
        config.validate_ast_grep_rules()?;

        Ok(config)
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

    #[test]
    fn test_commit_scopes() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        std::fs::write(
            tempdir.path().join(".koji.toml"),
            "[[commit_scopes]]\nname=\"app\"\ndescription=\"Application code\"",
        )?;
        let config = Config::new(Some(ConfigArgs {
            _current_dir: Some(tempdir.path().to_path_buf()),
            ..Default::default()
        }))?;
        assert!(config.commit_scopes.get("app").is_some());
        let scope = config.commit_scopes.get("app").unwrap();
        assert_eq!(scope.name, "app");
        assert_eq!(scope.description, Some("Application code".into()));
        tempdir.close()?;
        Ok(())
    }

    #[test]
    fn test_commit_scopes_from_config() -> Result<(), Box<dyn Error>> {
        let tempdir_config = tempfile::tempdir()?;
        std::fs::create_dir(tempdir_config.path().join("koji"))?;
        std::fs::write(
            tempdir_config.path().join("koji").join("config.toml"),
            "[[commit_scopes]]\nname=\"server\"\ndescription=\"Server code\"\n[[commit_scopes]]\nname=\"shared\"",
        )?;
        let tempdir_current = tempfile::tempdir()?;
        let config = Config::new(Some(ConfigArgs {
            _user_config_path: Some(tempdir_config.path().to_path_buf()),
            _current_dir: Some(tempdir_current.path().to_path_buf()),
            ..Default::default()
        }))?;
        assert!(config.commit_scopes.get("server").is_some());
        assert!(config.commit_scopes.get("shared").is_some());
        assert_eq!(config.commit_scopes.len(), 2);
        tempdir_current.close()?;
        tempdir_config.close()?;
        Ok(())
    }

    #[test]
    fn test_scope_patterns_inline() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        std::fs::write(
            tempdir.path().join(".koji.toml"),
            "[[commit_scopes]]\nname=\"core\"\ndescription=\"Core crate\"\npatterns = \"/crates/core/**/*.rs\"\n[[commit_scopes]]\nname=\"build\"\npatterns = [\"^/build\\\\.rs$\", \"/justfile\"]",
        )?;

        let config = Config::new(Some(ConfigArgs {
            _current_dir: Some(tempdir.path().to_path_buf()),
            ..Default::default()
        }))?;

        assert!(config.commit_scopes.contains_key("core"));
        assert!(config.commit_scopes.contains_key("build"));

        let core = config.commit_scopes.get("core").unwrap();
        assert_eq!(core.description, Some("Core crate".into()));
        assert_eq!(
            core.patterns,
            Some(ScopePatternValue::One("/crates/core/**/*.rs".into()))
        );

        let build = config.commit_scopes.get("build").unwrap();
        assert_eq!(
            build.patterns,
            Some(ScopePatternValue::Many(vec![
                "^/build\\.rs$".into(),
                "/justfile".into()
            ]))
        );

        tempdir.close()?;
        Ok(())
    }

    #[cfg(feature = "ast-grep")]
    #[test]
    fn test_scope_ast_grep_inline() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        // Use a subtable for ast_grep within the commit_scopes array entry
        std::fs::write(
            tempdir.path().join(".koji.toml"),
            "[[commit_scopes]]\nname = \"test\"\ndescription = \"Test functions\"\n\n[commit_scopes.ast_grep]\nlanguage = \"Rust\"\nrule = { kind = \"function_item\" }\nfiles = [\"**/*.rs\"]\n",
        )?;

        let config = Config::new(Some(ConfigArgs {
            _current_dir: Some(tempdir.path().to_path_buf()),
            ..Default::default()
        }))?;

        assert!(config.commit_scopes.contains_key("test"));
        let scope = config.commit_scopes.get("test").unwrap();
        assert_eq!(scope.description, Some("Test functions".into()));
        assert!(scope.ast_grep.is_some());

        tempdir.close()?;
        Ok(())
    }
}
