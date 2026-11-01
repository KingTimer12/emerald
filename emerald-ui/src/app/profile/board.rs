//! The Board ("Mural") of the full profile: the game widgets a profile
//! carries, with covers from `GET /games`. Kinds Emerald does not draw are
//! skipped.

use super::*;
use emerald_discord::model::{Game, Widget};

/// Games kept for Boards; the cache is emptied past this.
const MAX_GAMES: usize = 200;
/// Cover size on the Board.
const COVER: f32 = 64.;

/// The widget kinds the Board draws, with their title's key.
fn title(kind: &str) -> Option<&'static str> {
    match kind {
        "favorite_games" => Some("profile.favorite_game"),
        "current_games" => Some("profile.current_games"),
        "want_to_play_games" => Some("profile.want_to_play"),
        "played_games" => Some("profile.played_games"),
        _ => None,
    }
}

impl Emerald {
    /// Asks for the games `profile`'s Board names that are not known yet.
    pub(in super::super) fn ask_games(&mut self, profile: &Profile) {
        let missing: Vec<Snowflake> = profile
            .widgets
            .iter()
            .filter(|w| title(w.kind()).is_some())
            .flat_map(Widget::game_ids)
            .filter(|id| !self.games.contains_key(id) && self.games_asked.insert(*id))
            .collect();
        if !missing.is_empty() {
            self.net.post(Command::Games { ids: missing });
        }
    }

    pub(in super::super) fn on_games(&mut self, games: Vec<Game>) {
        if self.games.len() + games.len() > MAX_GAMES {
            self.games.clear();
            self.games_asked.clear();
        }
        self.games.extend(games.into_iter().map(|game| (game.id, game)));
    }

    /// Whether `profile` has anything the Board can draw.
    pub(super) fn has_board(&self, profile: Option<&Profile>) -> bool {
        profile.is_some_and(|p| p.widgets.iter().any(|w| title(w.kind()).is_some()))
    }

    /// One block per game widget: its title, then each game's cover and name.
    pub(super) fn board(&self, profile: Option<&Profile>) -> Div {
        let widgets = profile.map_or(&[][..], |p| p.widgets.as_slice());
        div().flex().flex_col().gap_4().children(widgets.iter().filter_map(|widget| {
            let title = title(widget.kind())?;
            let games = widget.game_ids();
            Some(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded(theme::radius::card())
                    .bg(theme::hover())
                    .child(
                        div()
                            .text_size(zoomed(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::text_default()))
                            .child(emerald_core::i18n::text(title)),
                    )
                    .child(div().flex().flex_wrap().gap_3().children(games.into_iter().map(|id| self.game_tile(id)))),
            )
        }))
    }

    fn game_tile(&self, id: Snowflake) -> Div {
        let game = self.games.get(&id);
        let cover = game
            .and_then(|g| g.cover_url((COVER * 2.) as u32))
            .and_then(|url| self.avatar_image(&url));
        div()
            .w(zoomed(COVER * 1.5))
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .w(zoomed(COVER * 1.5))
                    .h(zoomed(COVER * 2.))
                    .rounded(theme::radius::control())
                    .overflow_hidden()
                    .bg(theme::hover())
                    .children(cover.map(|image| img(image).size_full().object_fit(ObjectFit::Cover))),
            )
            .child(
                div()
                    .text_size(zoomed(12.))
                    .line_clamp(2)
                    .text_color(rgb(theme::text_default()))
                    .child(game.map(|g| g.name.clone()).unwrap_or_default()),
            )
    }
}
