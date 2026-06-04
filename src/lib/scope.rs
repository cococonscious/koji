use anyhow::{Context, Result};
use gix::bstr::ByteSlice;
use gix::diff::index::ChangeRef;
use gix::Repository;
use globset::{Glob, GlobMatcher};
use indexmap::IndexSet;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ast_grep_config::{GlobalRules, RuleCollection, RuleConfig};
use ast_grep_language::{LanguageExt, SupportLang};

use crate::config::Config;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeMatches {
    pub matches: Vec<String>,
}

impl ScopeMatches {
    pub fn suggested(&self) -> Option<String> {
        match self.matches.as_slice() {
            [scope] => Some(scope.clone()),
            _ => None,
        }
    }
}

struct CompiledAstGrepRules {
    rules: RuleCollection<SupportLang>,
    ids_to_scope: HashMap<String, String>,
}

#[derive(Debug, Clone)]
enum PathPatternMatcher {
    Regex(Regex),
    Glob(GlobMatcher),
}

impl PathPatternMatcher {
    fn new(pattern: &str) -> Result<Self> {
        if looks_like_regex(pattern) {
            let regex = Regex::new(pattern)
                .with_context(|| format!("invalid scope path regex `{pattern}`"))?;
            return Ok(Self::Regex(regex));
        }

        let glob_pattern = pattern.strip_prefix('/').unwrap_or(pattern);
        let glob = Glob::new(glob_pattern)
            .with_context(|| format!("invalid scope path pattern `{pattern}`"))?
            .compile_matcher();

        Ok(Self::Glob(glob))
    }

    fn is_match(&self, path: &str) -> bool {
        match self {
            Self::Regex(regex) => regex.is_match(path),
            Self::Glob(glob) => glob.is_match(path.strip_prefix('/').unwrap_or(path)),
        }
    }
}

fn looks_like_regex(pattern: &str) -> bool {
    pattern.starts_with('^')
        || pattern.ends_with('$')
        || pattern.contains('\\')
        || pattern.contains('(')
        || pattern.contains('|')
        || pattern.contains('+')
}

pub fn validate_scope_patterns(config: &Config) -> Result<()> {
    for (scope, patterns) in &config.scope_patterns {
        for pattern in patterns.iter() {
            PathPatternMatcher::new(pattern)
                .with_context(|| format!("invalid scope path pattern for `{scope}`"))?;
        }
    }

    Ok(())
}

pub fn validate_ast_grep_rules(config: &Config) -> Result<()> {
    compile_ast_grep_rules(config).map(|_| ())
}

pub fn detect_scope_matches(repo: &Repository, config: &Config) -> Result<ScopeMatches> {
    let changed_paths = staged_paths(repo)?;
    if changed_paths.is_empty() {
        return Ok(ScopeMatches::default());
    }

    let mut matched_scopes = IndexSet::new();

    for relative_path in &changed_paths {
        let normalized_path = normalize_relative_path(relative_path);

        for (scope, patterns) in &config.scope_patterns {
            let is_match = patterns
                .iter()
                .map(PathPatternMatcher::new)
                .collect::<Result<Vec<_>>>()?
                .iter()
                .any(|matcher| matcher.is_match(&normalized_path));

            if is_match {
                matched_scopes.insert(scope.clone());
            }
        }
    }

    let workdir = repo
        .workdir()
        .context("could not determine repository workdir")?;
    let ast_scopes = detect_ast_grep_scopes(config, workdir, &changed_paths)?;
    for scope in ast_scopes {
        matched_scopes.insert(scope);
    }

    Ok(ScopeMatches {
        matches: matched_scopes.into_iter().collect(),
    })
}

fn staged_paths(repo: &Repository) -> Result<Vec<PathBuf>> {
    let index = repo.index_or_empty().context("could not read index")?;
    let head_tree_id = repo
        .head_tree_id_or_empty()
        .context("could not resolve HEAD tree")?;

    let mut paths = IndexSet::new();
    repo.tree_index_status(
        &head_tree_id,
        &index,
        None,
        gix::status::tree_index::TrackRenames::Disabled,
        |change, _, _| {
            match change {
                ChangeRef::Addition { location, .. }
                | ChangeRef::Deletion { location, .. }
                | ChangeRef::Modification { location, .. } => {
                    paths.insert(PathBuf::from(location.to_str_lossy().into_owned()));
                }
                ChangeRef::Rewrite {
                    source_location,
                    location,
                    ..
                } => {
                    paths.insert(PathBuf::from(source_location.to_str_lossy().into_owned()));
                    paths.insert(PathBuf::from(location.to_str_lossy().into_owned()));
                }
            }

            Ok::<_, std::convert::Infallible>(gix::diff::index::Action::Continue(()))
        },
    )
    .context("could not diff HEAD tree against index")?;

    Ok(paths.into_iter().collect())
}

fn normalize_relative_path(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");

    if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    }
}

fn compile_ast_grep_rules(config: &Config) -> Result<CompiledAstGrepRules> {
    let capacity = config.scope_ast_grep.len();

    let globals = GlobalRules::default();
    let mut ids_to_scope = HashMap::new();
    let mut compiled_rules = Vec::with_capacity(capacity);

    for (index, rule) in config.scope_ast_grep.iter().enumerate() {
        let mut serializable = rule.rule.clone();
        let id = if serializable.id.is_empty() {
            format!("{index:04}-{}", rule.scope)
        } else {
            format!("{index:04}-{}", serializable.id)
        };

        serializable.id = id.clone();
        ids_to_scope.insert(id, rule.scope.clone());
        compiled_rules.push(RuleConfig::try_from(serializable, &globals)?);
    }

    Ok(CompiledAstGrepRules {
        rules: RuleCollection::try_new(compiled_rules)?,
        ids_to_scope,
    })
}

