// Release builds on Windows are GUI apps: no console window behind them.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[macro_use]
mod i18n;

mod discord;
mod logging;
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
    discord::token::forget();
    // Kept API answers belong to the account that is leaving.
    if let Some(cache) = discord::cache::Cache::open_default() {
        cache.clear_responses();
    }
    let error = match std::env::current_exe() {
        // `token::take_env` already took DISCORD_TOKEN out of the environment.
        Ok(exe) => restart(&mut std::process::Command::new(exe)),
        Err(e) => e,
    };
    tracing::error!("could not restart Emerald: {error}");
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
    // First, while the process is still single-threaded, as taking the
    // token out of the environment requires.
    let env_token = discord::token::take_env();
    // Before the child modes too: the login and captcha windows have titles.
    let settings = settings::Settings::load();
    i18n::set(settings.language());
    ui::set_time_format(settings.time_format);
    if std::env::args().nth(1).is_some_and(|arg| arg == "--login" || arg == "--captcha") {
        logging::init_child();
    }
    run_child_mode();

    let logging = logging::init();
    log_startup(&logging, &settings);

    let token = match env_token.inspect(|_| tracing::info!("token taken from DISCORD_TOKEN")) {
        Some(token) => token,
        None => match discord::token::load() {
            Some(token) => token,
            // No stored token: open the official login webview in a child process.
            None => {
                tracing::info!("opening the login window");
                match webview::run_child("--login", b"").and_then(discord::token::Token::new) {
                    Some(token) => {
                        tracing::info!("signed in");
                        if let Err(e) = discord::token::save(&token) {
                            tracing::warn!("could not save the token: {e}");
                        }
                        token
                    }
                    None => {
                        tracing::info!("login cancelled");
                        std::process::exit(1);
                    }
                }
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
        cx.on_action(|_: &Quit, cx| {
            tracing::info!("quitting");
            cx.quit()
        });

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
                                    tracing::warn!("Discord did not revoke the token: {error}");
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

/// What a bug report needs to know about this run, without anything
/// personal: version, platform, where files go, and the chosen language.
fn log_startup(logging: &logging::Logging, settings: &settings::Settings) {
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        build = if cfg!(debug_assertions) { "debug" } else { "release" },
        pid = std::process::id(),
        "Emerald starting"
    );
    let shown = |dir: Option<std::path::PathBuf>| dir.map_or_else(|| "unavailable".to_owned(), |d| d.display().to_string());
    tracing::info!(
        logs = %shown(logging.dir.clone()),
        config = %shown(paths::config_dir()),
        cache = %shown(paths::cache_dir()),
        "directories"
    );
    tracing::info!(language = ?settings.language(), "settings loaded");
}
