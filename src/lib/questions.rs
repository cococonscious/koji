use linked_hash_map::LinkedHashMap;

use crate::commit_types::CommitType;

/// Format the commit type choices.
pub fn render_commit_type_choice(
    use_emoji: bool,
    commit_type: &CommitType,
    commit_types: &LinkedHashMap<String, CommitType>,
) -> String {
    let name = &commit_type.name;
    let description = &commit_type.description;
    let use_emoji = use_emoji && commit_type.emoji.is_some();

    let emoji = if use_emoji {
        format!("{} ", commit_type.emoji.as_ref().unwrap())
    } else {
        "".into()
    };

    let width = commit_types
        .iter()
        .map(|(key, _)| key.chars().count())
        .max()
        .unwrap()
        - commit_type.name.chars().count()
        + if use_emoji { 5 } else { 3 };

    format!("{}:{:>width$}{}", name, emoji, description, width = width)
}

#[cfg(test)]
mod tests {
    use crate::{commit_types::get_commit_types, config::load_config};

    use super::*;

    #[test]
    fn test_render_commit_type_choice() {
        let config = load_config(None).unwrap();
        let commit_types = get_commit_types(&config);

        let choice =
            render_commit_type_choice(true, commit_types.get("refactor").unwrap(), &commit_types);

        assert_eq!(
            choice,
            "refactor:   🔨 A code change that neither fixes a bug nor adds a feature"
        );

        let choice =
            render_commit_type_choice(true, commit_types.get("ci").unwrap(), &commit_types);

        assert_eq!(
            choice,
            "ci:         🤖 Changes to our CI configuration files and scripts"
        );
    }

    #[test]
    fn test_render_commit_type_choice_with_emoji() {
        let config = load_config(None).unwrap();
        let commit_types = get_commit_types(&config);

        let choice =
            render_commit_type_choice(true, commit_types.get("refactor").unwrap(), &commit_types);

        assert_eq!(
            choice,
            "refactor:   🔨 A code change that neither fixes a bug nor adds a feature"
        );

        let choice =
            render_commit_type_choice(true, commit_types.get("ci").unwrap(), &commit_types);

        assert_eq!(
            choice,
            "ci:         🤖 Changes to our CI configuration files and scripts"
        );
    }
}