fn detect_ast_grep_scopes(
    config: &Config,
    workdir: &Path,
    changed_paths: &[PathBuf],
) -> Result<Vec<String>> {
    if config.scope_ast_grep.is_empty() {
        return Ok(Vec::new());
    }

    let compiled_rules = compile_ast_grep_rules(config)?;

    let mut matched_scopes = IndexSet::new();

    for relative_path in changed_paths {
        let applicable_rules = compiled_rules.rules.for_path(relative_path);
        if applicable_rules.is_empty() {
            continue;
        }

        let full_path = workdir.join(relative_path);
        let Ok(source) = std::fs::read_to_string(&full_path) else {
            continue;
        };

        for rule in applicable_rules {
            let root = rule.language.ast_grep(&source);
            if root.root().find(&rule.matcher).is_some() {
                if let Some(scope) = compiled_rules.ids_to_scope.get(&rule.id) {
                    matched_scopes.insert(scope.clone());
                }
            }
        }
    }

    Ok(matched_scopes.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CommitScope, Config, ScopePatternValue};
    use indexmap::IndexMap;
    use std::error::Error;

    fn empty_config(workdir: PathBuf) -> Config {
        Config {
            autocomplete: false,
            breaking_changes: false,
            commit_types: IndexMap::new(),
            commit_scopes: IndexMap::new(),
            emoji: false,
            issues: false,
            sign: false,
            force_scope: false,
            allow_empty_scope: true,
            workdir,
            scope_patterns: IndexMap::new(),
            scope_ast_grep: Vec::new(),
        }
    }

    #[test]
    fn test_normalize_relative_path() {
        assert_eq!(
            normalize_relative_path(Path::new("src/lib.rs")),
            "/src/lib.rs"
        );
        assert_eq!(
            normalize_relative_path(Path::new("/src/lib.rs")),
            "/src/lib.rs"
        );
    }

    #[test]
    fn test_validate_scope_patterns_with_glob_fallback() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        let mut config = empty_config(tempdir.path().to_path_buf());
        config.scope_patterns.insert(
            "core".into(),
            ScopePatternValue::Many(vec!["/crates/core/**/*.rs".into(), "^/src/.*\\.rs$".into()]),
        );

        validate_scope_patterns(&config)?;

        tempdir.close()?;
        Ok(())
    }

    #[test]
    fn test_detect_scope_matches_from_staged_paths() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        let repo = git2::Repository::init(tempdir.path())?;

        let mut index = repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let sig = git2::Signature::now("Tester", "test@example.com")?;
        repo.commit(Some("HEAD"), &sig, &sig, "chore: initial", &tree, &[])?;

        let gix_repo = gix::discover(tempdir.path())?;

        std::fs::create_dir_all(tempdir.path().join("crates/core/src"))?;
        std::fs::write(
            tempdir.path().join("crates/core/src/lib.rs"),
            "pub fn core() {}\n",
        )?;

        let mut index = repo.index()?;
        index.add_path(Path::new("crates/core/src/lib.rs"))?;
        index.write()?;

        let mut config = empty_config(tempdir.path().to_path_buf());
        config.commit_scopes.insert(
            "core".into(),
            CommitScope {
                name: "core".into(),
                description: None,
            },
        );
        config.scope_patterns.insert(
            "core".into(),
            ScopePatternValue::One("/crates/core/**/*.rs".into()),
        );

        let matches = detect_scope_matches(&gix_repo, &config)?;
        assert_eq!(matches.suggested(), Some("core".into()));

        tempdir.close()?;
        Ok(())
    }

    #[test]
    fn test_detect_scope_matches_from_ast_grep_rules() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        let repo = git2::Repository::init(tempdir.path())?;
        let gix_repo = gix::discover(tempdir.path())?;

        std::fs::create_dir_all(tempdir.path().join("src"))?;
        std::fs::write(
            tempdir.path().join("src/lib.rs"),
            "#[test]\nfn detects_test_scope() {}\n",
        )?;

        let mut index = repo.index()?;
        index.add_path(Path::new("src/lib.rs"))?;
        index.write()?;

        let mut config = empty_config(tempdir.path().to_path_buf());
        config.scope_ast_grep.push(crate::config::ScopeAstGrepRule {
            scope: "test".into(),
            rule: ast_grep_config::SerializableRuleConfig {
                core: ast_grep_config::SerializableRuleCore {
                    rule: ast_grep_config::from_str("pattern: fn $NAME() {}")?,
                    constraints: None,
                    transform: None,
                    utils: None,
                    fix: None,
                },
                id: String::new(),
                language: SupportLang::Rust,
                rewriters: None,
                message: String::new(),
                note: None,
                severity: ast_grep_config::Severity::Hint,
                labels: None,
                files: None,
                ignores: None,
                url: None,
                metadata: None,
            },
        });

        let matches = detect_scope_matches(&gix_repo, &config)?;
        assert_eq!(matches.suggested(), Some("test".into()));

        tempdir.close()?;
        Ok(())
    }
}
