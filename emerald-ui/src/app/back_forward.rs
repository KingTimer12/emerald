//! Back and forward: the places opened, walked like a browser's history by
//! the title bar's arrows and their shortcuts.

use gpui::{Context, Window};

use super::Emerald;
use super::state::VisitedPlace;

impl Emerald {
    /// Shows Home's friends list, a place back and forward can return to.
    pub(super) fn show_friends(&mut self, cx: &mut Context<Self>) {
        self.friends_open = true;
        self.requests_open = false;
        if !self.walking_history {
            self.places.visit(VisitedPlace::Friends);
        }
        cx.notify();
    }

    /// Returns to the place before this one, skipping channels that are gone.
    pub(super) fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut places = std::mem::take(&mut self.places);
        let target = places.back(|place| self.place_exists(place));
        self.places = places;
        self.walk_to(target, window, cx);
    }

    /// Goes ahead again after `go_back`.
    pub(super) fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut places = std::mem::take(&mut self.places);
        let target = places.forward(|place| self.place_exists(place));
        self.places = places;
        self.walk_to(target, window, cx);
    }

    fn place_exists(&self, place: VisitedPlace) -> bool {
        match place {
            VisitedPlace::Friends => true,
            VisitedPlace::Channel(channel) => self.reachable_space(channel).is_some(),
        }
    }

    fn walk_to(&mut self, target: Option<VisitedPlace>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = target else { return };
        self.walking_history = true;
        match target {
            VisitedPlace::Friends => self.select_space(0, window, cx),
            VisitedPlace::Channel(channel) => self.open_channel_link(None, channel, None, window, cx),
        }
        self.walking_history = false;
    }
}
