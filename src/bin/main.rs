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
#[clap(
    about = "🦊 An interactive CLI for creating conventional commits.",
    version,
    author
)]
struct Args {
    #[clap(
        short,
        long,
        help = "Path to a config file containing custom commit types"
    )]
    config: Option<String>,

    #[clap(
        short,
        long,
        help = "Prepend the commit summary with relevant emoji based on commit type"
    )]
    emoji: bool,

    #[clap(long, help = "Bypass the emoji flag")]
    no_emoji: bool,

    #[clap(
        short,
        long,
        help = "Enables auto-complete for scope prompt via scanning commit history"
    )]
    autocomplete: bool,

    #[clap(long, help = "Bypass the autocopmlete flag")]
    no_autocomplete: bool,

    #[clap(
        long,
        help = "Run as a git hook, writing the commit message to COMMIT_EDITMSG instead of committing"
    )]
    hook: bool,
}

fn main() -> Result<()> {
    // Get CLI args
    let Args {
        config,
        emoji,
        no_emoji,
        autocomplete,
        no_autocomplete,
        hook,
    } = Args::parse();

    // Find repo
    let repo =
        Repository::discover(&std::env::current_dir()?).context("could not find git repository")?;

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

    // Use emoji if set in config, or passed in via `-e`, and `--no-emoji` wasn't passed in
    let emoji = config.emoji.unwrap_or(emoji) && !no_emoji || emoji;

    // Use autocomplete if set in config, or passed in via `-a`, and `--no-autocomplete` wasn't passed in
    let autocomplete =
        config.autocomplete.unwrap_or(autocomplete) && !no_autocomplete || autocomplete;

    // Get answers from interactive prompt
    let answers = create_prompt(
        &repo,
        commit_message,
        emoji,
        autocomplete,
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
