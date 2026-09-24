use std::convert::Infallible;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use conventional_commit_parser::parse_summary;
use gix::bstr::ByteSlice;

use crate::config::VcsPreference;
use crate::status::StagingStatus;

pub enum VcsBackend {
    Git {
        repo: Box<gix::Repository>,
    },
    #[cfg(feature = "jj")]
    Jj {
        workspace_root: PathBuf,
    },
}

/// Dispatch on the VcsBackend enum, hiding the per-arm `#[cfg(feature = "jj")]`.
macro_rules! match_backend {
    ($self:expr, Git { $($g:tt)* } => $git:expr, Jj { $($j:tt)* } => $jj:expr $(,)?) => {
        match $self {
            Self::Git { $($g)* } => $git,
            #[cfg(feature = "jj")]
            Self::Jj { $($j)* } => $jj,
        }
    };
}

impl VcsBackend {
    /// Detection priority (when auto):
    /// 1. If `jj` feature enabled and `.jj/` exists -> Jj backend
    /// 2. If `.git/` exists -> Git backend
    /// 3. Error: no VCS found
    pub fn detect_with_hint(path: &Path, vcs: Option<VcsPreference>) -> Result<Self> {
        match vcs {
            Some(VcsPreference::Git) => return Self::open_git(path),
            #[cfg(feature = "jj")]
            Some(VcsPreference::Jj) => {
                let mut current = Some(path);
                while let Some(dir) = current {
                    if dir.join(".jj").is_dir() {
                        return Self::open_jj(dir);
                    }
                    current = dir.parent();
                }
                anyhow::bail!("vcs is set to \"jj\" but no .jj/ directory found");
            }
            #[cfg(not(feature = "jj"))]
            Some(VcsPreference::Jj) => {
                anyhow::bail!(
                    "vcs is set to \"jj\" but koji was not compiled with jj support (enable the 'jj' feature)"
                );
            }
            Some(VcsPreference::Auto) | None => {}
        }

        #[cfg(feature = "jj")]
        {
            let mut current = Some(path);
            while let Some(dir) = current {
                if dir.join(".jj").is_dir() {
                    return Self::open_jj(dir);
                }
                current = dir.parent();
            }
        }

        let jj_hint = if cfg!(feature = "jj") { " or jj" } else { "" };
        let ctx = format!("could not find a supported repository (git{jj_hint})");

        let result = Self::open_git(path).context(ctx);

        #[cfg(not(feature = "jj"))]
        if result.is_err() {
            let mut current = Some(path);
            while let Some(dir) = current {
                if dir.join(".jj").is_dir() {
                    anyhow::bail!(
                        "found a .jj/ directory but koji was not compiled with jj support"
                    );
                }
                current = dir.parent();
            }
        }

        result
    }

    fn open_git(path: &Path) -> Result<Self> {
        let repo = gix::discover(path).context("could not find git repository")?;
        Ok(Self::Git {
            repo: Box::new(repo),
        })
    }

    #[cfg(feature = "jj")]
    fn open_jj(path: &Path) -> Result<Self> {
        use anyhow::anyhow;

        let jj_dir = path.join(".jj");
        if !jj_dir.is_dir() {
            return Err(anyhow!("no .jj directory found at {}", path.display()));
        }

        Ok(Self::Jj {
            workspace_root: path.to_path_buf(),
        })
    }

    pub fn supports_hooks(&self) -> bool {
        match_backend!(self, Git { .. } => true, Jj { .. } => false)
    }

    pub fn is_jj(&self) -> bool {
        match_backend!(self, Git { .. } => false, Jj { .. } => true)
    }

    pub fn root_path(&self) -> PathBuf {
        match_backend!(
            self,
            Git { repo } => repo.workdir().unwrap_or_else(|| repo.path()).to_path_buf(),
            Jj { workspace_root, .. } => workspace_root.clone(),
        )
    }

    /// For git, reads `COMMIT_EDITMSG`. For jj, reads the working copy's description.
    pub fn read_current_description(&self) -> Result<Option<String>> {
        match_backend!(
            self,
            Git { repo } => {
                let msg_path = repo.path().join("COMMIT_EDITMSG");
                if msg_path.exists() {
                    let content = std::fs::read_to_string(&msg_path)?;
                    Ok(Some(content))
                } else {
                    Ok(None)
                }
            },
            Jj { workspace_root } => Self::jj_read_current_description(workspace_root),
        )
    }

