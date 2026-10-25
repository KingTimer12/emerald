//! The typing indicator under the message box.

use gpui::{FontWeight, HighlightStyle, StyledText};

use super::Emerald;

impl Emerald {
    /// "Paulo is typing…", in the web client's wording.
    pub(super) fn typing_text(&self) -> Option<StyledText> {
        let channel = self.selected_channel?;
        // Nicknames first, as the web client names typists in guilds.
        let guild = self.spaces.current().info();
        let names: Vec<&str> = self
            .typing
            .who(channel)
            .map(|user| {
                guild
                    .and_then(|g| g.member_style(user).0)
                    .or_else(|| self.people.display_name(user))
                    .unwrap_or(t!("common.someone"))
            })
            .collect();
        let (text, bold): (String, Vec<std::ops::Range<usize>>) = match names.as_slice() {
            [] => return None,
            [one] => t_spans!("chat.typing_one", name = one),
            [a, b] => t_spans!("chat.typing_two", a = a, b = b),
            _ => (t!("chat.typing_many").to_owned(), Vec::new()),
        };
        Some(StyledText::new(text).with_highlights(bold.into_iter().map(|range| {
            (
                range,
                HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..Default::default()
                },
            )
        })))
    }
}
