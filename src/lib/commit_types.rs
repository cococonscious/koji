use indexmap::IndexMap;
use serde::Deserialize;

use crate::config::Config;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CommitType {
    pub name: String,
    pub emoji: Option<String>,
    pub description: String,
}

/// Get commit types from config.
pub fn get_commit_types(config: &Config) -> IndexMap<String, CommitType> {
    let mut map = IndexMap::new();

    for commit_type in config.commit_types.iter() {
        map.insert(commit_type.name.to_owned(), commit_type.to_owned());
    }

    map
}

#[cfg(test)]
mod tests {
    use crate::config::load_config;

    use super::*;

    #[test]
    fn test_get_commit_types() {
        let config = load_config(None).unwrap();
        let commit_types = get_commit_types(&config);

        assert_eq!(
            commit_types.get("feat"),
            Some(&CommitType {
                name: "feat".into(),
                emoji: Some("✨".into()),
                description: "A new feature".into()
            })
        )
    }
}
