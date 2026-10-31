//! What a server shows atop its channel list: its banner and its boost goal.

use gpui::SharedString;

/// Boosts that reach level 3. A server with Discord's level 3 override
/// counts none of them.
const LEVEL_THREE: u32 = 14;
/// The boost price of every power-up not included in a level and not behind
/// an experiment (guild tags 3, enhanced role colors 3, the four badge packs
/// 3 + 5 + 3 + 2), as the web client adds them up. The total is inferred
/// from those prices; the client never writes it out.
const POWER_UPS: u32 = 19;

/// The boost goal bar: boosts so far against the goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoostGoal {
    pub count: u32,
    pub goal: u32,
}

impl BoostGoal {
    /// How much of the bar is filled, from 0 to 1.
    pub fn fraction(self) -> f32 {
        if self.goal == 0 { 1. } else { (self.count as f32 / self.goal as f32).min(1.) }
    }
}

/// The parts of the header a server has; none of them needs a request.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuildHeader {
    pub banner: Option<SharedString>,
    /// Only when the server turned its boost goal bar on.
    pub boost: Option<BoostGoal>,
    /// Scheduled events, for the Events row of a later version.
    pub events: usize,
}

impl GuildHeader {
    pub fn new(banner: Option<SharedString>, boosts: u32, bar: bool, level_three_override: bool, events: usize) -> Self {
        let goal = if level_three_override { 0 } else { LEVEL_THREE } + POWER_UPS;
        Self { banner, boost: bar.then_some(BoostGoal { count: boosts, goal }), events }
    }

    /// The header of `guild`, read from its GUILD_CREATE.
    pub fn of(guild: &emerald_discord::model::Guild) -> Self {
        Self::new(
            guild.banner_url().map(Into::into),
            guild.boosts(),
            guild.boost_bar(),
            guild.has_feature("PREMIUM_TIER_3_OVERRIDE"),
            guild.guild_scheduled_events.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_banner_no_bar_means_a_plain_header() {
        let header = GuildHeader::new(None, 5, false, false, 0);
        assert!(header.banner.is_none() && header.boost.is_none());
    }

    #[test]
    fn boost_goal_counts_level_three_and_the_power_ups() {
        let header = GuildHeader::new(None, 14, true, false, 0);
        assert_eq!(header.boost, Some(BoostGoal { count: 14, goal: 33 }));
    }

    #[test]
    fn a_level_three_override_leaves_only_the_power_ups() {
        assert_eq!(GuildHeader::new(None, 3, true, true, 0).boost, Some(BoostGoal { count: 3, goal: 19 }));
    }

    #[test]
    fn the_fill_stops_at_full() {
        assert_eq!(BoostGoal { count: 40, goal: 33 }.fraction(), 1.);
        assert_eq!(BoostGoal { count: 0, goal: 33 }.fraction(), 0.);
    }
}
