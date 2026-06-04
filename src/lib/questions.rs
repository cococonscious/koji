use crate::config::{CommitType, Config};
use crate::scope::ScopeMatches;
use anyhow::{Context, Result};
use conventional_commit_parser::parse_summary;
use gix::bstr::ByteSlice;
use indexmap::IndexMap;
use inquire::error::InquireError;
use inquire::ui::{Attributes, Color, RenderConfig, StyleSheet};
use inquire::{
    autocompletion::{Autocomplete, Replacement},
    validator::Validation,
    Confirm, CustomUserError, Select, Text,
};
use thiserror::Error;

#[derive(Debug, Error)]
enum PromptError {
    #[error("{0} cancelled")]
    Cancelled(&'static str),
    #[error(transparent)]
    Inquire(InquireError),
}

impl PromptError {
    fn from_inquire(e: InquireError, prompt_name: &'static str) -> Self {
        match e {
            InquireError::OperationCanceled | InquireError::OperationInterrupted => {
                PromptError::Cancelled(prompt_name)
            }
            other => PromptError::Inquire(other),
        }
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

/// Transform commit type choice
fn transform_commit_type_choice(choice: &str) -> String {
    choice.split(':').next().unwrap().into()
}

/// Format the commit type choices
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

fn prompt_type(config: &Config) -> Result<String> {
    let type_values = config
        .commit_types
        .iter()
        .map(|(_, choice)| format_commit_type_choice(config.emoji, choice, &config.commit_types))
        .collect();

    let selected_type = Select::new("What type of change are you committing?", type_values)
        .with_render_config(get_render_config())
        .with_formatter(&|v| transform_commit_type_choice(v.value))
        .prompt()
        .map_err(|e| PromptError::from_inquire(e, "Commit type selection"))?;

    Ok(transform_commit_type_choice(&selected_type))
}

#[derive(Debug, Clone)]
pub struct ScopeAutocompleter {
    pub config: Config,
}

impl ScopeAutocompleter {
    fn get_existing_scopes(&self) -> Result<Vec<String>> {
        let repo = gix::discover(&self.config.workdir).context("could not find git repository")?;

        let head_id = repo.head_id().context("could not get HEAD")?;

        let walk =
            repo.rev_walk([head_id.detach()])
                .sorting(gix::revision::walk::Sorting::ByCommitTime(
                    gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
                ));

        let mut scopes: Vec<String> = Vec::new();

        for info in walk.all()? {
            let info = info?;

            let commit = repo.find_commit(info.id)?;

            let message = commit.message()?;

            let summary = message.summary();

            // Parse the summary - ignore errors for invalid commit messages
            if let Ok(parsed) = parse_summary(summary.to_str()?) {
                if let Some(scope) = parsed.scope {
                    if !scopes.contains(&scope) {
                        scopes.push(scope);
                    }
                }
            }
        }

        Ok(scopes)
    }

    pub fn get_config_scopes(&self) -> Vec<String> {
        self.config.commit_scopes.keys().cloned().collect()
    }

    pub fn get_all_scopes(&self) -> Vec<String> {
        let mut scopes = self.get_config_scopes();
        let existing_scopes = self.get_existing_scopes().unwrap_or_default();
        // Add existing scopes that aren't already in the config scopes
        for scope in existing_scopes {
            if !scopes.contains(&scope) {
                scopes.push(scope);
            }
        }
        scopes
    }

    /// Returns scope suggestions formatted with descriptions for configured scopes.
    /// History-only scopes are returned as plain names so users can tell them apart.
    fn get_suggestions_with_descriptions(&self) -> Vec<String> {
        let config_scope_names: std::collections::HashSet<_> =
            self.config.commit_scopes.keys().cloned().collect();

        let mut suggestions: Vec<String> = self
            .config
            .commit_scopes
            .values()
            .map(|scope| {
                if let Some(desc) = &scope.description {
                    format!("{}: {}", scope.name, desc)
                } else {
                    scope.name.clone()
                }
            })
            .collect();

        // append the history scopes that aren't already in config
        let history = self.get_existing_scopes().unwrap_or_default();
        for scope in history {
            if !config_scope_names.contains(&scope) {
                suggestions.push(scope);
            }
        }

        suggestions
    }
}

impl Autocomplete for ScopeAutocompleter {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
        Ok(self
            .get_suggestions_with_descriptions()
            .into_iter()
            .filter(|s| {
                // Match against the name portion before any ':'
                let name = s.split(':').next().unwrap_or(s).trim();
                name.contains(input) || s.contains(input)
            })
            .collect())
    }

    #[cfg(not(tarpaulin_include))]
    fn get_completion(
        &mut self,
        _input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<Replacement, CustomUserError> {
        // Strip description from the suggestion so the input receives only the scope name
        Ok(highlighted_suggestion.map(|s| s.split(':').next().unwrap_or(&s).trim().to_string()))
    }
}

fn format_scope_display(scope: &crate::config::CommitScope) -> String {
    match &scope.description {
        Some(desc) => format!("{}: {}", scope.name, desc),
        None => scope.name.clone(),
    }
}

fn scope_name_from_display(display: &str) -> String {
    display
        .split(':')
        .next()
        .unwrap_or(display)
        .trim()
        .to_string()
}

impl Config {
    fn scope_values_ordered(&self, scope_matches: &ScopeMatches) -> Vec<String> {
        let matched: Vec<_> = scope_matches
            .matches
            .iter()
            .filter_map(|name| self.commit_scopes.get(name))
            .map(format_scope_display)
            .collect();

        let rest = self
            .commit_scopes
            .values()
            .filter(|s| !scope_matches.matches.contains(&s.name))
            .map(format_scope_display);

        matched.into_iter().chain(rest).collect()
    }

    fn prompt_scope(&self, scope_matches: &ScopeMatches) -> Result<Option<String>> {
        if self.force_config_scopes && !self.commit_scopes.is_empty() {
            return self.prompt_scope_select(scope_matches);
        }
        self.prompt_scope_text(scope_matches)
    }

    fn prompt_scope_select(&self, scope_matches: &ScopeMatches) -> Result<Option<String>> {
        // If we're able to match then... yay!
        // Only valid if ONE scope was matched.
        if let Some(scope) = scope_matches.suggested() {
            return Ok(Some(scope));
        }

        // Otherwise fallback with user selection
        let scope_values = self.scope_values_ordered(scope_matches);
        let prompt = Select::new("What's the scope of this change?", scope_values)
            .with_render_config(get_render_config())
            .with_formatter(&|v| scope_name_from_display(v.value));

        let result = if self.allow_empty_scope {
            prompt
                .prompt_skippable()
                .map_err(|e| PromptError::from_inquire(e, "Scope selection"))?
        } else {
            Some(
                prompt
                    .prompt()
                    .map_err(|e| PromptError::from_inquire(e, "Scope selection"))?,
            )
        };

        Ok(result.map(|s| scope_name_from_display(&s)))
    }

    fn prompt_scope_text(&self, scope_matches: &ScopeMatches) -> Result<Option<String>> {
        let mut autocompleter = ScopeAutocompleter {
            config: self.clone(),
        };
        let detected = scope_matches.suggested();
        let has_completions = self.autocomplete
            && !autocompleter
                .get_suggestions("")
                .unwrap_or_default()
                .is_empty();

        let help = match (
            has_completions,
            detected.as_deref(),
            scope_matches.matches.len(),
        ) {
            (true, Some(s), _) => format!(
                "↑↓ to move, tab to autocomplete, enter to use `{s}`, {}",
                get_skip_hint()
            ),
            (true, None, n) if n > 1 => format!(
                "↑↓ to move, tab to autocomplete, matched: {}, {}",
                scope_matches.matches.join(", "),
                get_skip_hint()
            ),
            (true, None, _) => format!("↑↓ to move, tab to autocomplete, {}", get_skip_hint()),
            (false, Some(s), _) => format!("enter to use `{s}`, {}", get_skip_hint()),
            (false, None, n) if n > 1 => format!(
                "matched: {}, {}",
                scope_matches.matches.join(", "),
                get_skip_hint()
            ),
            _ => get_skip_hint().to_string(),
        };

        let mut text = Text::new("What's the scope of this change?")
            .with_render_config(RenderConfig {
                option: StyleSheet::new().with_fg(Color::Grey),
                ..get_render_config()
            })
            .with_help_message(&help);

        if let Some(ref scope) = detected {
            text = text.with_initial_value(scope);
        }

        if self.autocomplete {
            text = text.with_autocomplete(autocompleter);
        }

        if !self.allow_empty_scope {
            let has_detected = detected.is_some();
            text = text.with_validator(move |input: &str| {
                if input.trim().is_empty() && !has_detected {
                    Ok(Validation::Invalid("A scope is required".into()))
                } else {
                    Ok(Validation::Valid)
                }
            });
        }

        if !self.allow_empty_scope && detected.is_none() {
            return Ok(Some(
                text.prompt()
                    .map_err(|e| PromptError::from_inquire(e, "Scope selection"))?,
            ));
        }

        match text
            .prompt_skippable()
            .map_err(|e| PromptError::from_inquire(e, "Scope selection"))?
        {
            Some(s) if s.is_empty() => Ok(None),
            result => Ok(result),
        }
    }
}

fn prompt_summary(msg: String) -> Result<String> {
    let previous_summary = match parse_summary(&msg) {
        Ok(parsed) => parsed.summary,
        Err(_) => "".into(),
    };

    let summary = Text::new("Write a short, imperative tense description of the change:")
        .with_render_config(get_render_config())
        .with_placeholder(&previous_summary)
        .with_validator(validate_summary)
        .prompt()
        .map_err(|e| PromptError::from_inquire(e, "Commit summary input"))?;

    Ok(summary)
}

fn prompt_body() -> Result<Option<String>> {
    let help_message = format!("{}, {}", "Use '\\n' for newlines", get_skip_hint());

    match Text::new("Provide a longer description of the change:")
        .with_render_config(get_render_config())
        .with_help_message(help_message.as_str())
        .prompt_skippable()
        .map_err(|e| PromptError::from_inquire(e, "Body input"))?
    {
        Some(summary) if summary.is_empty() => Ok(None),
        Some(summary) => Ok(Some(summary.replace("\\n", "\n"))),
        None => Ok(None),
    }
}

fn prompt_breaking() -> Result<bool> {
    let answer = Confirm::new("Are there any breaking changes?")
        .with_render_config(get_render_config())
        .with_default(false)
        .prompt()
        .map_err(|e| PromptError::from_inquire(e, "Breaking changes prompt"))?;

    Ok(answer)
}

fn prompt_breaking_text() -> Result<Option<String>> {
    let help_message = format!("{}, {}", "Use '\\n' for newlines", get_skip_hint());

    match Text::new("Describe the breaking changes in detail:")
        .with_render_config(get_render_config())
        .with_help_message(help_message.as_str())
        .prompt_skippable()
        .map_err(|e| PromptError::from_inquire(e, "Breaking changes description"))?
    {
        Some(text) if text.is_empty() => Ok(None),
        Some(text) => Ok(Some(text.replace("\\n", "\n"))),
        None => Ok(None),
    }
}

fn prompt_issues() -> Result<bool> {
    let answer = Confirm::new("Does this change affect any open issues?")
        .with_render_config(get_render_config())
        .with_default(false)
        .prompt()
        .map_err(|e| PromptError::from_inquire(e, "Issues prompt"))?;

    Ok(answer)
}

fn prompt_issue_text() -> Result<String> {
    let summary = Text::new("Add the issue reference:")
        .with_render_config(get_render_config())
        .with_help_message("e.g. \"closes #123\"")
        .with_validator(validate_issue_reference)
        .prompt()
        .map_err(|e| PromptError::from_inquire(e, "Issue reference input"))?;

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
    last_message: String,
    config: &Config,
    scope_matches: &ScopeMatches,
) -> Result<Answers> {
    let commit_type = prompt_type(config)?;
    let scope = config.prompt_scope(scope_matches)?;
    let summary = prompt_summary(last_message)?;
    let body = prompt_body()?;

    let mut breaking = false;
    let mut breaking_footer: Option<String> = None;
    if config.breaking_changes {
        breaking = prompt_breaking()?;
        if breaking {
            breaking_footer = prompt_breaking_text()?;
        }
    }

    let mut issue_footer = None;
    if config.issues && prompt_issues()? {
        issue_footer = Some(prompt_issue_text()?);
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
        .prompt()
        .map_err(|e| PromptError::from_inquire(e, "Confirmation"))?;

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
}
