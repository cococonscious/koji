use anyhow::Result;
use indexmap::IndexMap;

use crate::{
    config::{CommitType, Config},
    emoji::ReplaceEmoji,
    questions::Answers,
};

/// Get the summary, prepending a relevant emoji if enabled
fn get_summary(
    answer: &str,
    use_emoji: bool,
    commit_type: &str,
    commit_types: &IndexMap<String, CommitType>,
) -> Result<String> {
    let commit_type = commit_types.get(commit_type).unwrap();
    let use_emoji = use_emoji && commit_type.emoji.is_some();
    let summary = answer.replace_emoji_shortcodes();

    if use_emoji {
        Ok(format!("{} {summary}", commit_type.emoji.as_ref().unwrap()))
    } else {
        Ok(summary)
    }
}

/// Appends the breaking change footer key to the breaking change text if provided.
/// Functions using this are expected to check for the text validity before calling this function.
fn into_breaking_footer(breaking_text: &Option<String>) -> Option<String> {
    breaking_text
        .as_ref()
        .map(|b| format!("BREAKING CHANGE: {b}"))
}

/// Get the body, amending it with issue references and breaking changes if provided
fn get_amended_body(
    body: &Option<String>,
    issue_reference: &Option<String>,
    breaking_text: &Option<String>,
) -> Option<String> {
    let body = match (body, issue_reference, breaking_text) {
        (Some(body), Some(issue_reference), Some(breaking_text)) => {
            Some(format!("{body}\n\n{issue_reference}\n{breaking_text}"))
        }
        (Some(body), Some(issue_reference), None) => Some(format!("{body}\n\n{issue_reference}")),
        (Some(body), None, Some(breaking_text)) => Some(format!("{body}\n\n{breaking_text}")),
        (Some(body), None, None) => Some(body.into()),
        (None, Some(issue_reference), Some(breaking_text)) => {
            Some(format!("{issue_reference}\n{breaking_text}"))
        }
        (None, Some(issue_reference), None) => Some(issue_reference.to_owned()),
        (None, None, Some(breaking_text)) => Some(breaking_text.to_owned()),
        (None, None, None) => None,
    };
    body.map(|b| b.replace_emoji_shortcodes())
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExtractedAnswers {
    pub body: Option<String>,
    pub commit_type: String,
    pub is_breaking_change: bool,
    pub scope: Option<String>,
    pub summary: String,
}

/// Extract the prompt answers into an `ExtractedAnswers`,
/// making it usable for creating a commit
pub fn get_extracted_answers(answers: Answers, config: &Config) -> Result<ExtractedAnswers> {
    // The breaking change footer text should never be present if `is_breaking_change` is false, but
    // we're checking for it anyway
    let breaking_change_footer: Option<String> = if answers.is_breaking_change {
        into_breaking_footer(&answers.breaking_change_footer)
    } else {
        None
    };

    Ok(ExtractedAnswers {
        commit_type: answers.commit_type.clone(),
        scope: answers.scope.map(|s| s.replace_emoji_shortcodes()),
        summary: get_summary(
            &answers.summary,
            config.emoji,
            &answers.commit_type,
            &config.commit_types,
        )?,
        body: get_amended_body(
            &answers.body,
            &answers.issue_footer,
            &breaking_change_footer,
        ),
        is_breaking_change: answers.is_breaking_change,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use cocogitto::CocoGitto;

    use super::*;

    use indexmap::indexmap;

    #[test]
    fn test_get_summary() {
        let config = Config::new(None).unwrap();
        let commit_types = config.commit_types;

        let answer = "needed more badges";

        assert_eq!(
            get_summary(answer, false, "docs", &commit_types).unwrap(),
            "needed more badges"
        );
    }

    #[test]
    fn test_get_summary_with_emoji() {
        let config = Config::new(None).unwrap();
        let commit_types = config.commit_types;

        let answer = "needed more badges";

        assert_eq!(
            get_summary(answer, true, "docs", &commit_types).unwrap(),
            "📚 needed more badges"
        );
    }

    #[test]
    fn test_get_summary_with_non_configured_emoji() {
        let commit_types = indexmap! {
            "docs".into() => CommitType {
                name: "docs".into(),
                description: "Changes to documentation".into(),
                emoji: None,
            },
        };

        let answer = "needed more badges";

        assert_eq!(
            get_summary(answer, true, "docs", &commit_types).unwrap(),
            "needed more badges"
        );
    }

    #[test]
    fn test_get_summary_with_shortcode() {
        let config = Config::new(None).unwrap();
        let commit_types = config.commit_types;

        let answer = "needed more badges :badger:";

        assert_eq!(
            get_summary(answer, false, "docs", &commit_types).unwrap(),
            "needed more badges 🦡"
        );
    }

    #[test]
    fn test_into_breaking_footer() {
        let breaking_text = Some("this is a breaking change".to_string());
        assert_eq!(
            into_breaking_footer(&breaking_text),
            Some("BREAKING CHANGE: this is a breaking change".into())
        );

        let breaking_text = None;
        assert_eq!(into_breaking_footer(&breaking_text), None);
    }

    #[test]
    fn test_get_amended_body() {
        let body = Some("i _really_ like badges".to_string());
        let issue_reference = Some("closes #1".to_string());
        let breaking_text = Some("BREAKING CHANGE: this is a breaking change".to_string());

        assert_eq!(
            get_amended_body(&body, &issue_reference, &breaking_text),
            Some(
                "i _really_ like badges\n\ncloses #1\nBREAKING CHANGE: this is a breaking change"
                    .into()
            )
        );

        assert_eq!(
            get_amended_body(&body, &issue_reference, &None),
            Some("i _really_ like badges\n\ncloses #1".into())
        );

        assert_eq!(
            get_amended_body(&body, &None, &breaking_text),
            Some("i _really_ like badges\n\nBREAKING CHANGE: this is a breaking change".into())
        );

        assert_eq!(
            get_amended_body(&body, &None, &None),
            Some("i _really_ like badges".into())
        );

        assert_eq!(
            get_amended_body(&None, &issue_reference, &breaking_text),
            Some("closes #1\nBREAKING CHANGE: this is a breaking change".into())
        );

        assert_eq!(
            get_amended_body(&None, &issue_reference, &None),
            Some("closes #1".into())
        );

        assert_eq!(
            get_amended_body(&None, &None, &breaking_text),
            Some("BREAKING CHANGE: this is a breaking change".into())
        );

        assert_eq!(get_amended_body(&None, &None, &None), None);
    }

    #[test]
    fn test_get_extracted_answers() {
        let answers = Answers {
            commit_type: "feat".into(),
            scope: Some("space".into()),
            summary: "add more space".into(),
            body: Some("just never enough space!".into()),
            issue_footer: Some("closes #554".into()),
            is_breaking_change: true,
            breaking_change_footer: Some("this is a breaking change".into()),
        };

        let config = Config::new(None).unwrap();
        let extracted_answers = get_extracted_answers(answers, &config).unwrap();

        assert_eq!(
            extracted_answers,
            ExtractedAnswers {
                commit_type: "feat".into(),
                scope: Some("space".into()),
                summary: "add more space".into(),
                body: Some("just never enough space!\n\ncloses #554\nBREAKING CHANGE: this is a breaking change".into()),
                is_breaking_change: true,
            }
        );

        let message = CocoGitto::get_conventional_message(
            &extracted_answers.commit_type,
            extracted_answers.scope,
            extracted_answers.summary,
            extracted_answers.body,
            None,
            extracted_answers.is_breaking_change,
        )
        .unwrap();

        assert_eq!(
            message,
            "feat(space)!: add more space\n\njust never enough space!\n\ncloses #554\nBREAKING CHANGE: this is a breaking change"
        );

        // Test with no breaking change

        let answers = Answers {
            commit_type: "feat".into(),
            scope: Some("space".into()),
            summary: "add more space".into(),
            body: Some("just never enough space!".into()),
            issue_footer: Some("closes #554".into()),
            is_breaking_change: false,
            breaking_change_footer: Some("this is a breaking change".into()),
        };

        let extracted_answers = get_extracted_answers(answers, &config).unwrap();

        assert_eq!(
            extracted_answers,
            ExtractedAnswers {
                commit_type: "feat".into(),
                scope: Some("space".into()),
                summary: "add more space".into(),
                body: Some("just never enough space!\n\ncloses #554".into()),
                is_breaking_change: false,
            }
        );
    }
}
