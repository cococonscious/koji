use std::fs::{read_to_string, File};
use std::io::Write;

use anyhow::Result;
use clap::Parser;
use cocogitto::CocoGitto;

use conventional_commit_parser::parse;
use git2::Repository;
use koji::answers::{get_extracted_answers, ExtractedAnswers};
use koji::commit_types::get_commit_types;
use koji::config::load_config;
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
    let repo = Repository::discover(&std::env::current_dir()?)?;

    // Get CLI args.
    let Args {
        config,
        emoji,
        no_emoji,
        autocomplete,
        no_autocomplete,
        hook,
    } = Args::parse();

    // Get existing commit message (passed in via `-m`)
    let commit_editmsg = repo.path().join("COMMIT_EDITMSG");
    let message = match read_to_string(commit_editmsg) {
        Ok(contents) => contents.lines().next().unwrap_or("").to_string(),
        Err(_) => "".to_string(),
    };

    // If the existing message is already in the form of a conventional commit,
    // just go ahead and return early.
    if hook && parse(&message).is_ok() {
        return Ok(());
    }

    // Load config.
    let config = load_config(config)?;

    // Use emoji if set in config, or passed in via `-e`, and `--no-emoji` wasn't passed in.
    let emoji = config.emoji.unwrap_or(emoji) && !no_emoji || emoji;

    // Use autocomplete if set in config, or passed in via `-e`, and `--no-autocomplete` wasn't passed in.
    let autocomplete =
        config.autocomplete.unwrap_or(autocomplete) && !no_autocomplete || autocomplete;

    // Get commit types from config.
    let commit_types = get_commit_types(&config);

    // Get answers from interactive prompt.
    let answers = create_prompt(&repo, message, emoji, autocomplete, &commit_types)?;

    // Get data necessary for a conventional commit.
    let ExtractedAnswers {
        commit_type,
        scope,
        summary,
        body,
        is_breaking_change,
    } = get_extracted_answers(&answers, emoji, &commit_types)?;

    if hook {
        // Output the commit message to `.git/COMMIT_EDITMSG`.
        let message = CocoGitto::get_conventional_message(
            &commit_type,
            scope,
            summary,
            body,
            None,
            is_breaking_change,
        )?;

        let commit_editmsg = repo.path().join("COMMIT_EDITMSG");
        let mut file = File::create(commit_editmsg)?;

        file.write_all(message.as_bytes())?;
    } else {
        // Create the commit.
        let cocogitto = CocoGitto::get()?;

        cocogitto.conventional_commit(
            &commit_type,
            scope,
            summary,
            body,
            None,
            is_breaking_change,
        )?;
    }

    Ok(())
}
