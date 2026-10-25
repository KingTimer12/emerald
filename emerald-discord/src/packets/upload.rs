//! Attachment uploads: asking a channel for storage URLs, and what it
//! answers.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::protocol::Upload;

/// A file already in Discord's storage, ready to name in a message.
pub struct Uploaded {
    pub filename: String,
    pub uploaded_filename: String,
    pub description: String,
    pub spoiler: bool,
}

/// `POST /channels/{id}/attachments` for one file.
pub fn request(file: &Upload) -> Value {
    let mut entry = json!({
        "filename": file.name,
        "file_size": file.bytes.len(),
        "id": upload_id(),
        "is_clip": false,
    });
    if let Some(mime) = mime_type(&file.name) {
        entry["original_content_type"] = mime.into();
    }
    json!({ "files": [entry] })
}

#[derive(Deserialize)]
pub struct UploadUrls {
    pub attachments: Vec<UploadUrl>,
}

#[derive(Deserialize)]
pub struct UploadUrl {
    pub upload_url: String,
    pub upload_filename: String,
}

/// The web client numbers upload requests with lodash's `uniqueId`: a
/// counter shared with the rest of the page, sent as a string.
fn upload_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed).to_string()
}

/// The type a browser reports for a file, from its extension; unknown ones
/// send none.
fn mime_type(name: &str) -> Option<&'static str> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "txt" => "text/plain",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "json" => "application/json",
        _ => return None,
    })
}
