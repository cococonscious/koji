use git2::{Commit, Repository, RepositoryInitOptions, IndexAddOption};
#[cfg(not(target_os = "windows"))]
use rexpect::{
    process::wait,
    session::{spawn_command, PtySession},
};
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

trait ExpectPromps {
    fn expect_commit_type(&mut self) -> Result<String, rexpect::error::Error>;
    fn expect_scope(&mut self) -> Result<String, rexpect::error::Error>;
    fn expect_summary(&mut self) -> Result<String, rexpect::error::Error>;
    fn expect_body(&mut self) -> Result<String, rexpect::error::Error>;
    fn expect_breaking(&mut self) -> Result<String, rexpect::error::Error>;
    fn expect_breaking_details(&mut self) -> Result<String, rexpect::error::Error>;
    fn expect_issues(&mut self) -> Result<String, rexpect::error::Error>;
    fn expect_issues_details(&mut self) -> Result<String, rexpect::error::Error>;
}

impl ExpectPromps for PtySession {
    fn expect_commit_type(&mut self) -> Result<String, rexpect::error::Error> {
        self.exp_string("are you committing?")
    }

    fn expect_scope(&mut self) -> Result<String, rexpect::error::Error> {
        self.exp_string("scope of this change?")
    }

    fn expect_summary(&mut self) -> Result<String, rexpect::error::Error> {
        self.exp_string("description of the change")
    }

    fn expect_body(&mut self) -> Result<String, rexpect::error::Error> {
        self.exp_string("longer description of the change")
    }

    fn expect_breaking(&mut self) -> Result<String, rexpect::error::Error> {
        self.exp_string("breaking changes?")
    }

    fn expect_breaking_details(&mut self) -> Result<String, rexpect::error::Error> {
        self.exp_string("in detail:")
    }

    fn expect_issues(&mut self) -> Result<String, rexpect::error::Error> {
        self.exp_string("any open issues?")
    }

    fn expect_issues_details(&mut self) -> Result<String, rexpect::error::Error> {
        self.exp_string("issue reference:")
    }
}

#[test]
#[cfg(not(target_os = "windows"))]
fn test_everything_correct() -> Result<(), Box<dyn Error>> {
    let (bin_path, temp_dir, repo) = setup_test_dir()?;

    fs::write(temp_dir.path().join("config.json"), "abc")?;
    // TODO properly test "-a"
    // println!("{:?}", repo.status_file(Path::new("config.json"))?);
    repo.index()?
        .add_all(["*"].iter(), IndexAddOption::default(), None)?;
    // println!("{:?}", repo.status_file(Path::new("config.json"))?);

    let mut cmd = Command::new(bin_path);
    cmd.env("NO_COLOR", "1")
        .arg("-C")
        .arg(temp_dir.path())
        .arg("-a")
        .arg("--autocomplete=true");

    let mut process = spawn_command(cmd, Some(5000))?;

    process.expect_commit_type()?;
    process.send_line("feat")?;
    process.expect_scope()?;
    process.send_line("config")?;
    process.expect_summary()?;
    process.send_line("refactor config pairs")?;
    process.expect_body()?;
    process
        .send_line("Removed and added a config pair each\\nNecessary for future compatibility.")?;
    process.expect_breaking()?;
    process.send_line("Y")?;
    process.expect_breaking_details()?;
    process.send_line("Something can't be configured anymore")?;
    process.expect_issues()?;
    process.send_line("Y")?;
    process.expect_issues_details()?;
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

#[test]
#[cfg(not(target_os = "windows"))]
fn test_hook_correct() -> Result<(), Box<dyn Error>> {
    let (bin_path, temp_dir, repo) = setup_test_dir()?;

    fs::write(temp_dir.path().join("config.json"), "abc")?;
    repo.index()?
        .add_all(["*"].iter(), IndexAddOption::default(), None)?;
    fs::remove_file(temp_dir.path().join(".git").join("COMMIT_EDITMSG")).unwrap_or(());

    let mut cmd = Command::new(bin_path);
    cmd.env("NO_COLOR", "1")
        .arg("-C")
        .arg(temp_dir.path())
        .arg("--autocomplete=false");

    let mut process = spawn_command(cmd, Some(5000))?;

    process.expect_commit_type()?;
    process.send_line("fix")?;
    process.expect_scope()?;
    process.send_line("")?;
    process.expect_summary()?;
    process.send_line("some weird error")?;
    process.expect_body()?;
    process.send_line("")?;
    process.expect_breaking()?;
    process.send_line("N")?;
    process.expect_issues()?;
    process.send_line("N")?;
    let eof_output = process.exp_eof();

    let exitcode = process.process.wait()?;
    let success = matches!(exitcode, wait::WaitStatus::Exited(_, 0));

    if !success {
        panic!("Command exited non-zero, end of output: {:?}", eof_output);
    }

    let editmsg = temp_dir.path().join(".git").join("COMMIT_EDITMSG");
    assert!(editmsg.exists());
    assert_eq!(
        fs::read(editmsg)?,
        "fix: some weird error".as_bytes().to_vec()
    );

    temp_dir.close()?;
    Ok(())
}