    /// For git, writes to `COMMIT_EDITMSG`. For jj, describes the current change via jj-lib.
    pub fn write_commit_msg(
        &self,
        commit_type: String,
        scope: Option<String>,
        summary: String,
        body: Option<String>,
        is_breaking_change: bool,
    ) -> Result<()> {
        let message = crate::commit::generate_commit_msg(
            commit_type,
            scope,
            summary,
            body,
            is_breaking_change,
        )?;
        match_backend!(
            self,
            Git { repo } => {
                let commit_editmsg = repo.path().join("COMMIT_EDITMSG");
                let mut file = std::fs::File::create(commit_editmsg)?;
                std::io::Write::write_all(&mut file, message.as_bytes())?;
                Ok(())
            },
            Jj { workspace_root } => Self::jj_write_description(workspace_root, &message),
        )
    }

    pub fn commit_scopes(&self) -> Result<Vec<String>> {
        match_backend!(
            self,
            Git { repo } => {
                let head_id = repo.head_id().context("could not get HEAD")?;

                let walk = repo.rev_walk([head_id.detach()]).sorting(
                    gix::revision::walk::Sorting::ByCommitTime(
                        gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
                    ),
                );

                let mut scopes: Vec<String> = Vec::new();

                for info in walk.all()? {
                    let info = info?;
                    let commit = repo.find_commit(info.id)?;
                    let message = commit.message()?;
                    let summary = message.summary();

                    if let Ok(parsed) = parse_summary(summary.to_str()?) {
                        if let Some(scope) = parsed.scope {
                            if !scopes.contains(&scope) {
                                scopes.push(scope);
                            }
                        }
                    }
                }

                Ok(scopes)
            },
            Jj { workspace_root } => Self::jj_commit_scopes(workspace_root),
        )
    }

    /// For jj, always returns `Ready` since the working copy is always a commit.
    pub fn check_staging(&self) -> Result<StagingStatus> {
        match_backend!(
            self,
            Git { repo } => {
                let index = repo.index_or_empty().context("could not read index")?;
                let head_tree_id = repo
                    .head_tree_id_or_empty()
                    .context("could not resolve HEAD tree")?;

                let mut staged_count: usize = 0;
                repo.tree_index_status(
                    &head_tree_id,
                    &index,
                    None,
                    gix::status::tree_index::TrackRenames::Disabled,
                    |_, _, _| {
                        staged_count += 1;
                        Ok::<_, Infallible>(gix::diff::index::Action::Continue(()))
                    },
                )
                .context("could not diff HEAD tree against index")?;

                let mut unstaged_count: usize = 0;
                let status_iter = repo
                    .status(gix::progress::Discard)
                    .context("could not initialize status")?
                    .index_worktree_options_mut(|opts| {
                        opts.dirwalk_options = None; // only tracked files, not untracked
                    })
                    .into_index_worktree_iter(Vec::<gix::bstr::BString>::new())
                    .context("could not iterate worktree status")?;

                for entry in status_iter {
                    entry.context("error reading worktree status entry")?;
                    unstaged_count += 1;
                }

                match (staged_count, unstaged_count) {
                    (0, _) => Ok(StagingStatus::Empty),
                    (s, 0) => Ok(StagingStatus::Ready { staged: s }),
                    (s, u) => Ok(StagingStatus::Partial {
                        staged: s,
                        unstaged: u,
                    }),
                }
            },
            Jj { .. } => Ok(StagingStatus::Ready { staged: 0 }),
        )
    }

    // ---- jj-specific implementations ----
    //
    // The config-loading helpers below are ported and modified from the jj CLI
    // (cli/src/config.rs) to reproduce the same layer ordering:
    //   Default → EnvBase → User → Repo → EnvOverrides
    // These functions are derived from jj and retain their original copyright:
    // Copyright 2022 The Jujutsu Authors
    //
    // Licensed under the Apache License, Version 2.0 (the "License");
    // you may not use these files except in compliance with the License.
    // You may obtain a copy of the License at
    //
    //     https://www.apache.org/licenses/LICENSE-2.0
    //
    // Unless required by applicable law or agreed to in writing, software
    // distributed under the License is distributed on an "AS IS" BASIS,
    // WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    // See the License for the specific language governing permissions and
    // limitations under the License.

