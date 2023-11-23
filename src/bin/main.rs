use std::fs::read_to_string;

use anyhow::{Context, Result};
use clap::Parser;

use conventional_commit_parser::parse;
use git2::Repository;
use koji::answers::{get_extracted_answers, ExtractedAnswers};
use koji::commit::{commit, write_commit_msg};
use koji::config::Config;
use koji::questions::create_prompt;

#[derive(Parser, Debug)]
#[command(
    about = "🦊 An interactive CLI for creating conventional commits.",
    version,
    author
)]
struct Args {
    #[arg(
        short,
        long,
        help = "Enables autocomplete for scope prompt via scanning commit history"
    )]
    autocomplete: Option<bool>,

    #[arg(short, long, help = "Enables breaking change prompt")]
    breaking_changes: Option<bool>,

    #[arg(
        short,
        long,
        help = "Path to a config file containing custom commit types"
    )]
    config: Option<String>,

    #[arg(
        short,
        long,
        help = "Prepend the commit summary with relevant emoji based on commit type"
    )]
    emoji: Option<bool>,

    #[arg(
        long,
        help = "Run as a git hook, writing the commit message to COMMIT_EDITMSG instead of committing"
    )]
    hook: bool,

    #[arg(
        short,
        long,
        help = "Enables issue prompt, which will append a reference to an issue in the commit body"
    )]
    issues: Option<bool>,
}

fn main() -> Result<()> {
    // Get CLI args
    let Args {
        autocomplete,
        breaking_changes,
        config,
        emoji,
        hook,
        issues,
    } = Args::parse();

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
    let config = Config::new(config)?;

    // Use value of `autocomplete` if passed in, otherwise use value from config
    let autocomplete = autocomplete.unwrap_or(config.autocomplete.unwrap_or(false));

    // Use value of `breaking_changes` if passed in, otherwise use value from config
    let breaking_changes = breaking_changes.unwrap_or(config.breaking_changes.unwrap_or(true));

    // Use value of `emoji` if passed in, otherwise use value from config
    let emoji = emoji.unwrap_or(config.emoji.unwrap_or(false));

    // Use value of `issues` if passed in, otherwise use value from config
    let issues = issues.unwrap_or(config.issues.unwrap_or(true));

    // Get answers from interactive prompt
    let answers = create_prompt(
        &repo,
        commit_message,
        emoji,
        autocomplete,
        breaking_changes,
        issues,
        &config.commit_types,
    )?;

    // Get data necessary for a conventional commit
    let ExtractedAnswers {
        commit_type,
        scope,
        summary,
        body,
        is_breaking_change,
    } = get_extracted_answers(&answers, emoji, &config.commit_types)?;

    // Do the thing!
    if hook {
        write_commit_msg(repo, commit_type, scope, summary, body, is_breaking_change)?;
    } else {
        commit(commit_type, scope, summary, body, is_breaking_change)?;
    }

    Ok(())
}
