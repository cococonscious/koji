use crate::config::{CommitType, Config};
use anyhow::{Context, Result};
use conventional_commit_parser::parse_summary;
use git2::Repository;
use indexmap::IndexMap;
use inquire::ui::{Attributes, Color, RenderConfig, StyleSheet};
use inquire::{validator::Validation, Confirm, CustomUserError, Select, Text};

#[cfg(not(tarpaulin_include))]
fn get_skip_hint() -> &'static str {
    "<esc> or <return> to skip"
}

#[cfg(not(tarpaulin_include))]
fn get_render_config() -> RenderConfig<'static> {
    RenderConfig {
        prompt: StyleSheet::new().with_attr(Attributes::BOLD),
        default_value: StyleSheet::new().with_fg(Color::Grey),
        ..RenderConfig::default()
    }
}

/// Get a unique list of existing scopes in the commit history
#[cfg(not(tarpaulin_include))]
fn get_existing_scopes(repo: &Repository) -> Result<Vec<String>> {
    let mut walk = repo.revwalk()?;

    walk.push_head()?;
    walk.set_sorting(git2::Sort::TIME)?;

    let mut scopes: Vec<String> = Vec::new();

    for id in walk {
        if let Some(summary) = repo.find_commit(id?)?.summary() {
            // We want to throw away any error from `parse_summary` since an
            // invalid commit message should just be ignored
            if let Ok(parsed) = parse_summary(summary) {
                if let Some(scope) = parsed.scope {
                    if !scopes.contains(&scope) {
                        scopes.push(scope);
                    }
                }
            }
        }
    }

    Ok(scopes)
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

#[cfg(not(tarpaulin_include))]
pub fn prompt_type(config: &Config) -> Result<String> {
    let type_values = config
        .commit_types
        .iter()
        .map(|(_, choice)| format_commit_type_choice(config.emoji, choice, &config.commit_types))
        .collect();

    let selected_type = Select::new("What type of change are you committing?", type_values)
        .with_render_config(get_render_config())
        .with_formatter(&|v| transform_commit_type_choice(v.value))
        .prompt()?;

    Ok(transform_commit_type_choice(&selected_type))
}

#[cfg(not(tarpaulin_include))]
pub fn prompt_scope(config: &Config) -> Result<Option<String>> {
    fn scope_autocompleter_empty(_: &str) -> Result<Vec<String>, CustomUserError> {
        Ok(vec![])
    }
    fn scope_autocompleter(val: &str) -> Result<Vec<String>, CustomUserError> {
        let repo = Repository::discover(std::env::current_dir()?)
            .context("could not find git repository")?;
        let existing_scopes = get_existing_scopes(&repo)?;

        Ok(existing_scopes
            .iter()
            .filter(|s| s.contains(val))
            .cloned()
            .collect())
    }

    let help_message = if config.autocomplete && !scope_autocompleter("").unwrap().is_empty() {
        format!(
            "{}, {}",
            "↑↓ to move, tab to autocomplete, enter to submit",
            get_skip_hint()
        )
    } else {
        get_skip_hint().to_string()
    };

    let selected_scope = Text::new("What's the scope of this change?")
        .with_render_config(RenderConfig {
            option: StyleSheet::new().with_fg(Color::Grey),
            ..get_render_config()
        })
        .with_help_message(help_message.as_str())
        .with_autocomplete(if config.autocomplete {
            scope_autocompleter
        } else {
            scope_autocompleter_empty
        })
        .prompt_skippable()?;

    if let Some(scope) = selected_scope {
        if scope.is_empty() {
            return Ok(None);
        }
        Ok(Some(scope))
    } else {
        Ok(None)
    }
}

#[cfg(not(tarpaulin_include))]
pub fn prompt_summary(msg: String) -> Result<String> {
    let previous_summary = match parse_summary(&msg) {
        Ok(parsed) => parsed.summary,
        Err(_) => "".into(),
    };

    let summary = Text::new("Write a short, imperative tense description of the change:")
        .with_render_config(get_render_config())
        .with_placeholder(&previous_summary)
        .with_validator(validate_summary)
        .prompt()?;

    Ok(summary)
}

#[cfg(not(tarpaulin_include))]
pub fn prompt_body() -> Result<Option<String>> {
    let help_message = format!("{}, {}", "Use '\\n' for newlines", get_skip_hint());

    let summary = Text::new("Provide a longer description of the change:")
        .with_render_config(get_render_config())
        .with_help_message(help_message.as_str())
        .prompt_skippable()?;

    if let Some(summary) = summary {
        if summary.is_empty() {
            return Ok(None);
        }
        Ok(Some(summary.replace("\\n", "\n")))
    } else {
        Ok(None)
    }
}

#[cfg(not(tarpaulin_include))]
pub fn prompt_breaking() -> Result<bool> {
    let answer = Confirm::new("Are there any breaking changes?")
        .with_render_config(get_render_config())
        .with_default(false)
        .prompt()?;

    Ok(answer)
}

#[cfg(not(tarpaulin_include))]
pub fn prompt_breaking_text() -> Result<Option<String>> {
    let help_message = format!("{}, {}", "Use '\\n' for newlines", get_skip_hint());

    let breaking_text = Text::new("Describe the breaking changes in detail:")
        .with_render_config(get_render_config())
        .with_help_message(help_message.as_str())
        .prompt_skippable()?;

    if let Some(breaking_text) = breaking_text {
        if breaking_text.is_empty() {
            return Ok(None);
        }
        Ok(Some(breaking_text.replace("\\n", "\n")))
    } else {
        Ok(None)
    }
}

#[cfg(not(tarpaulin_include))]
pub fn prompt_issues() -> Result<bool> {
    let answer = Confirm::new("Does this change affect any open issues?")
        .with_render_config(get_render_config())
        .with_default(false)
        .prompt()?;

    Ok(answer)
}

#[cfg(not(tarpaulin_include))]
pub fn prompt_issue_text() -> Result<String> {
    let summary = Text::new("Add the issue reference:")
        .with_render_config(get_render_config())
        .with_help_message("e.g. \"closes #123\"")
        .with_validator(validate_issue_reference)
        .prompt()?;

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
#[cfg(not(tarpaulin_include))]
pub fn create_prompt(last_message: String, config: &Config) -> Result<Answers> {
    let commit_type = prompt_type(config)?;
    let scope = prompt_scope(config)?;
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
