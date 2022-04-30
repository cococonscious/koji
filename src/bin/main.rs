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

    #[clap(
        short,
        long,
        help = "Enables auto-complete for scope prompt via scanning commit history"
    )]
    autocomplete: bool,

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
        config: config_path,
        emoji: use_emoji,
        autocomplete: use_autocomplete,
        hook: as_hook,
    } = Args::parse();

    // Get existing commit message (passed in via `-m`)
    let commit_editmsg = repo.path().join("COMMIT_EDITMSG");
    let message = match read_to_string(commit_editmsg) {
        Ok(contents) => contents.lines().next().unwrap_or("").to_string(),
        Err(_) => "".to_string(),
    };

    // If the existing message is already in the form of a conventional commit,
    // just go ahead and return early.
    if let Ok(_) = parse(&message) {
        return Ok(());
    }

    // Load config if available and get commit types.
    let config = load_config(config_path)?;
    let commit_types = get_commit_types(&config);

    // Get answers from interactive prompt.
    let answers = create_prompt(&repo, message, use_emoji, use_autocomplete, &commit_types)?;

    // Get data necessary for a conventional commit.
    let ExtractedAnswers {
        commit_type,
        scope,
        summary,
        body,
        is_breaking_change,
    } = get_extracted_answers(&answers, use_emoji, &commit_types)?;

    if as_hook {
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
