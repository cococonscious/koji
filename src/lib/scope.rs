use anyhow::{Context, Result};
use gix::bstr::ByteSlice;
use gix::diff::index::ChangeRef;
use gix::Repository;
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
            [first, rest @ ..] => {
                // Warn the user that they have multiple matches (bad)
                if !rest.is_empty() {
                    eprintln!("More than one match was found. First: {first:?} Extra: {rest:?}");
                }
                Some(first.clone())
            }
            _ => None,
        }
    }
}

struct CompiledAstGrepRules {
    rules: RuleCollection<SupportLang>,
    ids_to_scope: HashMap<String, String>,
}

impl Config {
    pub fn validate_scope_patterns(&self) -> Result<()> {
        for (scope_name, scope) in &self.commit_scopes {
            if let Some(patterns) = &scope.patterns {
                for pattern in patterns.iter() {
                    Regex::new(pattern).with_context(|| {
                        format!("invalid scope path regex for `{scope_name}`: `{pattern}`")
                    })?;
                }
            }
        }
        Ok(())
    }

    pub fn validate_ast_grep_rules(&self) -> Result<()> {
        self.compile_ast_grep_rules().map(|_| ())
    }

    fn compile_ast_grep_rules(&self) -> Result<CompiledAstGrepRules> {
        let globals = GlobalRules::default();
        let mut ids_to_scope = HashMap::new();
        let mut compiled_rules = Vec::new();

        for (index, (scope_name, scope)) in self.commit_scopes.iter().enumerate() {
            let Some(ast_grep) = &scope.ast_grep else {
                continue;
            };
            let mut serializable = ast_grep.clone();
            let id = if serializable.id.is_empty() {
                format!("{index:04}-{scope_name}")
            } else {
                format!("{index:04}-{}", serializable.id)
            };

            serializable.id = id.clone();
            ids_to_scope.insert(id, scope_name.clone());
            compiled_rules.push(RuleConfig::try_from(serializable, &globals)?);
        }

        Ok(CompiledAstGrepRules {
            rules: RuleCollection::try_new(compiled_rules)?,
            ids_to_scope,
        })
    }
}

/// Stage all tracked modified/deleted files so
/// that scope detection sees the full, entire diff when `--all` is passed.
pub fn stage_tracked_changes(repo: &Repository) -> Result<()> {
    let workdir = repo
        .workdir()
        .context("repository has no working directory")?;

    let mut index = repo.open_index().context("could not open index")?;

    let (entries, backing) = index.entries_mut_and_pathbacking();
    for entry in entries.iter_mut() {
        let path = entry.path_in(backing);
        let full_path = workdir.join(gix::path::from_bstr(path));

        match gix::index::fs::Metadata::from_path_no_follow(&full_path) {
            Err(_) => {
                // File has left the worktree mark for removal.
                entry.flags.insert(gix::index::entry::Flags::REMOVE);
            }
            Ok(meta) => {
                let new_stat =
                    gix::index::entry::Stat::from_fs(&meta).context("could not read file stat")?;
                // Only re-hash if mtime or size changed.
                if new_stat.mtime.secs != entry.stat.mtime.secs || new_stat.size != entry.stat.size
                {
                    let content = std::fs::read(&full_path)
                        .with_context(|| format!("could not read `{}`", full_path.display()))?;
                    let oid = repo
                        .write_blob(&content)
                        .context("could not write blob")?
                        .detach();
                    entry.id = oid;
                    entry.stat = new_stat;
                }
            }
        }
    }

    index.remove_entries(|_, _, e| e.flags.contains(gix::index::entry::Flags::REMOVE));
    index
        .write(gix::index::write::Options::default())
        .context("could not write index")?;

    Ok(())
}

pub fn detect_scope_matches(repo: &Repository, config: &Config) -> Result<ScopeMatches> {
    let changed_paths = staged_paths(repo)?;
    if changed_paths.is_empty() {
        return Ok(ScopeMatches::default());
    }

    let mut matched_scopes = IndexSet::new();

    for relative_path in &changed_paths {
        let normalized_path = normalize_relative_path(relative_path);

        for (scope_name, scope) in &config.commit_scopes {
            if let Some(patterns) = &scope.patterns {
                let is_match = patterns
                    .iter()
                    .map(Regex::new)
                    .collect::<Result<Vec<_>, _>>()?
                    .iter()
                    .any(|re| re.is_match(&normalized_path));

                if is_match {
                    matched_scopes.insert(scope_name.clone());
                }
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

fn detect_ast_grep_scopes(
    config: &Config,
    workdir: &Path,
    changed_paths: &[PathBuf],
) -> Result<Vec<String>> {
    if config.commit_scopes.values().all(|s| s.ast_grep.is_none()) {
        return Ok(Vec::new());
    }

    let compiled_rules = config.compile_ast_grep_rules()?;

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
            force_config_scopes: false,
            allow_empty_scope: true,
            workdir,
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
    fn test_validate_scope_patterns() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        let mut config = empty_config(tempdir.path().to_path_buf());
        config.commit_scopes.insert(
            "core".into(),
            CommitScope {
                name: "core".into(),
                description: None,
                patterns: Some(ScopePatternValue::Many(vec![
                    "^/crates/core/.*\\.rs$".into(),
                    "^/src/.*\\.rs$".into(),
                ])),
                ast_grep: None,
            },
        );

        config.validate_scope_patterns()?;

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
                patterns: Some(ScopePatternValue::One("^/crates/core/.*\\.rs$".into())),
                ast_grep: None,
            },
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
        config.commit_scopes.insert(
            "test".into(),
            CommitScope {
                name: "test".into(),
                description: None,
                patterns: None,
                ast_grep: Some(ast_grep_config::SerializableRuleConfig {
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
                }),
            },
        );

        let matches = detect_scope_matches(&gix_repo, &config)?;
        assert_eq!(matches.suggested(), Some("test".into()));

        tempdir.close()?;
        Ok(())
    }
}
