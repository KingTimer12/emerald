//! Small decisions of the 2026 profile cards: how many badges fit before
//! "+N", and the line of friends and servers in common.

/// Badges drawn before the rest fold into "+N".
pub const MAX_BADGES: usize = 6;

/// How many of `total` badges are drawn, and how many the "+N" stands for.
pub fn badge_overflow(total: usize) -> (usize, usize) {
    let shown = total.min(MAX_BADGES);
    (shown, total - shown)
}

/// "3 Mutual Friends • 2 Mutual Servers", leaving out a part that is zero;
/// `None` when nothing is shared.
pub fn mutual_line(friends: usize, servers: usize) -> Option<String> {
    let friends = (friends > 0).then(|| {
        if friends == 1 {
            t!("profile.mutual_friends_one", count = friends)
        } else {
            t!("profile.mutual_friends_many", count = friends)
        }
    });
    let servers = (servers > 0).then(|| {
        if servers == 1 {
            t!("profile.mutual_servers_one", count = servers)
        } else {
            t!("profile.mutual_servers_many", count = servers)
        }
    });
    match (friends, servers) {
        (Some(friends), Some(servers)) => Some(format!("{friends} \u{2022} {servers}")),
        (one, other) => one.or(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_badges_show_six_and_plus_one() {
        assert_eq!(badge_overflow(7), (6, 1));
        assert_eq!(badge_overflow(3), (3, 0));
    }

    #[test]
    fn mutual_line_leaves_out_what_is_not_shared() {
        assert_eq!(mutual_line(0, 0), None);
        assert_eq!(mutual_line(1, 0).as_deref(), Some("1 Mutual Friend"));
        assert_eq!(mutual_line(3, 2).as_deref(), Some("3 Mutual Friends \u{2022} 2 Mutual Servers"));
        assert_eq!(mutual_line(0, 1).as_deref(), Some("1 Mutual Server"));
    }
}
