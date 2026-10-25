//! Pinned messages of the open channel, latest pin first, paged as the
//! web client pages them: by the time of the last pin shown.

use super::*;

use emerald_discord::{Command, PinPage};

impl Emerald {
    pub(in super::super) fn toggle_pins(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        if self.panel_is(PanelKind::Pins) {
            return self.close_panel(cx);
        }
        self.panel = Some(Panel::Pins {
            channel,
            results: Results::loading(),
            last_pinned: None,
        });
        self.net.post(Command::Pins { channel, before: None });
        cx.notify();
    }

    fn more_pins(&mut self, cx: &mut Context<Self>) {
        let Some(Panel::Pins {
            channel,
            results,
            last_pinned,
        }) = &mut self.panel
        else {
            return;
        };
        if results.loading {
            return;
        }
        results.loading = true;
        let (channel, before) = (*channel, last_pinned.clone());
        self.net.post(Command::Pins { channel, before });
        cx.notify();
    }

    pub(in super::super) fn on_pins(&mut self, channel: Snowflake, before: Option<String>, page: Result<PinPage, String>) {
        let Some(Panel::Pins {
            channel: shown,
            results,
            last_pinned,
        }) = &mut self.panel
        else {
            return;
        };
        if *shown != channel || *last_pinned != before {
            return;
        }
        results.loading = false;
        match page {
            Ok(page) => {
                results.more = page.has_more;
                if let Some((_, at)) = page.pins.last() {
                    *last_pinned = Some(at.clone());
                }
                let spaces = &self.spaces;
                results.items.extend(page.pins.into_iter().map(|(message, _)| Found::new(message, spaces)));
            }
            Err(error) => results.error = Some(error.into()),
        }
    }

    pub(super) fn render_pins(&self, results: &Results, cx: &mut Context<Self>) -> AnyElement {
        self.render_results("pins", results, t!("pins.empty"), false, Self::more_pins, cx)
    }
}