    /// Environment variables that should be overridden by config values.
    ///
    /// Ported from jj cli/src/config.rs `env_base_layer()`.
    /// Modifications: Removed NO_COLOR, VISUAL, EDITOR handling (not needed for koji's use case).
    #[cfg(feature = "jj")]
    fn jj_env_base_layer() -> jj_lib::config::ConfigLayer {
        use jj_lib::config::{ConfigLayer, ConfigSource};

        let mut layer = ConfigLayer::empty(ConfigSource::EnvBase);
        if let Ok(value) = whoami::hostname() {
            layer.set_value("operation.hostname", value).unwrap();
        }
        if let Ok(value) = whoami::username() {
            layer.set_value("operation.username", value).unwrap();
        } else if let Ok(value) = std::env::var("USER") {
            // On Unix, $USER is set by login(1). Use it as a fallback because
            // getpwuid() of musl libc appears not (fully?) supporting nsswitch.
            layer.set_value("operation.username", value).unwrap();
        }
        layer
    }

    /// Environment variables that override config values.
    ///
    /// Ported from jj cli/src/config.rs `env_overrides_layer()`.
    /// Modifications: Removed JJ_EDITOR and JJ_PAGER handling (not needed for koji's use case).
    #[cfg(feature = "jj")]
    fn jj_env_overrides_layer() -> jj_lib::config::ConfigLayer {
        use jj_lib::config::{ConfigLayer, ConfigSource};

        let mut layer = ConfigLayer::empty(ConfigSource::EnvOverrides);
        if let Ok(value) = std::env::var("JJ_USER") {
            layer.set_value("user.name", value).unwrap();
        }
        if let Ok(value) = std::env::var("JJ_EMAIL") {
            layer.set_value("user.email", value).unwrap();
        }
        if let Ok(value) = std::env::var("JJ_TIMESTAMP") {
            layer.set_value("debug.commit-timestamp", value).unwrap();
        }
        if let Ok(Ok(value)) = std::env::var("JJ_RANDOMNESS_SEED").map(|s| s.parse::<i64>()) {
            layer.set_value("debug.randomness-seed", value).unwrap();
        }
        if let Ok(value) = std::env::var("JJ_OP_TIMESTAMP") {
            layer.set_value("debug.operation-timestamp", value).unwrap();
        }
        if let Ok(value) = std::env::var("JJ_OP_HOSTNAME") {
            layer.set_value("operation.hostname", value).unwrap();
        }
        if let Ok(value) = std::env::var("JJ_OP_USERNAME") {
            layer.set_value("operation.username", value).unwrap();
        }
        layer
    }

    /// Resolves the list of user config paths following jj's precedence rules.
    ///
    /// Ported from jj cli/src/config.rs `UnresolvedConfigEnv::resolve()`.
    /// Modifications: Uses `dirs` crate instead of `etcetera` for platform directories.
    #[cfg(feature = "jj")]
    fn jj_user_config_paths() -> Vec<PathBuf> {
        use std::env::split_paths;

        // $JJ_CONFIG takes full precedence when set
        if let Ok(jj_config) = std::env::var("JJ_CONFIG") {
            return split_paths(&jj_config)
                .filter(|p| !p.as_os_str().is_empty())
                .collect();
        }

        let home_dir = dirs::home_dir();
        let config_dir = dirs::config_dir();

        let mut paths = Vec::new();

        let home_config_path = home_dir.map(|d| d.join(".jjconfig.toml"));
        let platform_config_path = config_dir.clone().map(|d| d.join("jj").join("config.toml"));
        let platform_config_dir = config_dir.map(|d| d.join("jj").join("conf.d"));

        if let Some(ref path) = home_config_path {
            if path.exists() || platform_config_path.is_none() {
                paths.push(path.clone());
            }
        }

        // This should be the default config created if there's
        // no user config and `jj config edit` is executed.
        if let Some(path) = platform_config_path {
            paths.push(path);
        }

        if let Some(path) = platform_config_dir {
            if path.exists() {
                paths.push(path);
            }
        }

        paths
    }

