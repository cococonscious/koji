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
        match &commit_type.emoji {
            Some(emoji) => Ok(format!("{emoji} {summary}")),
            _ => Ok(summary),
        }
    } else {
        Ok(summary)
    }
}

/// If there is a referenced issue, we want to return a new string
/// appending it to the body. If not, just give back the body
fn get_amended_body(body: &Option<String>, issue_reference: &Option<String>) -> Option<String> {
    let body = match (body, issue_reference) {
        (Some(body), Some(issue_reference)) => Some(format!("{body}\n\n{issue_reference}")),
        (Some(body), None) => Some(body.into()),
        (None, Some(issue_reference)) => Some(issue_reference.to_owned()),
        (None, None) => None,
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
    Ok(ExtractedAnswers {
        commit_type: answers.commit_type.clone(),
        scope: answers.scope.map(|s| s.replace_emoji_shortcodes()),
        summary: get_summary(
            &answers.summary,
            config.emoji,
            &answers.commit_type,
            &config.commit_types,
        )?,
        body: get_amended_body(&answers.body, &answers.issue_footer),
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

    #[test]
    fn test_get_extracted_answers() {
        let answers = Answers {
            commit_type: "feat".into(),
            scope: Some("space".into()),
            summary: "add more space".into(),
            body: Some("just never enough space!".into()),
            is_breaking_change: false,
            issue_footer: Some("closes #554".into()),
        };

        let config = Config::new(None).unwrap();
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
            "feat(space): add more space\n\njust never enough space!\n\ncloses #554"
        );
    }
}
