//! Picking several messages to delete at once, from the message menu. The
//! picks stay until the channel changes, the delete is confirmed or one of
//! the picked messages goes away.

use gpui::{AnyElement, Context, FontWeight, div, prelude::*, rgb};

use crate::app::dialogs::Ask;
use crate::app::widgets::{self, ButtonKind};
use crate::app::Emerald;
use crate::theme;
use crate::time;
use crate::zoom::zoomed;
use emerald_discord::model::{DISCORD_EPOCH_MS, Snowflake};

/// Discord takes at most this many messages in one bulk delete.
pub(in crate::app) const BULK_LIMIT: usize = 100;
/// Discord refuses bulk deletes of messages older than two weeks.
const BULK_MAX_AGE_MS: u64 = 14 * 24 * 60 * 60 * 1000;

/// The messages picked so far, all in one channel, in the order picked.
#[derive(Clone)]
pub(in crate::app) struct Selection {
    pub channel: Snowflake,
    pub messages: Vec<Snowflake>,
}

impl Selection {
    /// Adds `message`, or takes it out when already picked. A full selection
    /// takes no more.
    fn toggle(&mut self, message: Snowflake) {
        match self.messages.iter().position(|picked| *picked == message) {
            Some(ix) => {
                self.messages.remove(ix);
            }
            None if self.messages.len() < BULK_LIMIT => self.messages.push(message),
            None => {}
        }
    }
}

/// Whether Discord accepts `message` in a bulk delete, by its age from the
/// snowflake that carries its creation time.
pub(in crate::app) fn deletable(message: Snowflake) -> bool {
    deletable_at(message, time::now_secs().max(0) as u64 * 1000)
}

/// `deletable` at the instant `now_ms`, in milliseconds since the Unix epoch.
fn deletable_at(message: Snowflake, now_ms: u64) -> bool {
    let created = (message.0 >> 22) + DISCORD_EPOCH_MS;
    now_ms.saturating_sub(created) < BULK_MAX_AGE_MS
}

impl Emerald {
    /// Adds the message to the picks, or takes it out again. Picks in another
    /// channel are dropped first.
    pub(in crate::app) fn toggle_bulk(&mut self, channel: Snowflake, message: Snowflake, cx: &mut Context<Self>) {
        self.message_menu = None;
        if self.bulk.as_ref().is_some_and(|picks| picks.channel != channel) {
            self.bulk = None;
        }
        let picks = self.bulk.get_or_insert_with(|| Selection { channel, messages: Vec::new() });
        picks.toggle(message);
        if picks.messages.is_empty() {
            self.bulk = None;
        }
        cx.notify();
    }

    /// Drops a message that was deleted, so no pick points at nothing.
    pub(in crate::app) fn forget_bulk(&mut self, channel: Snowflake, message: Snowflake) {
        if let Some(picks) = &mut self.bulk {
            if picks.channel == channel {
                picks.messages.retain(|picked| *picked != message);
            }
            if picks.messages.is_empty() {
                self.bulk = None;
            }
        }
    }

    pub(in crate::app) fn is_bulk_picked(&self, channel: Snowflake, message: Snowflake) -> bool {
        self.bulk.as_ref().is_some_and(|picks| picks.channel == channel && picks.messages.contains(&message))
    }

    pub(in crate::app) fn clear_bulk(&mut self, cx: &mut Context<Self>) {
        self.bulk = None;
        cx.notify();
    }

    /// Asks before deleting the picks in the channel on screen.
    pub(in crate::app) fn ask_bulk_delete(&mut self, cx: &mut Context<Self>) {
        let Some(picks) = &self.bulk else { return };
        let ask = Ask::BulkDelete { channel: picks.channel, count: picks.messages.len() };
        self.ask(ask, cx);
    }

    /// Hands over the picks for a confirmed delete in `channel`; none when
    /// they are for another channel, which keeps them.
    pub(in crate::app) fn take_bulk(&mut self, channel: Snowflake) -> Option<Vec<Snowflake>> {
        match self.bulk.take() {
            Some(picks) if picks.channel == channel => Some(picks.messages),
            other => {
                self.bulk = other;
                None
            }
        }
    }

    /// The bar above the message box: how many are picked, and what to do.
    pub(in crate::app) fn render_bulk_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let picks = self.bulk.as_ref().filter(|picks| Some(picks.channel) == self.selected_channel)?;
        let count = picks.messages.len();
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .h(zoomed(40.))
                .px_4()
                .rounded_t_lg()
                .bg(rgb(theme::base_low()))
                .text_size(zoomed(14.))
                .text_color(rgb(theme::text_muted()))
                .child(div().flex_1().min_w(zoomed(0.)).font_weight(FontWeight::MEDIUM).child(t!("bulk.selected", count = count)))
                .child(
                    widgets::button("bulk-clear", t!("bulk.clear"), ButtonKind::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.clear_bulk(cx))),
                )
                .child(
                    widgets::button("bulk-delete", t!("bulk.delete"), ButtonKind::Danger)
                        .on_click(cx.listener(|this, _, _, cx| this.ask_bulk_delete(cx))),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A snowflake created at `ms`, milliseconds since the Unix epoch.
    fn at(ms: u64) -> Snowflake {
        Snowflake((ms - DISCORD_EPOCH_MS) << 22)
    }

    #[test]
    fn messages_under_two_weeks_old_can_be_bulk_deleted() {
        let now = DISCORD_EPOCH_MS + 100 * 24 * 60 * 60 * 1000;
        let day = 24 * 60 * 60 * 1000;
        assert!(deletable_at(at(now - day), now));
        assert!(!deletable_at(at(now - 15 * day), now));
    }

    #[test]
    fn picks_toggle_and_stop_at_the_limit() {
        let mut picks = Selection { channel: Snowflake(1), messages: Vec::new() };
        picks.toggle(Snowflake(10));
        picks.toggle(Snowflake(11));
        picks.toggle(Snowflake(10));
        assert_eq!(picks.messages, vec![Snowflake(11)]);
        for id in 0..BULK_LIMIT as u64 {
            picks.toggle(Snowflake(100 + id));
        }
        assert_eq!(picks.messages.len(), BULK_LIMIT);
        picks.toggle(Snowflake(999));
        assert!(!picks.messages.contains(&Snowflake(999)));
    }
}
