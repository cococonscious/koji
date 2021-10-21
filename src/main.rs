mod commit_types;
mod config;
mod get_answers;

use crate::commit_types::{get_custom_commit_types, get_default_commit_types};
use crate::config::load_config;
use crate::get_answers::{
    get_amended_body, get_body, get_commit_type, get_has_open_issue, get_is_breaking_change,
    get_issue_reference, get_scope, get_summary, render_commit_type_choice,
};

use anyhow::Result;
use clap::{crate_authors, crate_version, App, Arg};
use cocogitto::CocoGitto;
use requestty::{prompt, Answers, Question};

const ARG_EMOJI: &str = "emoji";
const Q_COMMIT_TYPE: &str = "commit_type";
const Q_SCOPE: &str = "scope";
const Q_SUMMARY: &str = "summary";
const Q_BODY: &str = "body";
const Q_IS_BREAKING_CHANGE: &str = "is_breaking_change";
const Q_HAS_OPEN_ISSUE: &str = "has_open_issue";
const Q_ISSUE_REFERENCE: &str = "issue_reference";

fn main() -> Result<()> {
    let commit_types = if let Some(config) = load_config()? {
        get_custom_commit_types(config)
    } else {
        get_default_commit_types()
    };

    let matches = App::new("koji")
        .about("An interactive CLI for creating conventional commits.")
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name(ARG_EMOJI)
                .short("e")
                .long("emoji")
                .help("Prepend summary with relevant emoji based on commit type."),
        )
        .get_matches();

    let use_emoji = matches.is_present(ARG_EMOJI);

    let answers = prompt(vec![
        Question::select(Q_COMMIT_TYPE)
            .message("What type of change are you committing?")
            .page_size(8)
            .transform(|choice, _, backend| {
                write!(backend, "{}", choice.text.split(':').next().unwrap())
            })
            .choices(
                commit_types
                    .iter()
                    .map(|(_, t)| render_commit_type_choice(use_emoji, t, &commit_types)),
            )
            .build(),
        Question::input(Q_SCOPE)
            .message("What is the scope of this change? (press enter to skip)")
            .build(),
        Question::input(Q_SUMMARY)
            .message("Write a short, imperative tense description of the change.")
            .validate(|summary, _| {
                if !summary.is_empty() {
                    Ok(())
                } else {
                    Err("A description is required.".into())
                }
            })
            .build(),
        Question::input(Q_BODY)
            .message("Provide a longer description of the change. (press enter to skip)")
            .build(),
        Question::confirm(Q_IS_BREAKING_CHANGE)
            .message("Are there any breaking changes?")
            .build(),
        Question::confirm(Q_HAS_OPEN_ISSUE)
            .message("Does this change affect any open issues?")
            .build(),
        Question::input(Q_ISSUE_REFERENCE)
            .message("Add issue references. (e.g. \"fix #123\", \"re #123\")")
            .when(|answers: &Answers| match answers.get(Q_HAS_OPEN_ISSUE) {
                Some(a) => a.as_bool().unwrap(),
                None => false,
            })
            .validate(|issue_reference, _| {
                if !issue_reference.is_empty() {
                    Ok(())
                } else {
                    Err(
                        "An issue reference is required if this commit is related to an open issue."
                            .into(),
                    )
                }
            })
            .build(),
    ])?;

    let commit_type = get_commit_type(answers.get(Q_COMMIT_TYPE))?;
    let scope = get_scope(answers.get(Q_SCOPE))?;
    let summary = get_summary(
        answers.get(Q_SUMMARY),
        use_emoji,
        commit_type,
        &commit_types,
    )?;
    let body = get_body(answers.get(Q_BODY))?;
    let is_breaking_change = get_is_breaking_change(answers.get(Q_IS_BREAKING_CHANGE))?;
    let has_open_issue = get_has_open_issue(answers.get(Q_HAS_OPEN_ISSUE))?;
    let issue_reference = get_issue_reference(answers.get(Q_ISSUE_REFERENCE), has_open_issue)?;
    let body = get_amended_body(&body, &issue_reference);

    let cocogitto = CocoGitto::get()?;

    cocogitto.conventional_commit(commit_type, scope, summary, body, None, is_breaking_change)?;

    Ok(())
}
