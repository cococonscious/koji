pub trait ReplaceEmoji {
    fn replace_emoji_shortcodes(&self) -> String;
}

impl ReplaceEmoji for &str {
    fn replace_emoji_shortcodes(&self) -> String {
        let mut fixed = self.to_string();

        for emoji in emojis::iter() {
            if let Some(shortcode) = emoji.shortcode() {
                fixed = fixed.replace(&format!(":{}:", shortcode), emoji.as_str());
            }
        }

        fixed
    }
}

impl ReplaceEmoji for String {
    fn replace_emoji_shortcodes(&self) -> String {
        let mut fixed = self.to_owned();

        for emoji in emojis::iter() {
            if let Some(shortcode) = emoji.shortcode() {
                fixed = fixed.replace(&format!(":{}:", shortcode), emoji.as_str());
            }
        }

        fixed
    }
}
