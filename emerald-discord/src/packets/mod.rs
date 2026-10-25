//! What goes on the wire, built the way the web client builds it: gateway
//! frames ([`gateway`]) and the JSON bodies of REST calls, with the answer
//! shapes that only one endpoint uses. Field order matters to Discord's
//! fingerprinting as much as the fields, so each builder keeps the web
//! client's order (`serde_json` preserves insertion order here).

pub mod context;
pub mod gateway;
pub mod guild;
pub mod interaction;
pub mod message;
pub mod thread;
pub mod upload;
pub mod user;
