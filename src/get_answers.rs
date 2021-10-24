use anyhow::{Context, Result};
use linked_hash_map::LinkedHashMap;
use requestty::Answer;

use crate::commit_types::CommitType;

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

/// Parse the commit type out of the menu choice.
/// e.g. `feat: A new feature` -> `feat`
pub fn get_commit_type(answer: Option<&Answer>) -> Result<&str> {
    answer
        .context("could not get commit type")?
        .as_list_item()
        .context("could not convert commit type to list item")?
        .text
        .split(':')
        .next()
        .context("could not extract commit type")
}

/// Gets the scope, returning `None` if it's an empty string.
pub fn get_scope(answer: Option<&Answer>) -> Result<Option<String>> {
    answer
        .context("could not get scope")?
        .as_string()
        .context("could not convert scope to string")
        .map(|s| if s.is_empty() { None } else { Some(s.into()) })
}

/// Gets the summary, prepending a relevant emoji if enabled.
pub fn get_summary(
    answer: Option<&Answer>,
    use_emoji: bool,
    commit_type: &str,
    commit_types: &LinkedHashMap<String, CommitType>,
) -> Result<String> {
    answer
        .context("could not get summary")?
        .as_string()
        .context("could not convert summary to string")
        .map(|s| {
            let commit_type = commit_types.get(commit_type).unwrap();
            let use_emoji = use_emoji && commit_type.emoji.is_some();

            if use_emoji {
                format!("{} {}", commit_type.emoji.as_ref().unwrap(), s)
            } else {
                s.into()
            }
        })
}

/// Gets the body, returning `None` if it's an empty string.
pub fn get_body(answer: Option<&Answer>) -> Result<Option<String>> {
    answer
        .context("could not get body")?
        .as_string()
        .context("could not convert body to string")
        .map(|b| if b.is_empty() { None } else { Some(b.into()) })
}

/// Returns whether or not there's a breaking change.
pub fn get_is_breaking_change(answer: Option<&Answer>) -> Result<bool> {
    answer
        .context("could not get breaking change")?
        .as_bool()
        .context("could not convert breaking change to bool")
}

/// Returns whether or not there's an open issue.
pub fn get_has_open_issue(answer: Option<&Answer>) -> Result<bool> {
    answer
        .context("could not get open issue")?
        .as_bool()
        .context("could not convert open issue to bool")
}

/// Get the issue reference, returning `None` if there isn't
/// an open issue.
pub fn get_issue_reference(
    answer: Option<&Answer>,
    has_open_issue: bool,
) -> Result<Option<String>> {
    if has_open_issue {
        answer
            .context("could not get issue reference")?
            .as_string()
            .context("could not convert issue reference to string")
            .map(|i| if i.is_empty() { None } else { Some(i.into()) })
    } else {
        Ok(None)
    }
}

/// If there is a referenced issue, we want to return a new string
/// appending it to the body. If not, just give back the body.
pub fn get_amended_body(body: &Option<String>, issue_reference: &Option<String>) -> Option<String> {
    match (body, issue_reference) {
        (Some(body), Some(issue_reference)) => {
            Some(format!("{}\n\n{}", body, issue_reference.to_owned()))
        }
        (Some(body), None) => Some(body.into()),
        (None, Some(issue_reference)) => Some(issue_reference.to_owned()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use requestty::ListItem;

    use crate::commit_types::get_default_commit_types;

    use super::*;

    #[test]
    fn test_render_commit_type_choice() {
        let commit_types = get_default_commit_types();

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

    #[test]
    fn test_get_commit_type() {
        let answer = Some(Answer::ListItem(ListItem {
            index: 0,
            text: "docs: Documentation only changes".into(),
        }));

        assert_eq!(get_commit_type(answer.as_ref()).unwrap(), "docs")
    }

    #[test]
    fn test_get_scope() {
        let answer = Some(Answer::String("readme".into()));
        assert_eq!(get_scope(answer.as_ref()).unwrap(), Some("readme".into()));

        let answer = Some(Answer::String("".into()));
        assert_eq!(get_scope(answer.as_ref()).unwrap(), None);
    }

    #[test]
    fn test_get_summary() {
        let commit_types = get_default_commit_types();

        let answer = Some(Answer::String("needed more badges".into()));
        assert_eq!(
            get_summary(answer.as_ref(), false, "docs", &commit_types).unwrap(),
            "needed more badges"
        );

        let answer = Some(Answer::String("needed more badges".into()));
        assert_eq!(
            get_summary(answer.as_ref(), true, "docs", &commit_types).unwrap(),
            "📚 needed more badges"
        );
    }

    #[test]
    fn test_get_body() {
        let answer = Some(Answer::String("i _really_ like badges".into()));
        assert_eq!(
            get_body(answer.as_ref()).unwrap(),
            Some("i _really_ like badges".into())
        );

        let answer = Some(Answer::String("".into()));
        assert_eq!(get_body(answer.as_ref()).unwrap(), None);
    }

    #[test]
    fn test_get_is_breaking_change() {
        let answer = Some(Answer::Bool(true));
        assert!(get_is_breaking_change(answer.as_ref()).unwrap(), "{}", true);
    }

    #[test]
    fn test_get_has_open_issue() {
        let answer = Some(Answer::Bool(true));
        assert!(get_has_open_issue(answer.as_ref()).unwrap(), "{}", true);
    }

    #[test]
    fn test_get_issue_reference() {
        let answer = None;
        assert_eq!(get_issue_reference(answer.as_ref(), false).unwrap(), None);

        let answer = Some(Answer::String("closes #1".into()));
        assert_eq!(
            get_issue_reference(answer.as_ref(), true).unwrap(),
            Some("closes #1".into())
        );
    }

    #[test]
    fn test_get_amended_body() {
        let body = Some("i _really_ like badges".to_string());
        let issue_reference = Some("closes #1".to_string());

        assert_eq!(
            get_amended_body(&body, &issue_reference),
            Some("i _really_ like badges\n\ncloses #1".into())
        );

        assert_eq!(
            get_amended_body(&body, &None),
            Some("i _really_ like badges".into())
        );

        assert_eq!(
            get_amended_body(&None, &issue_reference),
            Some("closes #1".into())
        );

        assert_eq!(get_amended_body(&None, &None), None);
    }
}
