use git2::{Commit, IndexAddOption, Oid, Repository, RepositoryInitOptions};
#[cfg(not(target_os = "windows"))]
use rexpect::{
    process::wait,
    session::{spawn_command, PtySession},
};
use std::{error::Error, fs, path::PathBuf, process::Command};
use tempfile::TempDir;

fn setup_config_home() -> Result<TempDir, Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

    Ok(temp_dir)
}

fn setup_test_dir() -> Result<(PathBuf, TempDir, Repository), Box<dyn Error>> {
    let bin_path = assert_cmd::cargo::cargo_bin("koji");
    let temp_dir = tempfile::tempdir()?;
    let mut init_options = RepositoryInitOptions::new();
    init_options.initial_head("main");
    let repo = Repository::init_opts(&temp_dir, &init_options)?;

    let mut gitconfig = repo.config()?;
    gitconfig.set_str("user.name", "test")?;
    gitconfig.set_str("user.email", "test@example.org")?;

    Ok((bin_path, temp_dir, repo))
}

fn get_last_commit(repo: &Repository) -> Result<Commit, git2::Error> {
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    let oid = walk.next().expect("cannot get commit in revwalk")?;

    repo.find_commit(oid)
}

fn do_initial_commit(repo: &Repository, message: &'static str) -> Result<Oid, git2::Error> {
    let signature = repo.signature()?;
    let oid = repo.index()?.write_tree()?;
    let tree = repo.find_tree(oid)?;

    repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])
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
    let config_temp_dir = setup_config_home()?;

    fs::write(temp_dir.path().join("README.md"), "foo")?;
    repo.index()?
        .add_all(["."].iter(), IndexAddOption::default(), None)?;
    do_initial_commit(&repo, "docs(readme): initial draft")?;

    fs::write(temp_dir.path().join("config.json"), "bar")?;
    // TODO properly test "-a"
    repo.index()?
        .add_all(["."].iter(), IndexAddOption::default(), None)?;
    repo.index()?.write()?;

    let mut cmd = Command::new(bin_path);
    cmd.env("NO_COLOR", "1")
        .arg("-C")
        .arg(temp_dir.path())
        .arg("-a")
        .arg("--autocomplete=true");

    let mut process = spawn_command(cmd, Some(5000))?;

    process.expect_commit_type()?;
    process.send_line("feat")?;
    process.flush()?;
    process.expect_scope()?;
    process.send_line("config")?;
    process.flush()?;
    process.expect_summary()?;
    process.send_line("refactor config pairs")?;
    process.flush()?;
    process.expect_body()?;
    process
        .send_line("Removed and added a config pair each\\nNecessary for future compatibility.")?;
    process.expect_breaking()?;
    process.send_line("Y")?;
    process.flush()?;
    process.expect_breaking_details()?;
    process.send_line("Something can't be configured anymore")?;
    process.flush()?;
    process.expect_issues()?;
    process.send_line("Y")?;
    process.flush()?;
    process.expect_issues_details()?;
    process.send_line("closes #1")?;
    process.flush()?;
    let eof_output = process.exp_eof();

    let exitcode = process.process.wait()?;
    let success = matches!(exitcode, wait::WaitStatus::Exited(_, 0));

    if !success {
        panic!("Command exited non-zero, end of output: {:?}", eof_output);
    }

    let commit = get_last_commit(&repo)?;
    assert_eq!(
        commit.summary(),
        Some("feat(config)!: refactor config pairs")
    );
    assert_eq!(
        commit.body(),
        Some("Removed and added a config pair each\nNecessary for future compatibility.\n\ncloses #1\nBREAKING CHANGE: Something can't be configured anymore")
    );

    temp_dir.close()?;
    config_temp_dir.close()?;
    Ok(())
}

