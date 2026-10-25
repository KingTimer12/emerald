//! Messages from blocked or ignored users, folded into one bar per run of
//! them as the web client does, until the reader asks to see them.

use emerald_discord::model::Snowflake;

/// How the chat draws the message at some index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fold {
    /// Drawn as usual.
    Shown,
    /// The first of `count` hidden messages in a row: drawn as the bar.
    Head { count: usize },
    /// Hidden behind the bar of an earlier message.
    Folded,
}

/// What the message at `ix` is, among `len` messages where `message`
/// gives each one's id and author, `hidden` tells whose are folded and
/// `revealed` which runs (by their first message) were opened.
pub fn fold(
    ix: usize,
    len: usize,
    message: impl Fn(usize) -> (Snowflake, Snowflake),
    hidden: impl Fn(Snowflake) -> bool,
    revealed: impl Fn(Snowflake) -> bool,
) -> Fold {
    let hides = |ix: usize| hidden(message(ix).1);
    if !hides(ix) {
        return Fold::Shown;
    }
    let mut head = ix;
    while head > 0 && hides(head - 1) {
        head -= 1;
    }
    if revealed(message(head).0) {
        return Fold::Shown;
    }
    if head != ix {
        return Fold::Folded;
    }
    let count = (ix..len).take_while(|ix| hides(*ix)).count();
    Fold::Head { count }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_fold_behind_their_first_message() {
        let blocked = Snowflake(9);
        let messages: Vec<_> = [1, 9, 9, 1, 9].iter().enumerate().map(|(ix, a)| (Snowflake(ix as u64), Snowflake(*a))).collect();
        let fold_at = |ix, open: Option<u64>| {
            fold(ix, messages.len(), |ix| messages[ix], |a| a == blocked, |m| Some(m.0) == open)
        };
        assert_eq!(fold_at(0, None), Fold::Shown);
        assert_eq!(fold_at(1, None), Fold::Head { count: 2 });
        assert_eq!(fold_at(2, None), Fold::Folded);
        assert_eq!(fold_at(4, None), Fold::Head { count: 1 });
        assert_eq!(fold_at(2, Some(1)), Fold::Shown);
        assert_eq!(fold_at(4, Some(1)), Fold::Head { count: 1 });
    }
}
