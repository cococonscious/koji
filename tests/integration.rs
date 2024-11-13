use git2::Repository;
#[cfg(not(target_os = "windows"))]
use rexpect::session::spawn_command;
use std::{error::Error, fs, path::PathBuf, process::Command};
use tempfile::TempDir;

fn setup_test_dir() -> Result<(PathBuf, TempDir, Repository), Box<dyn Error>> {
    let bin_path = assert_cmd::cargo::cargo_bin("koji");
    let temp_dir = tempfile::tempdir()?;
    let repo = Repository::init(&temp_dir)?;

    Ok((bin_path, temp_dir, repo))
}

#[ignore]
#[test]
#[cfg(not(target_os = "windows"))]
fn test_type_scope_summary_body_breaking_issue_add_files_correct() -> Result<(), Box<dyn Error>> {
    let (bin_path, temp_dir, _) = setup_test_dir()?;

    fs::write(temp_dir.path().join("config.json"), "abc")?;

    let mut cmd = Command::new(bin_path);
    cmd.env("NO_COLOR", "1").arg("-C").arg(temp_dir.path());

    cmd.arg("-a");

    let mut process = spawn_command(cmd, Some(10000))?;

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
    println!("{:?}", process.exp_eof());

    // TODO check if commit was created

    temp_dir.close()?;
    Ok(())
}