#[test]
#[cfg(not(target_os = "windows"))]
fn test_hook_correct() -> Result<(), Box<dyn Error>> {
    let (bin_path, temp_dir, repo) = setup_test_dir()?;
    let config_temp_dir = setup_config_home()?;

    fs::write(temp_dir.path().join("config.json"), "abc")?;
    repo.index()?
        .add_all(["*"].iter(), IndexAddOption::default(), None)?;
    repo.index()?.write()?;
    fs::remove_file(temp_dir.path().join(".git").join("COMMIT_EDITMSG")).unwrap_or(());

    let mut cmd = Command::new(bin_path);
    cmd.env("NO_COLOR", "1")
        .arg("-C")
        .arg(temp_dir.path())
        .arg("--hook")
        .arg("--autocomplete=true");

    let mut process = spawn_command(cmd, Some(5000))?;

    process.expect_commit_type()?;
    process.send_line("fix")?;
    process.flush()?;
    process.expect_scope()?;
    process.send_line("")?;
    process.flush()?;
    process.expect_summary()?;
    process.send_line("some weird error")?;
    process.flush()?;
    process.expect_body()?;
    process.send_line("")?;
    process.flush()?;
    process.expect_breaking()?;
    process.send_line("N")?;
    process.flush()?;
    process.expect_issues()?;
    process.send_line("N")?;
    process.flush()?;
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
    config_temp_dir.close()?;
    Ok(())
}

#[test]
#[cfg(not(target_os = "windows"))]
fn test_stdout_correct() -> Result<(), Box<dyn Error>> {
    let (bin_path, temp_dir, repo) = setup_test_dir()?;
    let config_temp_dir = setup_config_home()?;

    fs::write(temp_dir.path().join("config.json"), "abc")?;
    repo.index()?
        .add_all(["*"].iter(), IndexAddOption::default(), None)?;
    repo.index()?.write()?;
    fs::remove_file(temp_dir.path().join(".git").join("COMMIT_EDITMSG")).unwrap_or(());

    let mut cmd = Command::new(bin_path);
    cmd.env("NO_COLOR", "1")
        .arg("-C")
        .arg(temp_dir.path())
        .arg("--stdout")
        .arg("--autocomplete=true");

    let mut process = spawn_command(cmd, Some(5000))?;

    process.expect_commit_type()?;
    process.send_line("fix")?;
    process.flush()?;
    process.expect_scope()?;
    process.send_line("")?;
    process.flush()?;
    process.expect_summary()?;
    process.send_line("some weird error")?;
    process.flush()?;
    process.expect_body()?;
    process.send_line("")?;
    process.flush()?;
    process.expect_breaking()?;
    process.send_line("N")?;
    process.flush()?;
    process.expect_issues()?;
    process.send_line("N")?;
    process.flush()?;

    let expected_output = "fix: some weird error";

    let _ = process
        .exp_string(expected_output)
        .expect("failed to match output");

    let eof_output = process.exp_eof();

    let exitcode = process.process.wait()?;
    let success = matches!(exitcode, wait::WaitStatus::Exited(_, 0));

    if !success {
        panic!("Command exited non-zero, end of output: {:?}", eof_output);
    }

    let editmsg = temp_dir.path().join(".git").join("COMMIT_EDITMSG");
    assert!(!editmsg.exists());

    temp_dir.close()?;
    config_temp_dir.close()?;
    Ok(())
}

