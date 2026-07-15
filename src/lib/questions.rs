use crate::config::{CommitType, Config};
use crate::vcs::VcsBackend;
use anyhow::Result;
use indexmap::IndexMap;
use inquire::ui::{Attributes, Color, RenderConfig, StyleSheet};
use inquire::{
    autocompletion::{Autocomplete, Replacement},
    validator::Validation,
    Confirm, CustomUserError, Select, Text,
};

#[derive(Debug, Clone, Default)]
pub struct PreviousAnswers {
    pub commit_type: Option<String>,
    pub scope: Option<String>,
    pub summary: Option<String>,
    pub body: Option<String>,
    pub is_breaking_change: bool,
    pub breaking_change_text: Option<String>,
    pub issue_footer: Option<String>,
}

impl PreviousAnswers {
    /// Parse an existing conventional commit description into pre-populated answers.
    ///
    /// Returns `None` if the description cannot be parsed as a conventional commit.
    pub fn from_description(desc: &str) -> Option<Self> {
        let parsed = conventional_commit_parser::parse(desc).ok()?;

        let breaking_pos = parsed.footers.iter().position(|f| f.is_breaking_change());

        let issue_footer_entry = match breaking_pos {
            Some(pos) if pos > 0 => Some(&parsed.footers[pos - 1]),
            Some(_) => None,
            None => parsed.footers.last(),
        }
        .filter(|f| !f.is_breaking_change());

        Some(Self {
            commit_type: Some(parsed.commit_type.as_ref().to_string()),
            scope: parsed.scope,
            summary: Some(parsed.summary),
            body: parsed.body,
            is_breaking_change: parsed.is_breaking_change,
            breaking_change_text: parsed
                .footers
                .iter()
                .find(|f| f.is_breaking_change())
                .map(|f| f.content.clone()),
            issue_footer: issue_footer_entry.map(|f| {
                if matches!(
                    f.token_separator,
                    conventional_commit_parser::commit::Separator::Hash
                ) {
                    format!("{} #{}", f.token, f.content)
                } else {
                    format!("{}: {}", f.token, f.content)
                }
            }),
        })
    }
}

fn get_skip_hint() -> &'static str {
    "<esc> or <return> to skip"
}

fn get_render_config() -> RenderConfig<'static> {
    RenderConfig {
        prompt: StyleSheet::new().with_attr(Attributes::BOLD),
        default_value: StyleSheet::new().with_fg(Color::Grey),
        ..RenderConfig::default()
    }
}

fn transform_commit_type_choice(choice: &str) -> String {
    choice.split(':').next().unwrap().into()
}

