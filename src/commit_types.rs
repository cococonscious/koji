use anyhow::{Error, Result};
use linked_hash_map::LinkedHashMap;
use serde_derive::Deserialize;

use crate::config::Config;

#[derive(Clone, Deserialize)]
pub struct CommitType {
    pub name: String,
    pub emoji: String,
    pub description: String,
}

pub fn get_custom_commit_types(config: Config) -> Result<LinkedHashMap<String, CommitType>, Error> {
    let mut map = LinkedHashMap::new();

    for commit_type in config.commit_types.iter() {
        map.insert(commit_type.name.to_owned(), commit_type.to_owned());
    }

    Ok(map)
}

pub fn get_default_commit_types() -> LinkedHashMap<String, CommitType> {
    [
        (
            "feat".into(),
            CommitType {
                name: "feat".into(),
                emoji: "✨".into(),
                description: "A new feature".into(),
            },
        ),
        (
            "fix".into(),
            CommitType {
                name: "fix".into(),
                emoji: "🐛".into(),
                description: "A bug fix".into(),
            },
        ),
        (
            "docs".into(),
            CommitType {
                name: "docs".into(),
                emoji: "📚".into(),
                description: "Documentation only changes".into(),
            },
        ),
        (
            "style".into(),
            CommitType {
                name: "style".into(),
                emoji: "💄".into(),
                description: "Changes that do not affect the meaning of the code".into(),
            },
        ),
        (
            "refactor".into(),
            CommitType {
                name: "refactor".into(),
                emoji: "🔨".into(),
                description: "A code change that neither fixes a bug nor adds a feature".into(),
            },
        ),
        (
            "perf".into(),
            CommitType {
                name: "perf".into(),
                emoji: "⚡".into(),
                description: "A code change that improves performance".into(),
            },
        ),
        (
            "test".into(),
            CommitType {
                name: "test".into(),
                emoji: "🚨".into(),
                description: "Adding missing tests or correcting existing tests".into(),
            },
        ),
        (
            "build".into(),
            CommitType {
                name: "build".into(),
                emoji: "📦".into(),
                description: "Changes that affect the build system or external dependencies".into(),
            },
        ),
        (
            "ci".into(),
            CommitType {
                name: "ci".into(),
                emoji: "🤖".into(),
                description: "Changes to our CI configuration files and scripts".into(),
            },
        ),
        (
            "chore".into(),
            CommitType {
                name: "chore".into(),
                emoji: "🧹".into(),
                description: "Other changes that don't modify src or test files".into(),
            },
        ),
        (
            "revert".into(),
            CommitType {
                name: "revert".into(),
                emoji: "⏪".into(),
                description: "Reverts a previous commit".into(),
            },
        ),
    ]
    .iter()
    .cloned()
    .collect()
}
