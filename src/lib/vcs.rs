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

        Self::open_git(path)
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

    #[cfg(feature = "jj")]
    fn jj_load_repo(workspace_root: &Path) -> Result<std::sync::Arc<jj_lib::repo::ReadonlyRepo>> {
        use jj_lib::config::StackedConfig;
        use jj_lib::repo::StoreFactories;
        use jj_lib::settings::UserSettings;
        use jj_lib::workspace::{default_working_copy_factories, Workspace};
        use pollster::FutureExt as _;

        let config = StackedConfig::with_defaults();
        let settings =
            UserSettings::from_config(config).context("could not create jj UserSettings")?;
        let store_factories = StoreFactories::default();
        let wc_factories = default_working_copy_factories();

        let workspace = Workspace::load(&settings, workspace_root, &store_factories, &wc_factories)
            .context("could not load jj workspace")?;

        let repo = workspace
            .repo_loader()
            .load_at_head()
            .block_on()
            .context("could not load jj repo at head")?;

        Ok(repo)
    }

    #[cfg(feature = "jj")]
    fn jj_wc_commit(
        repo: &std::sync::Arc<jj_lib::repo::ReadonlyRepo>,
    ) -> Result<jj_lib::commit::Commit> {
        use jj_lib::ref_name::WorkspaceName;
        use jj_lib::repo::Repo as _;

        let wc_commit_id = repo
            .view()
            .get_wc_commit_id(WorkspaceName::DEFAULT)
            .context("no working copy commit found for default workspace")?;

        let commit = repo
            .store()
            .get_commit(wc_commit_id)
            .context("could not get working copy commit")?;

        Ok(commit)
    }

    #[cfg(feature = "jj")]
    fn jj_read_current_description(workspace_root: &Path) -> Result<Option<String>> {
        let repo = Self::jj_load_repo(workspace_root)?;
        let commit = Self::jj_wc_commit(&repo)?;
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

        let repo = Self::jj_load_repo(workspace_root)?;
        let commit = Self::jj_wc_commit(&repo)?;

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

        let repo = Self::jj_load_repo(workspace_root)?;
        let commit = Self::jj_wc_commit(&repo)?;

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
            if let Ok(parsed) = parse_summary(&desc) {
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
