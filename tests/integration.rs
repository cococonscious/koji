use git2::{Commit, Repository, RepositoryInitOptions};
#[cfg(not(target_os = "windows"))]
use rexpect::{process::wait, session::spawn_command};
use std::{error::Error, fs, path::PathBuf, process::Command};
use tempfile::TempDir;

fn setup_test_dir() -> Result<(PathBuf, TempDir, Repository), Box<dyn Error>> {
    let bin_path = assert_cmd::cargo::cargo_bin("koji");
    let temp_dir = tempfile::tempdir()?;
    let mut init_options = RepositoryInitOptions::new();
    init_options.initial_head("main");
    let repo = Repository::init_opts(&temp_dir, &init_options)?;

    Ok((bin_path, temp_dir, repo))
}

fn get_first_commit(repo: &Repository) -> Result<Commit, git2::Error> {
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    let oid = walk.next().expect("cannot get commit in revwalk")?;

    repo.find_commit(oid)
}

#[test]
#[cfg(not(target_os = "windows"))]
fn test_type_scope_summary_body_breaking_issue_add_files_correct() -> Result<(), Box<dyn Error>> {
    let (bin_path, temp_dir, repo) = setup_test_dir()?;

    fs::write(temp_dir.path().join("config.json"), "abc")?;

    let mut cmd = Command::new(bin_path);
    cmd.env("NO_COLOR", "1")
        .arg("-C")
        .arg(temp_dir.path())
        .arg("-a");

    let mut process = spawn_command(cmd, Some(5000))?;

    process.exp_string("are you committing?")?;
    process.send_line("feat")?;
    process.exp_string("scope of this change?")?;
    process.send_line("config")?;
    process.exp_string("description of the change")?;
    process.send_line("refactor config pairs")?;
    process.exp_string("longer description of the change")?;
    process
        .send_line("Removed and added a config pair each\\nNecessary for future compatibility.")?;
    process.exp_string("breaking changes?")?;
    process.send_line("Y")?;
    process.exp_string("in detail:")?;
    process.send_line("Something can't be configured anymore")?;
    process.exp_string("any open issues?")?;
    process.send_line("Y")?;
    process.exp_string("issue reference:")?;
    process.send_line("closes #1")?;
    let eof_output = process.exp_eof();

    let exitcode = process.process.wait()?;
    let success = matches!(exitcode, wait::WaitStatus::Exited(_, 0));

    if !success {
        panic!("Command exited non-zero, end of output: {:?}", eof_output);
    }

    let commit = get_first_commit(&repo)?;
    assert_eq!(
        commit.summary(),
        Some("feat(config)!: refactor config pairs")
    );
    assert_eq!(
        commit.body(),
        Some("Removed and added a config pair each\nNecessary for future compatibility.\n\ncloses #1\nBREAKING CHANGE: Something can't be configured anymore")
    );

    temp_dir.close()?;
    Ok(())
}
