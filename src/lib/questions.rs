use anyhow::{Context, Result};
use conventional_commit_parser::parse_summary;
use git2::Repository;
use linked_hash_map::LinkedHashMap;
use requestty::{
    prompt,
    question::{completions, Completions},
    Answers, Question,
};

use crate::{commit_types::CommitType, emoji::ReplaceEmoji};

/// These exist just so I don't make a typo when using them.
pub const Q_COMMIT_TYPE: &str = "commit_type";
pub const Q_SCOPE: &str = "scope";
pub const Q_SUMMARY: &str = "summary";
pub const Q_BODY: &str = "body";
pub const Q_IS_BREAKING_CHANGE: &str = "is_breaking_change";
pub const Q_HAS_OPEN_ISSUE: &str = "has_open_issue";
pub const Q_ISSUE_REFERENCE: &str = "issue_reference";

/// Get a unique list of existing scopes in the commit history.
fn get_existing_scopes() -> Result<Completions<String>> {
    let repo = Repository::discover(&std::env::current_dir()?)?;

    let mut walk = repo.revwalk()?;

    walk.push_head()?;
    walk.set_sorting(git2::Sort::TIME)?;

    let mut scopes: Completions<String> = Completions::new();

    for id in walk {
        if let Some(summary) = repo.find_commit(id?)?.summary() {
            // We want to throw away any error from `parse_summary` since an
            // invalid commit message should just be ignored.
            if let Ok(parsed) = parse_summary(summary) {
                let scope = parsed.scope;

                if let Some(scope) = scope {
                    if !scopes.contains(&scope) {
                        scopes.push(scope)
                    }
                }
            }
        }
    }

    Ok(scopes)
}

/// Transform commit type choice.
fn transform_commit_type_choice(choice: &str) -> String {
    choice.split(':').next().unwrap().into()
}

/// Format the commit type choices.
fn format_commit_type_choice(
    use_emoji: bool,
    commit_type: &CommitType,
    commit_types: &LinkedHashMap<String, CommitType>,
) -> String {
    let name = &commit_type.name;
    let description = &commit_type.description;
    let use_emoji = use_emoji && commit_type.emoji.is_some();

    let emoji = if use_emoji {
        if let Some(emoji) = &commit_type.emoji {
            format!("{emoji} ")
        } else {
            "".into()
        }
    } else {
        "".into()
    };

    let width = commit_types
        .iter()
        .map(|(key, _)| key.chars().count())
        .max()
        .unwrap()
        - commit_type.name.chars().count()
        + if use_emoji { 5 } else { 3 };

    format!("{name}:{emoji:>width$}{description}")
}

/// Validate summary.
fn validate_summary(summary: &str) -> Result<(), String> {
    if !summary.is_empty() {
        Ok(())
    } else {
        Err("A summary is required.".into())
    }
}

/// Validate issue reference.
fn validate_issue_reference(issue_reference: &str) -> Result<(), String> {
    if !issue_reference.is_empty() {
        Ok(())
    } else {
        Err("An issue reference is required if this commit is related to an open issue.".into())
    }
}

/// Create the interactive prompt.
pub fn create_prompt(
    use_emoji: bool,
    use_autocomplete: bool,
    commit_types: &LinkedHashMap<String, CommitType>,
) -> Result<Answers> {
    let scopes = if use_autocomplete {
        get_existing_scopes()?
    } else {
        completions![]
    };

    prompt(vec![
        Question::select(Q_COMMIT_TYPE)
            .message("What type of change are you committing?")
            .page_size(8)
            .transform(|choice, _, backend| {
                write!(backend, "{}", transform_commit_type_choice(&choice.text))
            })
            .choices(
                commit_types
                    .iter()
                    .map(|(_, choice)| format_commit_type_choice(use_emoji, choice, commit_types)),
            )
            .build(),
        Question::input(Q_SCOPE)
            .message("What is the scope of this change? (press enter to skip)")
            .transform(|scope, _, backend| write!(backend, "{}", scope.replace_emoji_shortcodes()))
            .auto_complete(|scope, _| {
                if scopes.is_empty() {
                    completions![scope]
                } else {
                    scopes.to_owned()
                }
            })
            .build(),
        Question::input(Q_SUMMARY)
            .message("Write a short, imperative tense description of the change.")
            .transform(|summary, _, backend| {
                write!(backend, "{}", summary.replace_emoji_shortcodes())
            })
            .validate(|summary, _| validate_summary(summary))
            .build(),
        Question::input(Q_BODY)
            .message("Provide a longer description of the change. (press enter to skip)")
            .transform(|body, _, backend| write!(backend, "{}", body.replace_emoji_shortcodes()))
            .build(),
        Question::confirm(Q_IS_BREAKING_CHANGE)
            .message("Are there any breaking changes?")
            .default(false)
            .build(),
        Question::confirm(Q_HAS_OPEN_ISSUE)
            .message("Does this change affect any open issues?")
            .default(false)
            .build(),
        Question::input(Q_ISSUE_REFERENCE)
            .message("Add issue references. (e.g. \"fix #123\", \"re #123\")")
            .when(|answers: &Answers| match answers.get(Q_HAS_OPEN_ISSUE) {
                Some(has_open_issue) => has_open_issue.as_bool().unwrap(),
                None => false,
            })
            .validate(|issue_reference, _| validate_issue_reference(issue_reference))
            .build(),
    ])
    .context("could not get answers from prompt")
}

#[cfg(test)]
mod tests {
    use crate::{commit_types::get_commit_types, config::load_config};

    use super::*;

    #[test]
    fn test_transform_commit_type_choice() {
        let transformed = transform_commit_type_choice(
            "feat:       A code change that neither fixes a bug nor adds a feature",
        );

        assert_eq!(transformed, "feat");
    }

    #[test]
    fn test_format_commit_type_choice() {
        let config = load_config(None).unwrap();
        let commit_types = get_commit_types(&config);

        let choice =
            format_commit_type_choice(false, commit_types.get("ci").unwrap(), &commit_types);

        assert_eq!(
            choice,
            "ci:         Changes to our CI configuration files and scripts"
        );
    }

    #[test]
    fn test_render_commit_type_choice_with_emoji() {
        let config = load_config(None).unwrap();
        let commit_types = get_commit_types(&config);

        let choice =
            format_commit_type_choice(true, commit_types.get("ci").unwrap(), &commit_types);

        assert_eq!(
            choice,
            "ci:         🤖 Changes to our CI configuration files and scripts"
        );
    }

    #[test]
    fn test_validate_summary() {
        let validated = validate_summary("needed more badges :badger:");

        assert!(validated.is_ok());

        let validated = validate_summary("");

        assert!(validated.is_err(), "A description is required.");
    }

    #[test]
    fn test_validate_issue_reference() {
        let validated = validate_issue_reference("closes #123");

        assert!(validated.is_ok());

        let validated = validate_issue_reference("");

        assert!(
            validated.is_err(),
            "An issue reference is required if this commit is related to an open issue."
        );
    }
}
