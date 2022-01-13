fn replace_emoji_shortcodes(mut string: String) -> String {
    for emoji in emojis::iter() {
        if let Some(shortcode) = emoji.shortcode() {
            string = string.replace(&format!(":{shortcode}:"), emoji.as_str());
        }
    }

    string
}

pub trait ReplaceEmoji {
    fn replace_emoji_shortcodes(&self) -> String;
}

impl ReplaceEmoji for &str {
    fn replace_emoji_shortcodes(&self) -> String {
        replace_emoji_shortcodes(self.to_string())
    }
}

impl ReplaceEmoji for String {
    fn replace_emoji_shortcodes(&self) -> String {
        replace_emoji_shortcodes(self.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_emoji_shortcodes() {
        let phrase = "yes sir :pinched_fingers: !";

        assert_eq!(replace_emoji_shortcodes(phrase.to_string()), "yes sir 🤌 !");

        assert_eq!(phrase.replace_emoji_shortcodes(), "yes sir 🤌 !");

        assert_eq!(
            phrase.to_string().replace_emoji_shortcodes(),
            "yes sir 🤌 !"
        );
    }
}
