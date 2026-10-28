//! The voice gateway's websocket: opened with the web client's headers,
//! frames out as JSON or binary, frames in as one of the two.

use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::identity;
use crate::packets::voice::VERSION;
use crate::urls;

/// DAVE frames are the largest; nothing honest comes near this.
const MAX_FRAME: usize = 1 << 20;

pub struct Socket(WebSocketStream<MaybeTlsStream<TcpStream>>);

/// One frame from the voice server.
pub enum Frame {
    Json(Value),
    Binary(Vec<u8>),
    /// The server closed the connection, with this code if it gave one.
    Closed(Option<u16>),
}

impl Socket {
    /// Opens `wss://{endpoint}/?v=9`. The endpoint must be one of Discord's
    /// media hosts.
    pub async fn open(endpoint: &str) -> Result<Self> {
        let host = endpoint.split(':').next().unwrap_or_default();
        if !host.ends_with(".discord.media") || endpoint.contains(['/', '?', '#', '@']) {
            return Err(anyhow!("voice endpoint is not a Discord media host"));
        }
        let mut request = format!("wss://{endpoint}/?v={VERSION}").into_client_request()?;
        let headers = request.headers_mut();
        headers.insert("user-agent", HeaderValue::from_static(identity::USER_AGENT));
        headers.insert("origin", HeaderValue::from_static(urls::WEB.as_str()));
        headers.insert("cache-control", HeaderValue::from_static("no-cache"));
        headers.insert("pragma", HeaderValue::from_static("no-cache"));
        let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(MAX_FRAME))
            .max_frame_size(Some(MAX_FRAME));
        let (socket, _) =
            timeout(Duration::from_secs(15), tokio_tungstenite::connect_async_with_config(request, Some(config), false))
                .await
                .context("voice gateway timeout")??;
        Ok(Self(socket))
    }

    pub async fn send(&mut self, frame: Value) -> Result<()> {
        self.write(Message::Text(frame.to_string().into())).await
    }

    pub async fn send_binary(&mut self, frame: Vec<u8>) -> Result<()> {
        self.write(Message::Binary(frame.into())).await
    }

    async fn write(&mut self, message: Message) -> Result<()> {
        timeout(Duration::from_secs(5), self.0.send(message)).await.context("voice gateway write timeout")??;
        Ok(())
    }

    /// The next frame; pings are answered on the way.
    pub async fn next(&mut self) -> Result<Frame> {
        loop {
            let Some(message) = self.0.next().await else { return Ok(Frame::Closed(None)) };
            match message? {
                Message::Text(text) => return Ok(Frame::Json(serde_json::from_str(&text)?)),
                Message::Binary(bytes) => return Ok(Frame::Binary(bytes.into())),
                Message::Close(frame) => return Ok(Frame::Closed(frame.map(|f| u16::from(f.code)))),
                Message::Ping(data) => self.write(Message::Pong(data)).await?,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn only_discord_media_hosts() {
        for endpoint in ["example.com", "evil.com/x.discord.media", "a@b.discord.media"] {
            assert!(Socket::open(endpoint).await.is_err(), "{endpoint}");
        }
    }
}
