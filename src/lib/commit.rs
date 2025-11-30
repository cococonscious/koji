use std::{fs::File, io::Write, path::PathBuf};

use anyhow::Result;
use cocogitto::command::commit::CommitOptions;
use cocogitto::CocoGitto;
use git2::Repository;

/// Generates the commit message
pub fn generate_commit_msg(
    commit_type: String,
    scope: Option<String>,
    summary: String,
    body: Option<String>,
    is_breaking_change: bool,
) -> Result<String> {
    let message = CocoGitto::get_conventional_message(
        &commit_type,
        scope,
        summary,
        body,
        None,
        is_breaking_change,
    )?;

    Ok(message)
}

/// Output a commit message to `.git/COMMIT_EDITMSG`
pub fn write_commit_msg(
    repo: &Repository,
    commit_type: String,
    scope: Option<String>,
    summary: String,
    body: Option<String>,
    is_breaking_change: bool,
) -> Result<()> {
    let message = generate_commit_msg(commit_type, scope, summary, body, is_breaking_change)?;

    let commit_editmsg = repo.path().join("COMMIT_EDITMSG");
    let mut file = File::create(commit_editmsg)?;

    file.write_all(message.as_bytes())?;

    Ok(())
}

/// Create a commit
pub fn commit(current_dir: PathBuf, options: CommitOptions) -> Result<()> {
    // Set config path before creating CocoGitto instance (required in 6.4.0+)
    let config_path = current_dir.join("cog.toml");
    cocogitto::set_config_path(config_path.to_string_lossy().to_string());

    let cocogitto = CocoGitto::get_at(current_dir)?;

    cocogitto.conventional_commit(options)?;

    Ok(())
}
