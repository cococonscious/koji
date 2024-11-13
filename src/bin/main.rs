use std::fs::read_to_string;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use cocogitto::command::commit::CommitOptions;
use conventional_commit_parser::parse;
use git2::Repository;
use koji::answers::{get_extracted_answers, ExtractedAnswers};
use koji::commit::{commit, write_commit_msg};
use koji::config::{Config, ConfigArgs};
use koji::questions::create_prompt;

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
    config: Option<String>,

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
        help = "Run as a git hook, writing the commit message to COMMIT_EDITMSG instead of committing"
    )]
    hook: bool,

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
        issues,
        sign,
        all,
    } = Args::parse();

    if command.is_some() {
        match command.unwrap() {
            SubCmds::Completions { shell } => {
                shell.generate(&mut Args::command(), &mut std::io::stdout());
            }
        }
        return Ok(());
    }

    // Find repo
    let repo =
        Repository::discover(std::env::current_dir()?).context("could not find git repository")?;

    // Get existing commit message (passed in via `-m`)
    let commit_editmsg = repo.path().join("COMMIT_EDITMSG");
    let commit_message = match read_to_string(commit_editmsg) {
        Ok(contents) => contents.lines().next().unwrap_or("").to_string(),
        Err(_) => "".to_string(),
    };

    // If the existing message is already in the form of a conventional commit,
    // just go ahead and return early
    if hook && parse(&commit_message).is_ok() {
        return Ok(());
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
        _current_dir: None,
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
    } = get_extracted_answers(answers, &config)?;

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

        commit(options)?;
    }

    Ok(())
}