#[test]
#[cfg(not(target_os = "windows"))]
fn test_empty_breaking_text_correct() -> Result<(), Box<dyn Error>> {
    let (bin_path, temp_dir, repo) = setup_test_dir()?;
    let config_temp_dir = setup_config_home()?;

    fs::write(temp_dir.path().join("Cargo.toml"), "bar")?;
    repo.index()?
        .add_all(["."].iter(), IndexAddOption::default(), None)?;
    repo.index()?.write()?;

    let mut cmd = Command::new(bin_path);
    cmd.env("NO_COLOR", "1")
        .arg("-C")
        .arg(temp_dir.path())
        .arg("-a")
        .arg("--autocomplete=true");

    let mut process = spawn_command(cmd, Some(5000))?;

    process.expect_commit_type()?;
    process.send_line("docs")?;
    process.flush()?;
    process.expect_scope()?;
    process.send_line("cargo")?;
    process.flush()?;
    process.expect_summary()?;
    process.send_line("rename project")?;
    process.flush()?;
    process.expect_body()?;
    process.send_line("Renamed the project to a new name.")?;
    process.expect_breaking()?;
    process.send_line("Y")?;
    process.flush()?;
    process.expect_breaking_details()?;
    // `^[` is the same as <esc>
    process.send_control('[')?;
    process.flush()?;
    process.expect_issues()?;
    process.send_line("N")?;
    process.flush()?;
    let eof_output = process.exp_eof();

    let exitcode = process.process.wait()?;
    let success = matches!(exitcode, wait::WaitStatus::Exited(_, 0));

    if !success {
        panic!("Command exited non-zero, end of output: {:?}", eof_output);
    }

    let commit = get_last_commit(&repo)?;
    assert_eq!(commit.summary(), Some("docs(cargo)!: rename project"));
    assert_eq!(commit.body(), Some("Renamed the project to a new name."));

    temp_dir.close()?;
    config_temp_dir.close()?;
    Ok(())
}

