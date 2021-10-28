use linked_hash_map::LinkedHashMap;
use serde::Deserialize;

use crate::config::Config;

#[derive(Clone, Deserialize)]
pub struct CommitType {
    pub name: String,
    pub emoji: Option<String>,
    pub description: String,
}

/// Get commit types from config.
pub fn get_commit_types(config: Config) -> LinkedHashMap<String, CommitType> {
    let mut map = LinkedHashMap::new();

    for commit_type in config.commit_types.iter() {
        map.insert(commit_type.name.to_owned(), commit_type.to_owned());
    }

    map
}
