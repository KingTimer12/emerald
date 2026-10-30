//! Back and forward through the places the user opened, like a browser's
//! history: the title bar's arrows and their shortcuts walk it.

use emerald_discord::model::Snowflake;

/// A place the user can go back to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// The friends list, Home's landing page.
    Friends,
    Channel(Snowflake),
}

/// The places behind and ahead of the one on screen.
#[derive(Default)]
pub struct NavHistory {
    back: Vec<Place>,
    forward: Vec<Place>,
    current: Option<Place>,
}

impl NavHistory {
    /// Places kept behind the current one; older ones are forgotten.
    pub const LIMIT: usize = 50;

    /// A history whose current place is the one shown at launch.
    pub fn starting_at(place: Place) -> Self {
        Self { current: Some(place), ..Self::default() }
    }

    /// The user opened `place` by choice: it becomes current and whatever was
    /// ahead is dropped. Opening the current place again is not a step.
    pub fn visit(&mut self, place: Place) {
        if self.current == Some(place) {
            return;
        }
        if let Some(previous) = self.current.replace(place) {
            push_bounded(&mut self.back, previous);
        }
        self.forward.clear();
    }

    /// The place before the current one, skipping those `exists` refuses
    /// (deleted, or no longer readable). Walks nothing when none is left.
    pub fn back(&mut self, exists: impl Fn(Place) -> bool) -> Option<Place> {
        let target = take_existing(&mut self.back, &exists)?;
        if let Some(current) = self.current.replace(target) {
            push_bounded(&mut self.forward, current);
        }
        Some(target)
    }

    /// The place after the current one, as `back` in the other direction.
    pub fn forward(&mut self, exists: impl Fn(Place) -> bool) -> Option<Place> {
        let target = take_existing(&mut self.forward, &exists)?;
        if let Some(current) = self.current.replace(target) {
            push_bounded(&mut self.back, current);
        }
        Some(target)
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

fn push_bounded(stack: &mut Vec<Place>, place: Place) {
    if stack.len() == NavHistory::LIMIT {
        stack.remove(0);
    }
    stack.push(place);
}

/// Pops places off `stack` until one still exists; the gone ones are dropped.
fn take_existing(stack: &mut Vec<Place>, exists: &impl Fn(Place) -> bool) -> Option<Place> {
    while let Some(place) = stack.pop() {
        if exists(place) {
            return Some(place);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_and_forward_walk_the_visits() {
        let mut history = NavHistory::default();
        history.visit(Place::Friends);
        history.visit(Place::Channel(Snowflake(1)));
        history.visit(Place::Channel(Snowflake(2)));
        assert_eq!(history.back(|_| true), Some(Place::Channel(Snowflake(1))));
        assert_eq!(history.back(|_| true), Some(Place::Friends));
        assert_eq!(history.back(|_| true), None);
        assert_eq!(history.forward(|_| true), Some(Place::Channel(Snowflake(1))));
    }

    #[test]
    fn a_new_visit_drops_the_forward_stack() {
        let mut history = NavHistory::default();
        history.visit(Place::Friends);
        history.visit(Place::Channel(Snowflake(1)));
        history.back(|_| true);
        history.visit(Place::Channel(Snowflake(3)));
        assert!(!history.can_go_forward());
    }

    #[test]
    fn revisiting_the_current_place_is_not_a_step() {
        let mut history = NavHistory::default();
        history.visit(Place::Friends);
        history.visit(Place::Friends);
        assert!(!history.can_go_back());
    }

    #[test]
    fn back_skips_places_that_no_longer_exist() {
        let mut history = NavHistory::default();
        history.visit(Place::Friends);
        history.visit(Place::Channel(Snowflake(9)));
        history.visit(Place::Channel(Snowflake(2)));
        assert_eq!(history.back(|place| place != Place::Channel(Snowflake(9))), Some(Place::Friends));
    }

    #[test]
    fn a_history_starting_on_friends_can_return_there() {
        let mut history = NavHistory::starting_at(Place::Friends);
        history.visit(Place::Channel(Snowflake(1)));
        assert_eq!(history.back(|_| true), Some(Place::Friends));
    }

    #[test]
    fn history_is_bounded() {
        let mut history = NavHistory::default();
        for id in 0..200 {
            history.visit(Place::Channel(Snowflake(id)));
        }
        let mut steps = 0;
        while history.back(|_| true).is_some() {
            steps += 1;
        }
        assert_eq!(steps, NavHistory::LIMIT);
    }
}
