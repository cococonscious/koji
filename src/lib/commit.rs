use std::{fs::File, io::Write, path::PathBuf};

use anyhow::Result;
use cocogitto::command::commit::CommitOptions;
use cocogitto::CocoGitto;
use git2::Repository;

/// Output a commit message to `.git/COMMIT_EDITMSG`
#[cfg(not(tarpaulin_include))]
pub fn write_commit_msg(
    repo: &Repository,
    commit_type: String,
    scope: Option<String>,
    summary: String,
    body: Option<String>,
    is_breaking_change: bool,
) -> Result<()> {
    let message = CocoGitto::get_conventional_message(
        &commit_type,
        scope,
        summary,
        body,
        None,
        is_breaking_change,
    )?;

    let commit_editmsg = repo.path().join("COMMIT_EDITMSG");
    let mut file = File::create(commit_editmsg)?;

    file.write_all(message.as_bytes())?;

    Ok(())
}

/// Create a commit
#[cfg(not(tarpaulin_include))]
pub fn commit(current_dir: PathBuf, options: CommitOptions) -> Result<()> {
    let cocogitto = CocoGitto::get_at(current_dir)?;

    cocogitto.conventional_commit(options)?;

    Ok(())
}
