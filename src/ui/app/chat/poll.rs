//! Polls: the question, the answers to pick from, and the results once
//! voted, closed or asked for. Voting sends the whole choice at once, as
//! the web client does; the counts follow the gateway's vote events.

use gpui::{AnyElement, Context, FontWeight, SharedString, div, img, prelude::*, px, relative, rgb};

use super::super::super::{theme, time};
use super::super::state::Msg;
use super::super::Emerald;
use crate::discord::model::{Poll, Snowflake, custom_url, iso_secs};
use crate::discord::{Action, Command};

/// Results asked for by "Show results" are remembered for this many polls.
const MAX_SHOWN_RESULTS: usize = 64;

impl Emerald {
    pub(super) fn render_poll(&self, poll: &Poll, msg: &Msg, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let message = msg.id;
        let results = poll.results.clone().unwrap_or_default();
        let expiry = poll.expiry.as_deref().and_then(iso_secs);
        let now = time::now_secs();
        let closed = results.is_finalized || expiry.is_some_and(|at| at <= now);
        let voted = results.answer_counts.iter().any(|c| c.me_voted);
        let showing = closed || voted || self.poll_results.contains(&message);
        let total: u32 = results.answer_counts.iter().map(|c| c.count).sum();
        let chosen: &[u32] = match &self.poll_choice {
            Some((choice_message, answers)) if *choice_message == message => answers,
            _ => &[],
        };
        let channel = self.selected_channel;
        let can_vote = !closed && !msg.pending && channel.is_some();

        let answers = poll.answers.iter().map(|answer| {
            let id = answer.answer_id;
            let count = results.answer_counts.iter().find(|c| c.id == id);
            let votes = count.map_or(0, |c| c.count);
            let mine = count.is_some_and(|c| c.me_voted);
            let picked = chosen.contains(&id);
            let share = if total > 0 { votes as f32 / total as f32 } else { 0. };
            let emoji = answer.poll_media.emoji.as_ref().and_then(|emoji| match emoji.id {
                Some(id) => self
                    .avatar_image(&custom_url(id))
                    .map(|image| img(image).size(px(20.)).into_any_element()),
                None => emoji.name.clone().map(|name| div().child(name).into_any_element()),
            });
            let text: SharedString = answer.poll_media.text.clone().unwrap_or_default().into();
            let row = div()
                .id(("poll-answer", ix * 16 + id as usize))
                .relative()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_md()
                .overflow_hidden()
                .border_1()
                .border_color(if picked || mine { rgb(theme::BRAND) } else { theme::border() })
                .bg(rgb(theme::BASE_LOWER));
            let row = if showing {
                // The share of votes fills the row from the left.
                row.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left_0()
                        .w(relative(share))
                        .bg(if mine { theme::reaction_mine() } else { theme::hover() }),
                )
            } else {
                row
            };
            let row = row
                .children(emoji)
                .child(div().relative().flex_1().min_w(px(0.)).truncate().child(text))
                .when(showing, |row| {
                    row.child(
                        div()
                            .relative()
                            .flex_none()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{}%", (share * 100.).round())),
                    )
                    .child(div().relative().flex_none().text_xs().text_color(theme::muted()).child(vote_count(votes)))
                })
                .when(!showing, |row| row.child(check_box(picked, poll.allow_multiselect)));
            if showing || !can_vote {
                row.into_any_element()
            } else {
                let multiple = poll.allow_multiselect;
                row.cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::SURFACE_HIGH)))
                    .on_click(cx.listener(move |this, _, _, cx| this.pick_answer(message, id, multiple, cx)))
                    .into_any_element()
            }
        });

        let status: SharedString = if closed {
            t!("poll.closed").into()
        } else {
            match expiry {
                Some(at) => t!("poll.left", time = time::duration((at - now).max(0) as u64)).into(),
                None => SharedString::default(),
            }
        };
        let footer_text = [vote_count(total).to_string(), status.to_string()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" • ");
        let busy = self.busy.contains(&Action::Message);
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .px_4()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT_STRONG))
                .bg(if primary { rgb(theme::BRAND) } else { theme::hover() })
                .child(label)
        };
        let actions = channel.filter(|_| can_vote).map(|channel| {
            let choice: Vec<u32> = chosen.to_vec();
            let row = div().flex().gap_2();
            if voted {
                row.child(button("poll-remove", t!("poll.remove_vote"), false).on_click(cx.listener(
                    move |this, _, _, cx| this.vote(channel, message, Vec::new(), cx),
                )))
            } else if showing {
                row.child(button("poll-back", t!("poll.hide_results"), false).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.poll_results.remove(&message);
                        cx.notify();
                    },
                )))
            } else {
                row.child(button("poll-show", t!("poll.show_results"), false).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if this.poll_results.len() >= MAX_SHOWN_RESULTS {
                            this.poll_results.clear();
                        }
                        this.poll_results.insert(message);
                        cx.notify();
                    },
                )))
                .when(!choice.is_empty() && !busy, |row| {
                    row.child(button("poll-vote", t!("poll.vote"), true).on_click(cx.listener(
                        move |this, _, _, cx| this.vote(channel, message, choice.clone(), cx),
                    )))
                })
            }
        });

        div()
            .max_w(px(520.))
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::BASE_LOW))
            .child(
                div()
                    .text_size(px(17.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_STRONG))
                    .child(poll.question.text.clone().unwrap_or_default()),
            )
            .when(!closed, |card| {
                card.child(div().text_xs().text_color(theme::muted()).child(if poll.allow_multiselect {
                    t!("poll.select_many")
                } else {
                    t!("poll.select_one")
                }))
            })
            .children(answers)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(div().text_size(px(14.)).text_color(theme::muted()).child(footer_text))
                    .children(actions),
            )
            .into_any_element()
    }

    /// Picks or unpicks an answer before voting; one at a time unless the
    /// poll takes several.
    fn pick_answer(&mut self, message: Snowflake, answer: u32, multiple: bool, cx: &mut Context<Self>) {
        let answers = match &mut self.poll_choice {
            Some((current, answers)) if *current == message => answers,
            choice => &mut choice.insert((message, Vec::new())).1,
        };
        if let Some(at) = answers.iter().position(|a| *a == answer) {
            answers.remove(at);
        } else {
            if !multiple {
                answers.clear();
            }
            answers.push(answer);
        }
        cx.notify();
    }

    /// Sends our vote, or takes it back with no answers. The counts change
    /// when the gateway echoes it.
    fn vote(&mut self, channel: Snowflake, message: Snowflake, answers: Vec<u32>, cx: &mut Context<Self>) {
        if self.run(Action::Message, Command::VotePoll { channel, message, answers }, cx) {
            self.poll_choice = None;
            self.poll_results.remove(&message);
        }
    }
}

/// "12 votes", or "1 vote".
fn vote_count(count: u32) -> SharedString {
    if count == 1 { t!("poll.vote_one").into() } else { t!("poll.vote_count", count = count).into() }
}

/// The box an answer shows before voting: round for one answer, square
/// for several, filled once picked.
fn check_box(picked: bool, multiple: bool) -> gpui::Div {
    let side = px(20.);
    div()
        .flex_none()
        .size(side)
        .border_2()
        .border_color(if picked { rgb(theme::BRAND) } else { rgb(theme::INTERACTIVE_MUTED) })
        .when(picked, |b| b.bg(rgb(theme::BRAND)))
        .map(|b| if multiple { b.rounded_sm() } else { b.rounded_full() })
}
