use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use cocogitto::command::commit::CommitOptions;
use conventional_commit_parser::parse;
use koji::answers::{get_extracted_answers, ExtractedAnswers};
use koji::commit::{commit, generate_commit_msg};
use koji::config::{Config, ConfigArgs};
use koji::questions::{create_prompt, prompt_confirm, PreviousAnswers};
use koji::status::StagingStatus;
use koji::vcs::VcsBackend;

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

    let backend = VcsBackend::detect_with_hint(&current_dir, config.vcs)
        .context("could not find a supported repository (git or jj)")?;

    if hook && !backend.supports_hooks() {
        anyhow::bail!("--hook mode is not supported with jj repositories (jj has no commit hooks)");
    }

    let commit_message = if hook {
        backend
            .read_current_description()?
            .map(|c| c.lines().next().unwrap_or("").to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };

    // For jj, parse the existing description to pre-populate prompts
    let previous_answers = if !hook && backend.is_jj() {
        backend
            .read_current_description()?
            .and_then(|desc| PreviousAnswers::from_description(&desc))
    } else {
        None
    };

    if hook && parse(&commit_message).is_ok() {
        return Ok(());
    }

    // --hook and --stdout don't create commits; --all stages tracked files automatically
    if !hook && !stdout && !all {
        match backend.check_staging()? {
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

    let answers = create_prompt(previous_answers, &config, &backend)?;

    let ExtractedAnswers {
        body,
        commit_type,
        is_breaking_change,
        scope,
        summary,
    } = get_extracted_answers(answers, config.emoji, &config.commit_types)?;

    let message = generate_commit_msg(
        commit_type.clone(),
        scope.clone(),
        summary.clone(),
        body.clone(),
        is_breaking_change,
    )?;

    if stdout {
        println!("{message}");
    } else {
        eprintln!("\n{message}\n");
    }

    if stdout {
        return Ok(());
    }

    if !yes && !prompt_confirm()? {
        eprintln!("Commit aborted.");
        return Ok(());
    }

    // Do the thing!
    if hook || backend.is_jj() {
        backend.write_commit_msg(commit_type, scope, summary, body, is_breaking_change)?;
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
