use std::{fs::File, io::Write, path::PathBuf};

use anyhow::{anyhow, Result};
use cocogitto::command::commit::CommitOptions;
use cocogitto::{CocoGitto, CommitHook};
use gix::Repository;

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
///
/// Staging is always performed by koji (never delegated to cocogitto) so that
/// the same set of files is staged regardless of whether `no_verify` is set.
/// When staging is requested (`add_files` or `update_files`) only tracked
/// modified/deleted files are staged, matching `git commit -a` semantics and
/// koji's documented `--all` flag.
///
/// When `no_verify` is `false`, the `pre-commit` and `post-commit` git hooks
/// are invoked around the commit. `post-commit` failures are reported as a
/// warning and do not abort, matching `git commit`.
pub fn commit(current_dir: PathBuf, mut options: CommitOptions, no_verify: bool) -> Result<()> {
    // Set config path before creating CocoGitto instance (required in 6.4.0+)
    let config_path = current_dir.join("cog.toml");
    cocogitto::set_config_path(config_path.to_string_lossy().to_string());

    let stage_requested = options.add_files || options.update_files;
    options.add_files = false;
    options.update_files = false;

    if stage_requested {
        let repo = gix::discover(&current_dir)?;
        stage_tracked(&repo)?;
    }

    let cocogitto = CocoGitto::get_at(current_dir)?;

    if !no_verify {
        cocogitto.run_commit_hook(CommitHook::PreCommit)?;
    }

    cocogitto.conventional_commit(options)?;

    if !no_verify {
        if let Err(e) = cocogitto.run_commit_hook(CommitHook::PostCommit) {
            eprintln!("warning: post-commit hook failed: {e}");
        }
    }

    Ok(())
}

/// Stage tracked modified/deleted files via gix, equivalent to `git add -u`.
fn stage_tracked(repo: &Repository) -> Result<()> {
    use gix::status::index_worktree::Item;
    use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};

    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow!("repository has no working directory"))?
        .to_path_buf();

    let mut to_remove: Vec<gix::bstr::BString> = Vec::new();
    let mut to_update: Vec<gix::bstr::BString> = Vec::new();

    let iter = repo
        .status(gix::progress::Discard)?
        .into_index_worktree_iter(Vec::new())?;

    for item in iter {
        if let Item::Modification {
            rela_path, status, ..
        } = item?
        {
            match status {
                EntryStatus::Change(Change::Removed) => to_remove.push(rela_path),
                EntryStatus::Change(Change::Modification { .. } | Change::Type { .. }) => {
                    to_update.push(rela_path);
                }
                _ => {}
            }
        }
    }

    if to_remove.is_empty() && to_update.is_empty() {
        return Ok(());
    }

    let index_path = repo.index_path();
    let mut index =
        gix::index::File::at(&index_path, repo.object_hash(), false, Default::default())?;

    if !to_remove.is_empty() {
        index.remove_entries(|_, path, _| {
            let path_bytes: &[u8] = path.as_ref();
            to_remove.iter().any(|p| p.as_slice() == path_bytes)
        });
    }

    for rela in to_update {
        let bstr = rela.as_ref();
        let abs = workdir.join(gix::path::from_bstr(bstr).as_ref());
        let bytes = std::fs::read(&abs)?;
        let oid = repo.write_blob(&bytes)?.detach();
        let metadata = gix::index::fs::Metadata::from_path_no_follow(&abs)?;
        let stat = gix::index::entry::Stat::from_fs(&metadata)?;
        if let Some(entry) =
            index.entry_mut_by_path_and_stage(bstr, gix::index::entry::Stage::Unconflicted)
        {
            entry.id = oid;
            // Refresh stat from disk so `git status` doesn't flag the file as
            // modified after commit due to a stale mtime/size in the index.
            entry.stat = stat;
        }
    }

    // Drop the cached TREE extension. Mutating entries above leaves the cache
    // pointing at the pre-edit tree; libgit2's `index.write_tree()` honors that
    // cache and would otherwise return the old tree id, producing an empty
    // commit (same tree as HEAD) when cocogitto writes the commit.
    index.remove_tree();

    index.write(gix::index::write::Options::default())?;
    Ok(())
}
