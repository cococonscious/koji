use std::path::PathBuf;

use anyhow::Result;
use cocogitto::command::commit::CommitOptions;
use cocogitto::CocoGitto;

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

/// Create a commit
pub fn commit(current_dir: PathBuf, options: CommitOptions) -> Result<()> {
    // Set config path before creating CocoGitto instance (required in 6.4.0+)
    let config_path = current_dir.join("cog.toml");
    cocogitto::set_config_path(config_path.to_string_lossy().to_string());

    let cocogitto = CocoGitto::get_at(current_dir)?;

    cocogitto.conventional_commit(options)?;

    Ok(())
}
