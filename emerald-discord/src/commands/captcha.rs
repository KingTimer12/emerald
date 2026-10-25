//! Writes Discord may answer with a captcha: the user solves it in a
//! webview child process, and the write goes out once more.

use crate::rest::{CaptchaRequired, Solution};

/// Evaluates `$call` with `$captcha = None`; if Discord answers with a
/// captcha, shows it to the user and evaluates `$call` once more with their
/// solution. Closing the captcha window keeps the original error. A macro
/// rather than an async closure so the spawned future stays `Send`.
macro_rules! with_captcha {
    (|$captcha:ident| $call:expr) => {{
        let $captcha: Option<&$crate::rest::Solution> = None;
        match $call {
            Err(error) => match $crate::commands::captcha::solve(&error).await {
                Some(solution) => {
                    let $captcha = Some(&solution);
                    $call
                }
                None => Err(error),
            },
            ok => ok,
        }
    }};
}
pub(super) use with_captcha;

/// When `error` is a captcha challenge, lets the user solve it and returns
/// the solution.
pub async fn solve(error: &anyhow::Error) -> Option<Solution> {
    let CaptchaRequired(challenge) = error.downcast_ref::<CaptchaRequired>()?;
    let challenge = challenge.clone();
    let input = serde_json::to_vec(&challenge).ok()?;
    // The child blocks until its window closes; keep it off the runtime thread.
    let key = tokio::task::spawn_blocking(move || emerald_core::process::run_child("--captcha", &input))
        .await
        .ok()
        .flatten()?;
    Some(Solution::new(key, &challenge))
}
