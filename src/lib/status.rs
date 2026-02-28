use std::convert::Infallible;

use anyhow::{Context, Result};
use gix::Repository;

#[derive(Debug, PartialEq, Eq)]
pub enum StagingStatus {
    Empty,
    Partial { staged: usize, unstaged: usize },
    Ready { staged: usize },
}

/// Compares HEAD tree vs index (staged) and index vs worktree (unstaged).
/// Uses an empty tree as the baseline for initial commits.
pub fn check_staging(repo: &Repository) -> Result<StagingStatus> {
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
}
