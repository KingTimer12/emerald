//! `X-Context-Properties`: which part of the web client a request came
//! from, sent base64-encoded on relationship, invite and some message calls.

use serde_json::{Value, json};

/// The surface the request came from, by the web client's name for it.
pub fn location(location: &str) -> Value {
    json!({ "location": location })
}

/// The header with nothing in it, as the web client sends on opening a DM.
pub fn empty() -> Value {
    json!({})
}