    /// Builds a jj StackedConfig matching the layer ordering used by the jj CLI.
    ///
    /// Ported from jj cli/src/config.rs `config_from_environment()` +
    /// `ConfigEnv::reload_user_config()`.
    /// Modifications: Simplified to only load user and repo config layers.
    #[cfg(feature = "jj")]
    fn jj_load_config(workspace_root: &Path) -> Result<jj_lib::config::StackedConfig> {
        use jj_lib::config::{ConfigSource, StackedConfig};

        let mut config = StackedConfig::with_defaults();

        // EnvBase
        config.add_layer(Self::jj_env_base_layer());

        // User config files
        for path in Self::jj_user_config_paths() {
            if path.is_dir() {
                let _ = config.load_dir(ConfigSource::User, &path);
            } else if path.exists() {
                let _ = config.load_file(ConfigSource::User, &path);
            }
        }

        // Repo-level config
        let repo_config = workspace_root.join(".jj").join("repo").join("config.toml");
        if repo_config.is_file() {
            let _ = config.load_file(ConfigSource::Repo, &repo_config);
        }

        // EnvOverrides
        config.add_layer(Self::jj_env_overrides_layer());

        Ok(config)
    }

    #[cfg(feature = "jj")]
    fn jj_load_repo(
        workspace_root: &Path,
    ) -> Result<(
        std::sync::Arc<jj_lib::repo::ReadonlyRepo>,
        jj_lib::ref_name::WorkspaceNameBuf,
    )> {
        use jj_lib::repo::StoreFactories;
        use jj_lib::settings::UserSettings;
        use jj_lib::workspace::{default_working_copy_factories, Workspace};
        use pollster::FutureExt as _;

        let config = Self::jj_load_config(workspace_root)?;
        let settings =
            UserSettings::from_config(config).context("could not create jj UserSettings")?;
        let store_factories = StoreFactories::default();
        let wc_factories = default_working_copy_factories();

        let workspace = Workspace::load(&settings, workspace_root, &store_factories, &wc_factories)
            .context("could not load jj workspace")?;
        let workspace_name = workspace.workspace_name().to_owned();

        let repo = workspace
            .repo_loader()
            .load_at_head()
            .block_on()
            .context("could not load jj repo at head")?;

        Ok((repo, workspace_name))
    }

    #[cfg(feature = "jj")]
    fn jj_wc_commit(
        repo: &std::sync::Arc<jj_lib::repo::ReadonlyRepo>,
        workspace_name: &jj_lib::ref_name::WorkspaceName,
    ) -> Result<jj_lib::commit::Commit> {
        use jj_lib::repo::Repo as _;

        let wc_commit_id = repo
            .view()
            .get_wc_commit_id(workspace_name)
            .context("no working copy commit found for current workspace")?;

        let commit = repo
            .store()
            .get_commit(wc_commit_id)
            .context("could not get working copy commit")?;

        Ok(commit)
    }

