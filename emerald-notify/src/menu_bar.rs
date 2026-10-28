//! The app's item in the macOS menu bar, through AppKit: an icon with the
//! unread count beside it, and a menu to show the window or quit.

use std::sync::OnceLock;

use async_channel::Sender;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{AnyThread, MainThreadMarker, define_class, msg_send, sel};
use objc2_app_kit::{NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};

use super::{TrayAction, badge_text};

/// Where the menu's clicks go; set once, by the first item made.
static SENDER: OnceLock<Sender<TrayAction>> = OnceLock::new();

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the class does
    // not implement `Drop`.
    #[unsafe(super(NSObject))]
    #[name = "EmeraldMenuBarTarget"]
    struct Target;

    impl Target {
        #[unsafe(method(showWindow:))]
        fn show_window(&self, _sender: Option<&AnyObject>) {
            send(TrayAction::Show);
        }

        #[unsafe(method(quitApp:))]
        fn quit_app(&self, _sender: Option<&AnyObject>) {
            send(TrayAction::Quit);
        }
    }

    unsafe impl NSObjectProtocol for Target {}
);

impl Target {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

/// Hands a click to the app. The menu runs on the main thread and the
/// channel is unbounded, so this never blocks.
fn send(action: TrayAction) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.try_send(action);
    }
}

/// The status item, kept alive with its menu and the menu's target.
pub struct Item {
    status: Retained<NSStatusItem>,
    _menu: Retained<NSMenu>,
    _target: Retained<Target>,
}

impl Item {
    /// Puts the item in the menu bar with `show` and `quit` as the menu's
    /// titles. `None` off the main thread.
    pub fn new(sender: Sender<TrayAction>, show: &str, quit: &str) -> Option<Self> {
        let Some(mtm) = MainThreadMarker::new() else {
            tracing::warn!("menu bar item: not on the main thread");
            return None;
        };
        let _ = SENDER.set(sender);
        let target = Target::new();
        let target_object: &AnyObject = &target;

        let menu = NSMenu::new(mtm);
        for (title, action) in [(show, sel!(showWindow:)), (quit, sel!(quitApp:))] {
            let item = menu_item(mtm, title, action);
            unsafe { item.setTarget(Some(target_object)) };
            menu.addItem(&item);
        }

        let status = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = status.button(mtm) {
            // The symbol is the icon; its description is what VoiceOver reads.
            let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &NSString::from_str("bubble.left.fill"),
                Some(&NSString::from_str(show)),
            );
            button.setImage(image.as_deref());
        }
        status.setMenu(Some(&menu));
        Some(Self {
            status,
            _menu: menu,
            _target: target,
        })
    }

    /// The count beside the icon; empty when there is nothing unread.
    pub fn set_count(&self, count: u32) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if let Some(button) = self.status.button(mtm) {
            let title = badge_text(count).unwrap_or_default();
            button.setTitle(&NSString::from_str(&title));
        }
    }
}

fn menu_item(mtm: MainThreadMarker, title: &str, action: Sel) -> Retained<NSMenuItem> {
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            Some(action),
            &NSString::from_str(""),
        )
    }
}
