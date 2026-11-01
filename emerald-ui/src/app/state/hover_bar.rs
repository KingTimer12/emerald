//! The 2026 hover bar of a message: which buttons it shows, in the web
//! client's order, normally and while Shift is held, and where "Mark
//! Unread" puts the read marker.

use emerald_discord::model::Snowflake;

/// A button of the hover bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarButton {
    /// One of the quick reactions, by its place in the list.
    Quick(usize),
    Divider,
    AddReaction,
    Reply,
    Edit,
    Forward,
    Thread,
    More,
    CopyId,
    CopyLink,
    MarkUnread,
    Bookmark,
    Pin,
    Delete,
}

/// What the user may do to the message, as far as the bar goes.
#[derive(Clone, Copy, Debug, Default)]
pub struct BarRights {
    pub react: bool,
    pub reply: bool,
    pub edit: bool,
    pub forward: bool,
    pub thread: bool,
    pub pin: bool,
    pub delete: bool,
    /// Off when the message has a reminder: bookmarks and reminders
    /// replace each other.
    pub bookmark: bool,
    /// Off in threads not joined or archived, where marking unread would
    /// first need writes the web client makes.
    pub mark_unread: bool,
}

/// The bar's buttons left to right. `quick` is how many quick reactions
/// there are; `own` the user's own message; `expanded` while Shift is held.
pub fn buttons(own: bool, expanded: bool, rights: BarRights, quick: usize) -> Vec<BarButton> {
    use BarButton::*;
    let mut bar = Vec::new();
    if expanded {
        bar.extend([CopyId, CopyLink]);
        if rights.mark_unread {
            bar.push(MarkUnread);
        }
        if rights.bookmark {
            bar.push(Bookmark);
        }
        if rights.pin {
            bar.push(Pin);
        }
        // Creating a thread moves here when Forward takes its place.
        if rights.thread && rights.forward {
            bar.push(Thread);
        }
        if own && rights.reply {
            bar.push(Reply);
        }
    } else if rights.react && quick > 0 {
        bar.extend((0..quick).map(Quick));
        bar.push(Divider);
    }
    if rights.react {
        bar.push(AddReaction);
    }
    if !own && rights.reply {
        bar.push(Reply);
    }
    if own && rights.edit {
        bar.push(Edit);
    }
    if rights.forward {
        bar.push(Forward);
    }
    if rights.thread && !rights.forward {
        bar.push(Thread);
    }
    bar.push(if expanded && rights.delete { Delete } else { More });
    bar
}

/// Shift as remembered once the window is, or is not, in front: released
/// keys go unseen while it is in the back, so it is forgotten there.
pub fn shift_after(held: bool, window_active: bool) -> bool {
    held && window_active
}

/// Whether the bar shows its Shift form: only while the window is in
/// front and no menu or picker of the message is open.
pub fn expanded(shift: bool, window_active: bool, popout_open: bool) -> bool {
    shift && window_active && !popout_open
}

/// The message "Mark Unread" acknowledges: the loaded one just before
/// `message`, else the id one millisecond earlier, as the web client does.
/// `loaded` is the channel's loaded messages, oldest first.
pub fn unread_from(loaded: &[Snowflake], message: Snowflake) -> Snowflake {
    loaded
        .iter()
        .rev()
        .find(|id| **id < message)
        .copied()
        .unwrap_or_else(|| previous_millisecond(message))
}

/// The smallest id of the millisecond before `id`'s.
fn previous_millisecond(id: Snowflake) -> Snowflake {
    let ms = (id.0 >> 22).saturating_sub(1);
    Snowflake(ms << 22)
}

/// A message link as Discord's "Copy Link" writes it.
pub fn message_link(guild: Option<Snowflake>, channel: Snowflake, message: Snowflake) -> String {
    let guild = guild.map_or_else(|| "@me".to_owned(), |g| g.to_string());
    format!("{}/channels/{guild}/{channel}/{message}", emerald_discord::urls::WEB)
}

#[cfg(test)]
mod tests {
    use super::*;
    use BarButton::*;

    fn rights() -> BarRights {
        BarRights {
            react: true,
            reply: true,
            edit: true,
            forward: true,
            thread: true,
            pin: true,
            delete: true,
            bookmark: true,
            mark_unread: true,
        }
    }

    #[test]
    fn others_normal_and_expanded() {
        assert_eq!(
            buttons(false, false, rights(), 3),
            [Quick(0), Quick(1), Quick(2), Divider, AddReaction, Reply, Forward, More]
        );
        assert_eq!(
            buttons(false, true, rights(), 3),
            [CopyId, CopyLink, MarkUnread, Bookmark, Pin, Thread, AddReaction, Reply, Forward, Delete]
        );
    }

    #[test]
    fn own_normal_and_expanded() {
        assert_eq!(buttons(true, false, rights(), 0), [AddReaction, Edit, Forward, More]);
        assert_eq!(
            buttons(true, true, rights(), 2),
            [CopyId, CopyLink, MarkUnread, Bookmark, Pin, Thread, Reply, AddReaction, Edit, Forward, Delete]
        );
    }

    #[test]
    fn thread_stays_in_the_bar_without_forward() {
        let no_forward = BarRights { forward: false, delete: false, ..rights() };
        assert_eq!(buttons(false, false, no_forward, 0), [AddReaction, Reply, Thread, More]);
        assert_eq!(*buttons(false, true, no_forward, 0).last().unwrap(), More);
    }

    #[test]
    fn shift_form_only_in_front_with_nothing_open() {
        assert!(expanded(true, true, false));
        assert!(!expanded(true, false, false));
        assert!(!expanded(true, true, true));
        assert!(!expanded(false, true, false));
    }

    #[test]
    fn shift_is_forgotten_when_the_window_goes_to_the_back() {
        assert!(!shift_after(true, false));
        assert!(shift_after(true, true));
    }

    #[test]
    fn mark_unread_hides_where_the_menu_hides_it() {
        let no_unread = BarRights { mark_unread: false, ..rights() };
        assert!(!buttons(false, true, no_unread, 0).contains(&MarkUnread));
    }

    #[test]
    fn mark_unread_acks_the_message_before() {
        let loaded = [Snowflake(10 << 22), Snowflake(20 << 22), Snowflake(30 << 22)];
        assert_eq!(unread_from(&loaded, Snowflake(30 << 22)), Snowflake(20 << 22));
        assert_eq!(unread_from(&loaded, Snowflake(10 << 22)), Snowflake(9 << 22));
    }

    #[test]
    fn links_name_the_guild_or_me() {
        assert_eq!(
            message_link(Some(Snowflake(1)), Snowflake(2), Snowflake(3)),
            "https://discord.com/channels/1/2/3"
        );
        assert_eq!(message_link(None, Snowflake(2), Snowflake(3)), "https://discord.com/channels/@me/2/3");
    }
}
