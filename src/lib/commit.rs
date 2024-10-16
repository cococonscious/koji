use std::{fs::File, io::Write};

use anyhow::Result;
use cocogitto::CocoGitto;
use conventional_commit_parser::commit::{CommitType::Custom, ConventionalCommit};
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
    let message = ConventionalCommit {
        commit_type: Custom(commit_type),
        scope,
        summary,
        body,
        is_breaking_change,
        footers: vec![],
    }
    .to_string();

    let commit_editmsg = repo.path().join("COMMIT_EDITMSG");
    let mut file = File::create(commit_editmsg)?;

    file.write_all(message.as_bytes())?;

    Ok(())
}

/// Create a commit
#[cfg(not(tarpaulin_include))]
pub fn commit(
    commit_type: String,
    scope: Option<String>,
    summary: String,
    body: Option<String>,
    is_breaking_change: bool,
    sign: bool,
) -> Result<()> {
    let cocogitto = CocoGitto::get()?;

    cocogitto.conventional_commit(
        &commit_type,
        scope,
        summary,
        body,
        None,
        is_breaking_change,
        sign,
    )?;

    Ok(())
}
