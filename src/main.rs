// Release builds on Windows are GUI apps: no console window behind them.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[macro_use]
mod i18n;

mod discord;
mod paths;
mod settings;
mod ui;
mod webview;

use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds,
    WindowOptions, actions, px, size,
};

actions!(emerald, [Quit, Logout]);

/// Forgets the stored token and restarts this binary, which then finds no
/// token and opens the login webview. On Unix `exec` replaces the process;
/// on Windows the new one is spawned and this one exits. Either way gpui, the
/// network thread and the gateway session all go away with it.
pub(crate) fn sign_out() -> ! {
    if let Some(path) = discord::identity::token_path() {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => eprintln!("warning: could not delete the token: {e}"),
        }
    }
    let error = match std::env::current_exe() {
        // DISCORD_TOKEN would hand the revoked token straight back.
        Ok(exe) => restart(std::process::Command::new(exe).env_remove("DISCORD_TOKEN")),
        Err(e) => e,
    };
    eprintln!("could not restart Emerald: {error}");
    std::process::exit(1);
}

/// Replaces this process with `command`; returns only on failure.
#[cfg(unix)]
fn restart(command: &mut std::process::Command) -> std::io::Error {
    use std::os::unix::process::CommandExt;
    command.exec()
}

#[cfg(windows)]
fn restart(command: &mut std::process::Command) -> std::io::Error {
    match command.spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => e,
    }
}

/// Reads the account token from `DISCORD_TOKEN`, falling back to the stored
/// one (`token` under `paths::config_dir`).
fn load_token() -> Option<String> {
    std::env::var("DISCORD_TOKEN")
        .ok()
        .or_else(|| std::fs::read_to_string(discord::identity::token_path()?).ok())
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// Child mode (see `webview`): runs one webview flow, prints its result and
/// exits before any gpui / NSApplication setup in this process.
fn run_child_mode() {
    let result = match std::env::args().nth(1).as_deref() {
        Some("--login") => webview::acquire_token(),
        Some("--captcha") => {
            let challenge: Option<discord::rest::Challenge> =
                serde_json::from_reader(std::io::stdin()).ok();
            challenge.and_then(|c| webview::solve_captcha(&c.sitekey, c.rqdata.as_deref()))
        }
        _ => return,
    };
    match result {
        Some(result) => {
            print!("{result}");
            std::process::exit(0);
        }
        None => std::process::exit(2),
    }
}

fn main() {
    // Before the child modes too: the login and captcha windows have titles.
    let settings = settings::Settings::load();
    i18n::set(settings.language());
    ui::set_time_format(settings.time_format);
    run_child_mode();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let token = match load_token() {
        Some(token) => token,
        // No stored token: open the official login webview in a child process.
        None => match webview::run_child("--login", b"") {
            Some(token) => {
                if let Err(e) = discord::identity::persist_token(&token) {
                    eprintln!("warning: could not save the token: {e}");
                }
                token
            }
            None => {
                eprintln!("{}", t!("login.cancelled"));
                std::process::exit(1);
            }
        },
    };
    let (events, commands) = discord::spawn(token, settings.status);

    Application::new().with_assets(ui::Icons).run(move |cx: &mut App| {
        ui::text_input::bind_keys(cx);
        ui::bind_keys(cx);
        cx.bind_keys([
            KeyBinding::new("secondary-q", Quit, None),
            KeyBinding::new("secondary-shift-l", Logout, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        let bounds = Bounds::centered(None, size(px(1200.), px(780.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Emerald".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| ui::Emerald::new(commands, settings, window, cx)),
            )
            .expect("open window");

        window
            .update(cx, |view, window, cx| {
                window.focus(&view.input_focus(cx));
                cx.activate(true);
            })
            .ok();

        // Drain network events in batches so a burst triggers a single re-render.
        // A rejected token sends the user back to the login webview.
        cx.spawn(async move |cx| {
            while let Ok(event) = events.recv().await {
                // The window is needed so events can move focus (e.g. into a new DM).
                let alive = window.update(cx, |view, window, cx| {
                    let mut event = Some(event);
                    while let Some(next) = event.take().or_else(|| events.try_recv().ok()) {
                        match &next {
                            discord::NetEvent::AuthExpired => sign_out(),
                            discord::NetEvent::LoggedOut { error } => {
                                if let Some(error) = error {
                                    eprintln!("warning: Discord did not revoke the token: {error}");
                                }
                                sign_out();
                            }
                            _ => {}
                        }
                        view.on_net(next, window, cx);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    });
}
