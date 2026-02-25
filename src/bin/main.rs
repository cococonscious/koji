use std::fs::read_to_string;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use cocogitto::command::commit::CommitOptions;
use conventional_commit_parser::parse;
use koji::answers::{get_extracted_answers, ExtractedAnswers};
use koji::commit::{commit, generate_commit_msg, write_commit_msg};
use koji::config::{Config, ConfigArgs};
use koji::questions::{create_prompt, prompt_confirm};
use koji::status::{check_staging, StagingStatus};

#[derive(Parser, Debug)]
#[command(
    about = "🦊 An interactive CLI for creating conventional commits.",
    version
)]
struct Args {
    #[command(subcommand)]
    command: Option<SubCmds>,

    #[arg(
        long,
        default_missing_value = "true",
        num_args = 0..=1,
        value_name = "ENABLE",
        help_heading = Some("Configuration"),
        help = "Enables autocomplete for scope prompt via scanning commit history"
    )]
    autocomplete: Option<bool>,

    #[arg(
        long,
        default_missing_value = "true",
        num_args = 0..=1,
        value_name = "ENABLE",
        help_heading = Some("Configuration"),
        help = "Enables breaking change prompts, one of them for adding the BREAKING CHANGE footer"
    )]
    breaking_changes: Option<bool>,

    #[arg(
        short,
        long,
        value_name = "FILE",
        help_heading = Some("Configuration"),
        help = "Path to a custom config file"
    )]
    config: Option<PathBuf>,

    #[arg(
        long,
        default_missing_value = "true",
        num_args = 0..=1,
        value_name = "ENABLE",
        help_heading = Some("Configuration"),
        help = "Prepend the commit summary with relevant emoji based on commit type"
    )]
    emoji: Option<bool>,

    #[arg(
        long,
        help = "Run as a git hook, writing the commit message to COMMIT_EDITMSG instead of committing",
        conflicts_with = "all"
    )]
    hook: bool,

    #[arg(
        long,
        help = "Outputs the commit message to stdout instead of committing",
        conflicts_with = "all",
        conflicts_with = "hook"
    )]
    stdout: bool,

    #[arg(
        long,
        default_missing_value = "true",
        num_args = 0..=1,
        value_name = "ENABLE",
        help_heading = Some("Configuration"),
        help = "Enables issue prompts, to add a footer for issue references"
    )]
    issues: Option<bool>,

    #[arg(
        short = 'S',
        long,
        default_missing_value = "true",
        num_args = 0..=1,
        value_name = "ENABLE",
        help_heading = Some("Configuration"),
        help = "Sign the commit using the user's GPG key, if one is configured"
    )]
    sign: Option<bool>,

    #[arg(short, long, help = "Stage all tracked modified or deleted files")]
    all: bool,

    #[arg(
        short = 'y',
        long,
        help = "Skip the confirmation prompt and proceed with the commit"
    )]
    yes: bool,

    #[arg(
        short = 'C',
        value_name = "PATH",
        help = "Run as if koji was started in <path>"
    )]
    current_workdir: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum SubCmds {
    #[command(about = "Generate shell completions")]
    Completions { shell: clap_complete_command::Shell },
}

#[cfg(not(tarpaulin_include))]
fn main() -> Result<()> {
    // Get CLI args

    let Args {
        command,
        autocomplete,
        breaking_changes,
        config,
        emoji,
        hook,
        stdout,
        issues,
        sign,
        all,
        yes,
        current_workdir,
    } = Args::parse();

    if let Some(command) = command {
        match command {
            SubCmds::Completions { shell } => {
                shell.generate(&mut Args::command(), &mut std::io::stdout());
            }
        }

        return Ok(());
    }

    let current_dir = match current_workdir {
        Some(dir) => dir,
        None => std::env::current_dir()?,
    };

    // Find repo
    let repo = gix::discover(&current_dir).context("could not find git repository")?;

    // Get existing commit message (passed in via `-m`)
    let commit_editmsg = repo.path().join("COMMIT_EDITMSG");
    let commit_message = match read_to_string(commit_editmsg) {
        Ok(contents) => contents.lines().next().unwrap_or("").to_string(),
        Err(_) => "".to_string(),
    };

    if hook && parse(&commit_message).is_ok() {
        return Ok(());
    }

    // --hook and --stdout don't create commits; --all stages tracked files automatically
    if !hook && !stdout && !all {
        match check_staging(&repo)? {
            StagingStatus::Empty => {
                anyhow::bail!("no files staged for commit");
            }
            StagingStatus::Partial { staged, unstaged } => {
                eprintln!(
                    "Warning: {staged} file(s) staged for commit, \
                     {unstaged} file(s) with unstaged changes not included\n"
                );
            }
            StagingStatus::Ready { .. } => {}
        }
    }

    // Load config
    let config = Config::new(Some(ConfigArgs {
        autocomplete,
        breaking_changes,
        emoji,
        issues,
        path: config,
        sign,
        _user_config_path: None,
        _current_dir: Some(current_dir.clone()),
    }))?;

    // Get answers from interactive prompt
    let answers = create_prompt(commit_message, &config)?;

    // Get data necessary for a conventional commit
    let ExtractedAnswers {
        body,
        commit_type,
        is_breaking_change,
        scope,
        summary,
    } = get_extracted_answers(answers, config.emoji, &config.commit_types)?;

    // Generate the commit message
    let message = generate_commit_msg(
        commit_type.clone(),
        scope.clone(),
        summary.clone(),
        body.clone(),
        is_breaking_change,
    )?;

    // Print the commit message preview
    if stdout {
        println!("{message}");
    } else {
        eprintln!("\n{message}\n");
    }

    // --stdout just prints the message without committing
    if stdout {
        return Ok(());
    }

    // Prompt for confirmation unless --yes is set
    if !yes && !prompt_confirm()? {
        eprintln!("Commit aborted.");
        return Ok(());
    }

    // Do the thing!
    if hook {
        write_commit_msg(&repo, commit_type, scope, summary, body, is_breaking_change)?;
    } else {
        let options = CommitOptions {
            commit_type: commit_type.as_str(),
            scope,
            summary,
            body,
            footer: None,
            breaking: is_breaking_change,
            sign: config.sign,
            add_files: all,
            update_files: false,
        };

        commit(current_dir, options)?;
    }

    Ok(())
}