    #[cfg(feature = "jj")]
    fn jj_read_current_description(workspace_root: &Path) -> Result<Option<String>> {
        let (repo, workspace_name) = Self::jj_load_repo(workspace_root)?;
        let commit = Self::jj_wc_commit(&repo, &workspace_name)?;
        let desc = commit.description().to_string();
        if desc.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(desc))
        }
    }

    #[cfg(feature = "jj")]
    fn jj_write_description(workspace_root: &Path, message: &str) -> Result<()> {
        use pollster::FutureExt as _;

        let (repo, workspace_name) = Self::jj_load_repo(workspace_root)?;
        let commit = Self::jj_wc_commit(&repo, &workspace_name)?;

        let mut tx = repo.start_transaction();
        tx.repo_mut()
            .rewrite_commit(&commit)
            .set_description(message)
            .write()
            .block_on()
            .context("could not write jj commit description")?;
        tx.repo_mut()
            .rebase_descendants()
            .block_on()
            .context("could not rebase descendants after jj describe")?;
        tx.commit("koji: describe change")
            .block_on()
            .context("could not commit jj transaction")?;

        Ok(())
    }

    #[cfg(feature = "jj")]
    fn jj_commit_scopes(workspace_root: &Path) -> Result<Vec<String>> {
        use jj_lib::repo::Repo as _;
        use jj_lib::revset::{ResolvedRevsetExpression, RevsetIteratorExt as _};

        let (repo, workspace_name) = Self::jj_load_repo(workspace_root)?;
        let commit = Self::jj_wc_commit(&repo, &workspace_name)?;

        let ancestors_expr =
            ResolvedRevsetExpression::commits(vec![commit.id().clone()]).ancestors();
        let revset = ancestors_expr
            .evaluate(repo.as_ref())
            .context("could not evaluate jj revset")?;

        let mut scopes: Vec<String> = Vec::new();

        for commit_result in revset.iter().commits(repo.store()) {
            let ancestor = match commit_result {
                Ok(c) => c,
                Err(_) => break,
            };
            let desc = ancestor.description().to_string();
            let first_line = desc.lines().next().unwrap_or("");
            if let Ok(parsed) = parse_summary(first_line) {
                if let Some(scope) = parsed.scope {
                    if !scopes.contains(&scope) {
                        scopes.push(scope);
                    }
                }
            }
        }

        Ok(scopes)
    }
}