#[test]
fn test_non_repository_error() -> Result<(), Box<dyn Error>> {
    let bin_path = assert_cmd::cargo::cargo_bin("koji");
    let temp_dir = tempfile::tempdir()?;

    let mut cmd = Command::new(bin_path);
    cmd.arg("-C").arg(temp_dir.path());

    let cmd_out = cmd.output()?;
    let stderr_out = String::from_utf8(cmd_out.stderr)?;

    assert!(!cmd_out.status.success());
    assert!(stderr_out.contains("could not find git repository"));

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_empty_repository_error() -> Result<(), Box<dyn Error>> {
    let (bin_path, temp_dir, _) = setup_test_dir()?;

    let mut cmd = Command::new(bin_path);
    cmd.arg("-C").arg(temp_dir.path());

    let mut process = spawn_command(cmd, Some(5000))?;

    process.expect_commit_type()?;
    process.send_line("chore")?;
    process.flush()?;
    process.expect_scope()?;
    process.send_line("")?;
    process.flush()?;
    process.expect_summary()?;
    process.send_line("new eslint config")?;
    process.flush()?;
    process.expect_body()?;
    process.send_line("")?;
    process.flush()?;
    process.expect_breaking()?;
    process.send_line("N")?;
    process.flush()?;
    process.expect_issues()?;
    process.send_line("N")?;
    process.flush()?;
    let eof_output = process.exp_eof();

    let exitcode = process.process.wait()?;
    let success = matches!(exitcode, wait::WaitStatus::Exited(_, 0));

    assert!(!success);
    assert!(eof_output?.contains("nothing to commit"));

    temp_dir.close()?;
    Ok(())
}

#[test]
fn test_all_hook_exclusive_error() -> Result<(), Box<dyn Error>> {
    let bin_path = assert_cmd::cargo::cargo_bin("koji");

    let mut cmd = Command::new(bin_path);
    cmd.arg("--hook");
    cmd.arg("--all");

    let cmd_out = cmd.output()?;
    let stderr_out = String::from_utf8(cmd_out.stderr)?;

    assert!(!cmd_out.status.success());
    assert!(stderr_out.contains("the argument '--hook' cannot be used with '--all'"));

    Ok(())
}

#[test]
fn test_all_stdout_exclusive_error() -> Result<(), Box<dyn Error>> {
    let bin_path = assert_cmd::cargo::cargo_bin("koji");

    let mut cmd = Command::new(bin_path);
    cmd.arg("--stdout");
    cmd.arg("--all");

    let cmd_out = cmd.output()?;
    let stderr_out = String::from_utf8(cmd_out.stderr)?;

    assert!(!cmd_out.status.success());
    assert!(stderr_out.contains("the argument '--stdout' cannot be used with '--all'"));

    Ok(())
}

#[test]
fn test_hook_stdout_exclusive_error() -> Result<(), Box<dyn Error>> {
    let bin_path = assert_cmd::cargo::cargo_bin("koji");

    let mut cmd = Command::new(bin_path);
    cmd.arg("--stdout");
    cmd.arg("--hook");

    let cmd_out = cmd.output()?;
    let stderr_out = String::from_utf8(cmd_out.stderr)?;

    assert!(!cmd_out.status.success());
    assert!(stderr_out.contains("the argument '--stdout' cannot be used with '--hook'"));

    Ok(())
}

#[test]
fn test_completion_scripts_success() -> Result<(), Box<dyn Error>> {
    fn run_for(shell: &'static str, containing: &'static str) -> Result<(), Box<dyn Error>> {
        let bin_path = assert_cmd::cargo::cargo_bin("koji");

        let mut cmd = Command::new(bin_path);
        cmd.arg("completions").arg(shell);

        let cmd_out = cmd.output()?;
        let stdout = String::from_utf8(cmd_out.stdout)?;

        assert!(cmd_out.status.success());
        assert!(stdout.contains(containing));

        Ok(())
    }

    run_for("nushell", "def \"nu-complete koji")?;
    run_for("fish", "complete -c koji -n \"__fish_koji_needs_command")?;
    run_for("bash", "complete -F _koji -o bashdefault -o default koji")?;
    run_for(
        "powershell",
        "Register-ArgumentCompleter -Native -CommandName 'koji'",
    )?;
    run_for("zsh", "#compdef koji")
}

#[test]
fn test_xdg_config_on_macos() -> Result<(), Box<dyn Error>> {
    let (bin_path, temp_dir, repo) = setup_test_dir()?;
    let config_temp_dir = setup_config_home()?;

    fs::write(temp_dir.path().join("README.md"), "foo")?;
    repo.index()?
        .add_all(["."].iter(), IndexAddOption::default(), None)?;
    do_initial_commit(&repo, "docs(readme): initial draft")?;

    fs::write(temp_dir.path().join("config.json"), "bar")?;
    // TODO properly test "-a"
    repo.index()?
        .add_all(["."].iter(), IndexAddOption::default(), None)?;
    repo.index()?.write()?;

    let mut config_dir = std::env::current_dir()?;
    config_dir.push("meta");

    let mut cmd = Command::new(bin_path);
    cmd.env("NO_COLOR", "1")
        .env("XDG_CONFIG_HOME", config_dir.as_path().to_str().unwrap())
        .arg("-C")
        .arg(temp_dir.path())
        .arg("--stdout")
        .arg("--autocomplete=true");

    let mut process = spawn_command(cmd, Some(5000))?;

    process.expect_commit_type()?;
    process.send_line("wip")?;
    process.flush()?;
    process.expect_scope()?;
    process.send_line("")?;
    process.flush()?;
    process.expect_summary()?;
    process.send_line("some weird error")?;
    process.flush()?;
    process.expect_body()?;
    process.send_line("")?;
    process.flush()?;
    process.expect_breaking()?;
    process.send_line("N")?;
    process.flush()?;
    process.expect_issues()?;
    process.send_line("N")?;
    process.flush()?;

    let expected_output = "wip: some weird error";

    let _ = process
        .exp_string(expected_output)
        .expect("failed to match output");

    let eof_output = process.exp_eof();

    let exitcode = process.process.wait()?;
    let success = matches!(exitcode, wait::WaitStatus::Exited(_, 0));

    if !success {
        panic!("Command exited non-zero, end of output: {:?}", eof_output);
    }

    let editmsg = temp_dir.path().join(".git").join("COMMIT_EDITMSG");
    assert!(!editmsg.exists());

    temp_dir.close()?;
    config_temp_dir.close()?;
    Ok(())
}
