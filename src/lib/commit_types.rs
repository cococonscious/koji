use linked_hash_map::LinkedHashMap;
use serde::Deserialize;

use crate::config::Config;

#[derive(Clone, Deserialize)]
pub struct CommitType {
    pub name: String,
    pub emoji: Option<String>,
    pub description: String,
}

/// Get custom commit types from the given config.
pub fn get_custom_commit_types(config: Config) -> LinkedHashMap<String, CommitType> {
    let mut map = LinkedHashMap::new();

    for commit_type in config.commit_types.iter() {
        map.insert(commit_type.name.to_owned(), commit_type.to_owned());
    }

    map
}

/// Get the default commit types.
pub fn get_default_commit_types() -> LinkedHashMap<String, CommitType> {
    [
        (
            "feat".into(),
            CommitType {
                name: "feat".into(),
                emoji: Some("✨".into()),
                description: "A new feature".into(),
            },
        ),
        (
            "fix".into(),
            CommitType {
                name: "fix".into(),
                emoji: Some("🐛".into()),
                description: "A bug fix".into(),
            },
        ),
        (
            "docs".into(),
            CommitType {
                name: "docs".into(),
                emoji: Some("📚".into()),
                description: "Documentation only changes".into(),
            },
        ),
        (
            "style".into(),
            CommitType {
                name: "style".into(),
                emoji: Some("💄".into()),
                description: "Changes that do not affect the meaning of the code".into(),
            },
        ),
        (
            "refactor".into(),
            CommitType {
                name: "refactor".into(),
                emoji: Some("🔨".into()),
                description: "A code change that neither fixes a bug nor adds a feature".into(),
            },
        ),
        (
            "perf".into(),
            CommitType {
                name: "perf".into(),
                emoji: Some("⚡".into()),
                description: "A code change that improves performance".into(),
            },
        ),
        (
            "test".into(),
            CommitType {
                name: "test".into(),
                emoji: Some("🚨".into()),
                description: "Adding missing tests or correcting existing tests".into(),
            },
        ),
        (
            "build".into(),
            CommitType {
                name: "build".into(),
                emoji: Some("📦".into()),
                description: "Changes that affect the build system or external dependencies".into(),
            },
        ),
        (
            "ci".into(),
            CommitType {
                name: "ci".into(),
                emoji: Some("🤖".into()),
                description: "Changes to our CI configuration files and scripts".into(),
            },
        ),
        (
            "chore".into(),
            CommitType {
                name: "chore".into(),
                emoji: Some("🧹".into()),
                description: "Other changes that don't modify src or test files".into(),
            },
        ),
        (
            "revert".into(),
            CommitType {
                name: "revert".into(),
                emoji: Some("⏪".into()),
                description: "Reverts a previous commit".into(),
            },
        ),
    ]
    .iter()
    .cloned()
    .collect()
}
