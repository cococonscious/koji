mod commit_types;

use commit_types::{
    config_file_exists, get_custom_commit_types, get_default_commit_types, CommitType,
};

use anyhow::{Context, Error, Result};
use clap::{crate_authors, crate_version, App, Arg};
use cocogitto::CocoGitto;
use linked_hash_map::LinkedHashMap;
use requestty::{prompt, Answer, Answers, Question};

/// This look worse than it is, I think, but it's still
/// pretty bad I'm sure. All it's doing is formatting the
/// commit type choices to look like this:
///
/// feat:   ✨ A new feature
/// fix:    🐛 A bug fix
/// docs:   📚 Documentation only changes
fn render_commit_type_choice(
    use_emoji: bool,
    commit_type: &CommitType,
    commit_types: &LinkedHashMap<String, CommitType>,
) -> String {
    format!(
        "{}:{:>width$}{}",
        commit_type.name,
        if use_emoji {
            format!("{} ", commit_type.emoji)
        } else {
            "".into()
        },
        commit_type.description,
        width = commit_types
            .iter()
            .map(|(key, _)| key.chars().count())
            .max()
            .unwrap()
            - commit_type.name.chars().count()
            + if use_emoji { 5 } else { 3 },
    )
}

/// Parse the commit type out of the menu choice.
/// e.g. `feat: A new feature` -> `feat`
fn get_commit_type(answer: Option<&Answer>) -> Result<&str, Error> {
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
fn get_scope(answer: Option<&Answer>) -> Result<Option<String>, Error> {
    answer
        .context("could not get scope")?
        .as_string()
        .context("could not convert scope to string")
        .map(|s| if s.is_empty() { None } else { Some(s.into()) })
}

/// Gets the summary, prepending a relevant emoji if enabled.
fn get_summary(
    answer: Option<&Answer>,
    use_emoji: bool,
    commit_type: &str,
    commit_types: &LinkedHashMap<String, CommitType>,
) -> Result<String, Error> {
    answer
        .context("could not get summary")?
        .as_string()
        .context("could not convert summary to string")
        .map(|s| {
            if use_emoji {
                format!("{} {}", commit_types.get(commit_type).unwrap().emoji, s)
            } else {
                s.into()
            }
        })
}

/// Gets the body, returning `None` if it's an empty string.
fn get_body(answer: Option<&Answer>) -> Result<Option<String>, Error> {
    answer
        .context("could not get body")?
        .as_string()
        .context("could not convert body to string")
        .map(|b| if b.is_empty() { None } else { Some(b.into()) })
}

// Returns whether or not there's a breaking change.
fn get_is_breaking_change(answer: Option<&Answer>) -> Result<bool, Error> {
    answer
        .context("could not get breaking change")?
        .as_bool()
        .context("could not convert breaking change to bool")
}

/// Returns whether or not there's an open issue.
fn get_has_open_issue(answer: Option<&Answer>) -> Result<bool, Error> {
    answer
        .context("could not get open issue")?
        .as_bool()
        .context("could not convert open issue to bool")
}

/// Get the issue reference, returning `None` if there isn't
/// an open issue.
fn get_issue_reference(
    answer: Option<&Answer>,
    has_open_issue: bool,
) -> Result<Option<String>, Error> {
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
fn get_amended_body(
    body: &Option<String>,
    issue_reference: &Option<String>,
) -> Result<Option<String>, Error> {
    match body {
        Some(body) => {
            if issue_reference.is_some() {
                Ok(Some(format!(
                    "{}\n\n{}",
                    body,
                    issue_reference.to_owned().unwrap()
                )))
            } else {
                Ok(Some(body.to_owned()))
            }
        }
        None => {
            if issue_reference.is_some() {
                Ok(issue_reference.to_owned())
            } else {
                Ok(None)
            }
        }
    }
}

fn main() -> Result<()> {
    let commit_types = if config_file_exists() {
        get_custom_commit_types().unwrap()
    } else {
        get_default_commit_types()
    };

    let matches = App::new("koji")
        .about("An interactive cli for creating conventional commits.")
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name("emoji")
                .short("e")
                .long("emoji")
                .help("Prepend summary with relevant emoji based on commit type."),
        )
        .get_matches();

    let use_emoji = matches.is_present("emoji");

    let answers = prompt(vec![
        Question::select("commit_type")
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
        Question::input("scope")
            .message("What is the scope of this change? (press enter to skip)")
            .build(),
        Question::input("summary")
            .message("Write a short, imperative tense description of the change.")
            .build(),
        Question::input("body")
            .message("Provide a longer description of the change. (press enter to skip)")
            .build(),
        Question::confirm("is_breaking_change")
            .message("Are there any breaking changes?")
            .build(),
        Question::confirm("has_open_issue")
            .message("Does this change affect any open issues?")
            .build(),
        Question::input("issue_reference")
            .message("Add issue references. (e.g. \"fix #123\", \"re #123\")")
            .when(|answers: &Answers| match answers.get("has_open_issue") {
                Some(a) => a.as_bool().unwrap(),
                None => false,
            })
            .build(),
    ])?;

    let commit_type = get_commit_type(answers.get("commit_type"))?;
    let scope = get_scope(answers.get("scope"))?;
    let summary = get_summary(
        answers.get("summary"),
        use_emoji,
        commit_type,
        &commit_types,
    )?;
    let body = get_body(answers.get("body"))?;
    let is_breaking_change = get_is_breaking_change(answers.get("is_breaking_change"))?;
    let has_open_issue = get_has_open_issue(answers.get("has_open_issue"))?;
    let issue_reference = get_issue_reference(answers.get("issue_reference"), has_open_issue)?;
    let body = get_amended_body(&body, &issue_reference)?;

    let cocogitto = CocoGitto::get()?;

    cocogitto.conventional_commit(commit_type, scope, summary, body, None, is_breaking_change)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use requestty::ListItem;

    use super::*;

    #[test]
    fn test_render_commit_type_choice() {
        let commit_types = get_default_commit_types();

        let choice =
            render_commit_type_choice(false, commit_types.get("refactor").unwrap(), &commit_types);
        assert_eq!(
            choice,
            "refactor:   A change that neither fixes a bug or adds a feature"
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
            "refactor:   🔨 A change that neither fixes a bug or adds a feature"
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

        assert_eq!(
            get_amended_body(&body, &None).unwrap(),
            Some("i _really_ like badges".into())
        );

        let issue_reference = Some("closes #1".to_string());

        assert_eq!(
            get_amended_body(&body, &issue_reference).unwrap(),
            Some("i _really_ like badges\n\ncloses #1".into())
        );
    }
}