fn format_commit_type_choice(
    use_emoji: bool,
    commit_type: &CommitType,
    commit_types: &IndexMap<String, CommitType>,
) -> String {
    let name = &commit_type.name;
    let description = &commit_type.description;
    let use_emoji = use_emoji && commit_type.emoji.is_some();

    let emoji = if use_emoji {
        format!("{} ", commit_type.emoji.as_ref().unwrap())
    } else {
        String::new()
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

fn validate_summary(input: &str) -> Result<Validation, CustomUserError> {
    match input.trim().is_empty() {
        false => Ok(Validation::Valid),
        true => Ok(Validation::Invalid("A summary is required".into())),
    }
}

fn validate_issue_reference(input: &str) -> Result<Validation, CustomUserError> {
    if input.trim().is_empty() {
        Ok(Validation::Invalid("An issue reference is required".into()))
    } else {
        Ok(Validation::Valid)
    }
}

fn prompt_type(config: &Config, previous_type: Option<&str>) -> Result<String> {
    let mut type_values: Vec<String> = config
        .commit_types
        .iter()
        .map(|(_, choice)| format_commit_type_choice(config.emoji, choice, &config.commit_types))
        .collect();

    if let Some(prev) = previous_type {
        if let Some(pos) = type_values
            .iter()
            .position(|v| transform_commit_type_choice(v) == prev)
        {
            let item = type_values.remove(pos);
            type_values.insert(0, item);
        }
    }

    let selected_type = Select::new("What type of change are you committing?", type_values)
        .with_render_config(get_render_config())
        .with_formatter(&|v| transform_commit_type_choice(v.value))
        .prompt()?;

    Ok(transform_commit_type_choice(&selected_type))
}

#[derive(Debug, Clone)]
struct ScopeAutocompleter {
    scopes: Vec<String>,
}

impl ScopeAutocompleter {
    fn new(backend: &VcsBackend) -> Self {
        let scopes = backend.commit_scopes().unwrap_or_default();
        Self { scopes }
    }
}

impl Autocomplete for ScopeAutocompleter {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
        Ok(self
            .scopes
            .iter()
            .filter(|s| s.contains(input))
            .cloned()
            .collect())
    }

    #[cfg(not(tarpaulin_include))]
    fn get_completion(
        &mut self,
        _input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<Replacement, CustomUserError> {
        Ok(highlighted_suggestion)
    }
}

fn prompt_scope(
    config: &Config,
    backend: &VcsBackend,
    previous_scope: Option<&str>,
) -> Result<Option<String>> {
    let scope_autocompleter = ScopeAutocompleter::new(backend);
    let help_message = if config.autocomplete && !scope_autocompleter.scopes.is_empty() {
        format!(
            "{}, {}",
            "↑↓ to move, tab to autocomplete, enter to submit",
            get_skip_hint()
        )
    } else {
        get_skip_hint().to_string()
    };

    let mut selected_scope = Text::new("What's the scope of this change?")
        .with_render_config(RenderConfig {
            option: StyleSheet::new().with_fg(Color::Grey),
            ..get_render_config()
        })
        .with_help_message(help_message.as_str());

    if let Some(prev) = previous_scope {
        selected_scope = selected_scope.with_initial_value(prev);
    }

    if config.autocomplete {
        selected_scope = selected_scope.with_autocomplete(scope_autocompleter);
    }

    if let Some(scope) = selected_scope.prompt_skippable()? {
        if scope.is_empty() {
            return Ok(None);
        }
        Ok(Some(scope))
    } else {
        Ok(None)
    }
}

fn prompt_summary(previous_summary: Option<&str>) -> Result<String> {
    let mut prompt = Text::new("Write a short, imperative tense description of the change:")
        .with_render_config(get_render_config())
        .with_validator(validate_summary);

    if let Some(prev) = previous_summary {
        if !prev.is_empty() {
            prompt = prompt.with_initial_value(prev);
        }
    }

    let summary = prompt.prompt()?;

    Ok(summary)
}

fn prompt_body(previous_body: Option<&str>) -> Result<Option<String>> {
    let help_message = format!("{}, {}", "Use '\\n' for newlines", get_skip_hint());

    // Pre-fill: convert real newlines back to \\n for display in the single-line input
    let escaped_body = previous_body
        .filter(|p| !p.is_empty())
        .map(|p| p.replace('\n', "\\n"));

    let mut prompt = Text::new("Provide a longer description of the change:")
        .with_render_config(get_render_config())
        .with_help_message(help_message.as_str());

    if let Some(ref escaped) = escaped_body {
        prompt = prompt.with_initial_value(escaped);
    }

    let summary = prompt.prompt_skippable()?;

    if let Some(summary) = summary {
        if summary.is_empty() {
            return Ok(None);
        }
        Ok(Some(summary.replace("\\n", "\n")))
    } else {
        Ok(None)
    }
}

fn prompt_breaking(previous_breaking: bool) -> Result<bool> {
    let answer = Confirm::new("Are there any breaking changes?")
        .with_render_config(get_render_config())
        .with_default(previous_breaking)
        .prompt()?;

    Ok(answer)
}

fn prompt_breaking_text(previous_text: Option<&str>) -> Result<Option<String>> {
    let help_message = format!("{}, {}", "Use '\\n' for newlines", get_skip_hint());

    let escaped_text = previous_text
        .filter(|p| !p.is_empty())
        .map(|p| p.replace('\n', "\\n"));

    let mut prompt = Text::new("Describe the breaking changes in detail:")
        .with_render_config(get_render_config())
        .with_help_message(help_message.as_str());

    if let Some(ref escaped) = escaped_text {
        prompt = prompt.with_initial_value(escaped);
    }

    let breaking_text = prompt.prompt_skippable()?;

    if let Some(breaking_text) = breaking_text {
        if breaking_text.is_empty() {
            return Ok(None);
        }
        Ok(Some(breaking_text.replace("\\n", "\n")))
    } else {
        Ok(None)
    }
}

fn prompt_issues(has_previous_issue: bool) -> Result<bool> {
    let answer = Confirm::new("Does this change affect any open issues?")
        .with_render_config(get_render_config())
        .with_default(has_previous_issue)
        .prompt()?;

    Ok(answer)
}

fn prompt_issue_text(previous_issue: Option<&str>) -> Result<String> {
    let mut prompt = Text::new("Add the issue reference:")
        .with_render_config(get_render_config())
        .with_help_message("e.g. \"closes #123\"")
        .with_validator(validate_issue_reference);

    if let Some(prev) = previous_issue {
        if !prev.is_empty() {
            prompt = prompt.with_initial_value(prev);
        }
    }

    let summary = prompt.prompt()?;

    Ok(summary)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Answers {
    pub commit_type: String,
    pub scope: Option<String>,
    pub summary: String,
    pub body: Option<String>,
    pub issue_footer: Option<String>,
    pub is_breaking_change: bool,
    pub breaking_change_footer: Option<String>,
}

/// Create the interactive prompt
pub fn create_prompt(
    previous: Option<PreviousAnswers>,
    config: &Config,
    backend: &VcsBackend,
) -> Result<Answers> {
    let prev = previous.unwrap_or_default();

    let commit_type = prompt_type(config, prev.commit_type.as_deref())?;
    let scope = prompt_scope(config, backend, prev.scope.as_deref())?;
    let summary = prompt_summary(prev.summary.as_deref())?;
    let body = prompt_body(prev.body.as_deref())?;

    let mut breaking = false;
    let mut breaking_footer: Option<String> = None;
    if config.breaking_changes {
        breaking = prompt_breaking(prev.is_breaking_change)?;
        if breaking {
            breaking_footer = prompt_breaking_text(prev.breaking_change_text.as_deref())?;
        }
    }

    let mut issue_footer = None;
    if config.issues && prompt_issues(prev.issue_footer.is_some())? {
        issue_footer = Some(prompt_issue_text(prev.issue_footer.as_deref())?);
    }

    Ok(Answers {
        commit_type,
        scope,
        summary,
        body,
        issue_footer,
        is_breaking_change: breaking,
        breaking_change_footer: breaking_footer,
    })
}

/// Prompt the user to confirm the commit
pub fn prompt_confirm() -> Result<bool> {
    let answer = Confirm::new("Proceed with this commit?")
        .with_render_config(get_render_config())
        .with_default(true)
        .prompt()?;

    Ok(answer)
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

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
        let config = Config::new(None).unwrap();
        let commit_types = config.commit_types;

        let choice =
            format_commit_type_choice(false, commit_types.get("ci").unwrap(), &commit_types);

        assert_eq!(
            choice,
            "ci:         Changes to our CI configuration files and scripts"
        );
    }

    #[test]
    fn test_format_commit_type_choice_emoji() {
        let commit_type = CommitType {
            name: "123".into(),
            emoji: None,
            description: "Test".into(),
        };

        let commit_types = indexmap::indexmap! {
            "123".into() => commit_type.clone(),
        };

        let choice = format_commit_type_choice(true, &commit_type, &commit_types);

        assert_eq!(choice, "123:   Test");
    }

    #[test]
    fn test_render_commit_type_choice_with_emoji() {
        let config = Config::new(None).unwrap();
        let commit_types = config.commit_types;

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
        assert!(validated
            .expect("Summary should be OK")
            .eq(&Validation::Valid));

        let validated = validate_summary("");

        assert!(validated.is_ok());
        assert!(validated
            .expect("Summary should be OK")
            .eq(&Validation::Invalid("A summary is required".into())));
    }

    #[test]
    fn test_validate_issue_reference() {
        let validated = validate_issue_reference("closes #123");

        assert!(validated.is_ok());
        assert!(validated
            .expect("Issue reference should be OK")
            .eq(&Validation::Valid));

        let validated = validate_issue_reference("");

        assert!(validated.is_ok());
        assert!(validated
            .expect("Summary should be OK")
            .eq(&Validation::Invalid(
                "An issue reference is required".into()
            )));
    }

    #[test]
    fn test_from_description_simple() {
        let result = PreviousAnswers::from_description("feat: add user login");

        let answers = result.expect("should parse a simple conventional commit");
        assert_eq!(answers.commit_type, Some("feat".into()));
        assert_eq!(answers.scope, None);
        assert_eq!(answers.summary, Some("add user login".into()));
        assert_eq!(answers.body, None);
        assert!(!answers.is_breaking_change);
        assert_eq!(answers.breaking_change_text, None);
        assert_eq!(answers.issue_footer, None);
    }

    #[test]
    fn test_from_description_with_scope() {
        let result = PreviousAnswers::from_description("fix(parser): handle edge case");

        let answers = result.expect("should parse commit with scope");
        assert_eq!(answers.commit_type, Some("fix".into()));
        assert_eq!(answers.scope, Some("parser".into()));
        assert_eq!(answers.summary, Some("handle edge case".into()));
    }

    #[test]
    fn test_from_description_with_body() {
        let desc = "feat: add login\n\nThis adds a complete login flow with OAuth support.";
        let result = PreviousAnswers::from_description(desc);

        let answers = result.expect("should parse commit with body");
        assert_eq!(answers.commit_type, Some("feat".into()));
        assert_eq!(answers.summary, Some("add login".into()));
        assert_eq!(
            answers.body,
            Some("This adds a complete login flow with OAuth support.".into())
        );
    }

    #[test]
    fn test_from_description_with_breaking_change_footer() {
        let desc =
            "feat!: remove deprecated API\n\nBREAKING CHANGE: The /v1 endpoints have been removed";
        let result = PreviousAnswers::from_description(desc);

        let answers = result.expect("should parse commit with breaking change");
        assert!(answers.is_breaking_change);
        assert_eq!(
            answers.breaking_change_text,
            Some("The /v1 endpoints have been removed".into())
        );
    }

    #[test]
    fn test_from_description_with_breaking_change_bang_only() {
        let result = PreviousAnswers::from_description("feat!: remove deprecated API");

        let answers = result.expect("should parse breaking change with bang");
        assert!(answers.is_breaking_change);
        assert_eq!(answers.breaking_change_text, None);
    }

    #[test]
    fn test_from_description_with_issue_footer() {
        let desc = "fix: resolve crash\n\nCloses #456";
        let result = PreviousAnswers::from_description(desc);

        let answers = result.expect("should parse commit with issue footer");
        assert_eq!(answers.issue_footer, Some("Closes #456".into()));
    }

    #[test]
    fn test_from_description_with_issue_and_breaking_footers() {
        let desc = "feat!: overhaul auth\n\nRefs #789\nBREAKING CHANGE: Token format changed";
        let result = PreviousAnswers::from_description(desc);

        let answers = result.expect("should parse commit with both footers");
        assert!(answers.is_breaking_change);
        assert_eq!(
            answers.breaking_change_text,
            Some("Token format changed".into())
        );
        assert_eq!(answers.issue_footer, Some("Refs #789".into()));
    }

    #[test]
    fn test_from_description_with_colon_separator_footer() {
        let desc = "fix: resolve crash\n\nCloses: 456";
        let result = PreviousAnswers::from_description(desc);

        let answers = result.expect("should parse commit with colon-separated footer");
        assert_eq!(answers.issue_footer, Some("Closes: 456".into()));
    }

    #[test]
    fn test_from_description_invalid_message() {
        let result = PreviousAnswers::from_description("not a conventional commit");
        assert!(result.is_none());
    }

    #[test]
    fn test_from_description_empty_string() {
        let result = PreviousAnswers::from_description("");
        assert!(result.is_none());
    }

    #[test]
    fn test_from_description_breaking_footer_only_no_issue() {
        let desc = "feat!: drop support\n\nBREAKING CHANGE: Removed Python 2 support";
        let result = PreviousAnswers::from_description(desc);

        let answers = result.expect("should parse breaking-only footer");
        assert!(answers.is_breaking_change);
        assert_eq!(
            answers.breaking_change_text,
            Some("Removed Python 2 support".into())
        );
        assert_eq!(answers.issue_footer, None);
    }
}