#[cfg(feature = "jj")]
#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;
    use std::env;

    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};

    struct EnvRestoreGuard(HashMap<OsString, Option<OsString>>);

    impl Drop for EnvRestoreGuard {
        fn drop(&mut self) {
            for (key, value) in self.0.iter() {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }

    impl EnvRestoreGuard {
        pub fn new() -> Self {
            Self(HashMap::new())
        }

        pub fn set_var<K, V>(&mut self, key: K, value: V)
        where
            K: Into<OsString>,
            V: AsRef<OsStr>,
        {
            let key = key.into();
            let val = env::var_os(&key);
            env::set_var(&key, value);
            self.0.insert(key, val);
        }

        pub fn remove_var<K>(&mut self, key: K)
        where
            K: Into<OsString>,
        {
            let key = key.into();
            let val = env::var_os(&key);
            env::remove_var(&key);
            self.0.insert(key, val);
        }
    }

    /// Tests that jj_env_base_layer creates a valid layer
    #[test]
    #[serial]
    fn test_jj_env_base_layer_creates_valid_layer() {
        let layer = VcsBackend::jj_env_base_layer();
        assert!(matches!(
            layer.source,
            jj_lib::config::ConfigSource::EnvBase
        ));
    }

    /// Tests jj_env_overrides_layer with all JJ_* environment variables set
    #[test]
    #[serial]
    fn test_jj_env_overrides_layer_with_all_vars() {
        let mut guard = EnvRestoreGuard::new();

        guard.set_var("JJ_USER", "Test User");
        guard.set_var("JJ_EMAIL", "test@example.com");
        guard.set_var("JJ_TIMESTAMP", "1234567890");
        guard.set_var("JJ_RANDOMNESS_SEED", "42");
        guard.set_var("JJ_OP_TIMESTAMP", "0987654321");
        guard.set_var("JJ_OP_HOSTNAME", "test-host");
        guard.set_var("JJ_OP_USERNAME", "test-username");

        let layer = VcsBackend::jj_env_overrides_layer();

        assert!(matches!(
            layer.source,
            jj_lib::config::ConfigSource::EnvOverrides
        ));
    }

    /// Tests jj_env_overrides_layer without any JJ_* environment variables
    #[test]
    #[serial]
    fn test_jj_env_overrides_layer_empty() {
        let mut guard = EnvRestoreGuard::new();

        for var in [
            "JJ_USER",
            "JJ_EMAIL",
            "JJ_TIMESTAMP",
            "JJ_RANDOMNESS_SEED",
            "JJ_OP_TIMESTAMP",
            "JJ_OP_HOSTNAME",
            "JJ_OP_USERNAME",
        ] {
            guard.remove_var(var);
        }

        let layer = VcsBackend::jj_env_overrides_layer();

        assert!(matches!(
            layer.source,
            jj_lib::config::ConfigSource::EnvOverrides
        ));
    }

    /// Tests jj_user_config_paths with JJ_CONFIG environment variable (single path)
    #[test]
    #[serial]
    fn test_jj_user_config_paths_jj_config_single() {
        let mut guard = EnvRestoreGuard::new();

        guard.set_var("JJ_CONFIG", "/tmp/test/jjconfig.toml");

        let paths = VcsBackend::jj_user_config_paths();

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("/tmp/test/jjconfig.toml"));
    }

    /// Tests jj_user_config_paths with multiple paths in JJ_CONFIG
    #[test]
    #[serial]
    fn test_jj_user_config_paths_jj_config_multiple() {
        let mut guard = EnvRestoreGuard::new();

        // Use (semi)colon-separated paths which split_paths handles
        guard.set_var(
            "JJ_CONFIG",
            cfg_select! {
                unix => "/tmp/test1:/tmp/test2",
                _ => "/tmp/test1;/tmp/test2"
            },
        );

        let paths = VcsBackend::jj_user_config_paths();

        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&PathBuf::from("/tmp/test1")));
        assert!(paths.contains(&PathBuf::from("/tmp/test2")));
    }

    /// Tests that jj_user_config_paths filters out empty paths from JJ_CONFIG
    #[test]
    #[serial]
    fn test_jj_user_config_paths_filters_empty() {
        let mut guard = EnvRestoreGuard::new();

        // Use (semi)colon-separated paths with empty entries
        guard.set_var(
            "JJ_CONFIG",
            cfg_select! {
                unix => "/tmp/test1:::/tmp/test2",
                _ => "/tmp/test1;;;/tmp/test2"
            },
        );

        let paths = VcsBackend::jj_user_config_paths();

        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&PathBuf::from("/tmp/test1")));
        assert!(paths.contains(&PathBuf::from("/tmp/test2")));
    }

    /// Tests jj_user_config_paths without JJ_CONFIG set (uses default paths)
    #[test]
    #[serial]
    #[cfg(target_os = "linux")]
    fn test_jj_user_config_paths_default() {
        let mut guard = EnvRestoreGuard::new();
        guard.remove_var("JJ_CONFIG");

        // Set HOME to a temporary directory for reproducibility
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let home_path = temp_dir.path();
        guard.set_var("HOME", home_path);

        let xdg_config = home_path.join(".config");
        std::fs::create_dir_all(&xdg_config).expect("Failed to create XDG_CONFIG_HOME");
        guard.set_var("XDG_CONFIG_HOME", &xdg_config);

        let jj_config_dir = xdg_config.join("jj");
        std::fs::create_dir_all(&jj_config_dir).expect("Failed to create jj config dir");

        let paths = VcsBackend::jj_user_config_paths();

        let expected_platform_config = jj_config_dir.join("config.toml");
        assert!(!paths.is_empty());
        assert!(
            paths.contains(&expected_platform_config),
            "Should contain platform config path: {}",
            expected_platform_config.display()
        );
    }

    /// Tests jj_load_config creates a valid StackedConfig with basic structure
    #[test]
    #[serial]
    fn test_jj_load_config_creates_valid_config() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let workspace_root = temp_dir.path();

        let jj_dir = workspace_root.join(".jj").join("repo");
        std::fs::create_dir_all(&jj_dir).expect("Failed to create .jj/repo dir");

        let config = VcsBackend::jj_load_config(workspace_root).expect("Failed to load config");

        assert!(!config.layers().is_empty());
    }

    /// Tests jj_load_config loads repo config file when present
    #[test]
    #[serial]
    fn test_jj_load_config_with_repo_config() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let workspace_root = temp_dir.path();

        let jj_dir = workspace_root.join(".jj").join("repo");
        std::fs::create_dir_all(&jj_dir).expect("Failed to create .jj/repo dir");

        let repo_config_path = jj_dir.join("config.toml");
        std::fs::write(
            &repo_config_path,
            r#"
[user]
name = "Test Repo User"
email = "repo-test@example.com"
"#,
        )
        .expect("Failed to write repo config");

        let config = VcsBackend::jj_load_config(workspace_root).expect("Failed to load config");

        assert!(!config.layers().is_empty());
    }
}
