use anyhow::{Context, Result};
use conventional_commit_parser::parse_summary;
use git2::Repository;
use linked_hash_map::LinkedHashMap;
use requestty::{
    prompt,
    question::{completions, Completions},
    Answers, Question,
};

use crate::commit_types::CommitType;

// These exist just so I don't make a typo when using them.
pub const Q_COMMIT_TYPE: &str = "commit_type";
pub const Q_SCOPE: &str = "scope";
pub const Q_SUMMARY: &str = "summary";
pub const Q_BODY: &str = "body";
pub const Q_IS_BREAKING_CHANGE: &str = "is_breaking_change";
pub const Q_HAS_OPEN_ISSUE: &str = "has_open_issue";
pub const Q_ISSUE_REFERENCE: &str = "issue_reference";

// Get a unique list of existing scopes in the commit history.
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

/// Format the commit type choices.
pub fn render_commit_type_choice(
    use_emoji: bool,
    commit_type: &CommitType,
    commit_types: &LinkedHashMap<String, CommitType>,
) -> String {
    let name = &commit_type.name;
    let description = &commit_type.description;
    let use_emoji = use_emoji && commit_type.emoji.is_some();

    let emoji = if use_emoji {
        format!("{} ", commit_type.emoji.as_ref().unwrap())
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

    format!("{}:{:>width$}{}", name, emoji, description, width = width)
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
                write!(backend, "{}", choice.text.split(':').next().unwrap())
            })
            .choices(
                commit_types
                    .iter()
                    .map(|(_, t)| render_commit_type_choice(use_emoji, t, commit_types)),
            )
            .build(),
        Question::input(Q_SCOPE)
            .message("What is the scope of this change? (press enter to skip)")
            .auto_complete(|scope, _| {
                if !use_autocomplete || scopes.is_empty() {
                    completions![scope]
                } else {
                    scopes.clone()
                }
            })
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
            .default(false)
            .build(),
        Question::confirm(Q_HAS_OPEN_ISSUE)
            .message("Does this change affect any open issues?")
            .default(false)
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
    ])
    .context("could not build prompt")
}

#[cfg(test)]
mod tests {
    use crate::{commit_types::get_commit_types, config::load_config};

    use super::*;

    #[test]
    fn test_render_commit_type_choice() {
        let config = load_config(None).unwrap();
        let commit_types = get_commit_types(&config);

        let choice =
            render_commit_type_choice(false, commit_types.get("refactor").unwrap(), &commit_types);

        assert_eq!(
            choice,
            "refactor:   A code change that neither fixes a bug nor adds a feature"
        );

        let choice =
            render_commit_type_choice(false, commit_types.get("ci").unwrap(), &commit_types);

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
            render_commit_type_choice(true, commit_types.get("refactor").unwrap(), &commit_types);

        assert_eq!(
            choice,
            "refactor:   🔨 A code change that neither fixes a bug nor adds a feature"
        );

        let choice =
            render_commit_type_choice(true, commit_types.get("ci").unwrap(), &commit_types);

        assert_eq!(
            choice,
            "ci:         🤖 Changes to our CI configuration files and scripts"
        );
    }
}
