//! GIFs from the picker.

use serde::Deserialize;

/// A GIF from Discord's Tenor proxy, as the GIF picker lists them.
#[derive(Clone, Debug, Deserialize)]
pub struct Gif {
    /// Tenor's id, a string of digits.
    #[serde(deserialize_with = "text_or_number")]
    pub id: String,
    /// The Tenor page; sending it is how a GIF is posted.
    pub url: String,
    /// A still of the GIF; not every answer has one.
    #[serde(default)]
    pub preview: Option<String>,
    /// The GIF itself, in GIF format.
    #[serde(default)]
    pub gif_src: Option<String>,
    /// The GIF in the asked `media_format`, usually a video.
    #[serde(default)]
    pub src: Option<String>,
}

impl Gif {
    /// The first of its URLs that is an image this client can draw, as
    /// the web client falls back from the video `src` to `gif_src`.
    pub fn image_url(&self) -> Option<&str> {
        [&self.preview, &self.gif_src, &self.src]
            .into_iter()
            .filter_map(|url| url.as_deref())
            .find(|url| is_image_url(url))
    }
}

/// Whether `url`'s path ends like a PNG, JPEG, GIF or WebP file.
fn is_image_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or_default().to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp"].iter().any(|ext| path.ends_with(ext))
}

fn text_or_number<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(text) => Ok(text),
        serde_json::Value::Number(number) => Ok(number.to_string()),
        other => Err(serde::de::Error::custom(format!("unexpected GIF id {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gifs_parse_and_pick_an_image() {
        let gif: Gif = serde_json::from_str(
            r#"{"id":"123","title":"","url":"https://tenor.com/view/x-123","src":"https://media.tenor.com/a.webm",
                "gif_src":"https://media.tenor.com/a.gif","width":498,"height":280,"preview":"https://media.tenor.com/a.png"}"#,
        )
        .unwrap();
        assert_eq!((gif.id.as_str(), gif.preview.as_deref()), ("123", Some("https://media.tenor.com/a.png")));
        assert_eq!(gif.image_url(), Some("https://media.tenor.com/a.png"));
        let gif: Gif = serde_json::from_str(r#"{"id":7,"url":"u"}"#).unwrap();
        assert_eq!((gif.id.as_str(), gif.image_url()), ("7", None));
        // A video preview is passed over for the GIF.
        let gif: Gif = serde_json::from_str(
            r#"{"id":"1","url":"u","src":"https://m/a.webm","preview":"https://m/a.mp4","gif_src":"https://m/a.GIF?x=1"}"#,
        )
        .unwrap();
        assert_eq!(gif.image_url(), Some("https://m/a.GIF?x=1"));
    }
}
