//! Sync an RSS news feed into the `news` database table.
//!
//! A background loop fetches the feed immediately at startup and then
//! every 15 minutes.  Each item is upserted by its RSS `<guid>`.
//! Images are downloaded and stored as BYTEA alongside the text.

use sqlx::PgPool;
use tracing::{info, warn};

use crate::database;

/// Maximum number of items to keep in the database.
const KEEP_ITEMS: i64 = 20;

/// Maximum image download size (2 MB).
const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;

/// Image download / RSS fetch timeout.
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// ── Public API ──────────────────────────────────────────────────────

/// Fetch the RSS feed, upsert every item into the database, and prune old rows.
pub async fn sync_news(pool: &PgPool, feed_url: &str) {
    let items = match fetch_and_parse(feed_url).await {
        Ok(v) => v,
        Err(e) => {
            warn!("news: RSS fetch failed: {e}");
            return;
        }
    };

    // Find out which guids already have an image stored so we can skip
    // re-downloading them every cycle.
    let have_images = database::news_guids_with_images(pool)
        .await
        .unwrap_or_default();

    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .build()
        .unwrap_or_default();

    for item in &items {
        let image = if have_images.contains(&item.guid) {
            // Already stored — no need to download again.
            None
        } else {
            match item.image_url {
                Some(ref url) => download_image(&client, url).await,
                None => None,
            }
        };

        if let Err(e) = database::upsert_news_item(
            pool,
            &item.guid,
            &item.text,
            &item.link,
            item.pub_date,
            image.as_ref().map(|(data, _)| data.as_slice()),
            image.as_ref().map(|(_, mime)| mime.as_str()),
        )
        .await
        {
            warn!("news: failed to upsert guid={}: {e}", item.guid);
        }
    }

    match database::prune_old_news(pool, KEEP_ITEMS).await {
        Ok(0) => {}
        Ok(n) => info!("news: pruned {n} old items"),
        Err(e) => warn!("news: prune failed: {e}"),
    }

    info!("news: synced {} items from RSS", items.len());
}

// ── Internal types ──────────────────────────────────────────────────

/// Parsed item ready for upsert (not stored permanently).
struct ParsedItem {
    guid: String,
    text: String,
    link: String,
    pub_date: Option<chrono::DateTime<chrono::Utc>>,
    image_url: Option<String>,
}

// ── RSS fetch + parse ───────────────────────────────────────────────

async fn fetch_and_parse(url: &str) -> anyhow::Result<Vec<ParsedItem>> {
    let body = reqwest::Client::new()
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await?
        .bytes()
        .await?;

    let channel = rss::Channel::read_from(&body[..])?;

    let mut items: Vec<ParsedItem> = channel
        .items()
        .iter()
        .filter_map(|item| {
            let guid = item.guid().map(|g| g.value().to_string())?;
            let link = item.link().unwrap_or_default().to_string();

            // Text: keep the longest of title vs stripped description
            let title = item.title().unwrap_or_default().trim().to_string();
            let description_text = item.description().map(strip_html_tags).unwrap_or_default();
            let text = if description_text.len() >= title.len() {
                description_text
            } else {
                title
            };
            if text.is_empty() {
                return None;
            }

            let pub_date = item
                .pub_date()
                .and_then(|s| chrono::DateTime::parse_from_rfc2822(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            let image_url = extract_image_url(item);

            Some(ParsedItem {
                guid,
                text,
                link,
                pub_date,
                image_url,
            })
        })
        .collect();

    // Push a "fake news" item which is just the title+link+favicon of the feed
    items.push(ParsedItem {
        guid: String::new(),
        text: channel.title,
        link: channel.link,
        pub_date: None,
        image_url: channel.image.map(|image| image.url),
    });

    Ok(items)
}

// ── Image extraction ────────────────────────────────────────────────

/// Extract the best image URL from an RSS item.
///
/// 1. Try `<media:content medium="image" url="..."/>` extension.
/// 2. Fall back to the first `<img src="...">` in the HTML description.
fn extract_image_url(item: &rss::Item) -> Option<String> {
    // 1. media:content extension
    let media_ns = "http://search.yahoo.com/mrss/";
    if let Some(ns_map) = item.extensions().get(media_ns)
        && let Some(contents) = ns_map.get("content")
    {
        for ext in contents {
            if let Some(url) = ext.attrs.get("url")
                && !url.is_empty()
            {
                return Some(decode_xml_entities(url));
            }
        }
    }

    // 2. <img src="..."> in description HTML
    if let Some(desc) = item.description() {
        return extract_img_src(desc).map(|u| decode_xml_entities(&u));
    }

    None
}

/// Pull the first `src` attribute out of an `<img ...>` tag.
fn extract_img_src(html: &str) -> Option<String> {
    // Find <img ... src="..." ...> or <img ... src='...' ...>
    let img_start = html.find("<img ")?;
    let after_img = &html[img_start..];
    let src_pos = after_img.find("src=")?;
    let after_src = &after_img[src_pos + 4..];
    let quote = after_src.as_bytes().first()?;
    if *quote != b'"' && *quote != b'\'' {
        return None;
    }
    let inner = &after_src[1..];
    let end = inner.find(*quote as char)?;
    let url = &inner[..end];
    if url.is_empty() {
        return None;
    }
    Some(url.to_string())
}

/// Decode the five standard XML entities in a string.
///
/// RSS extension attributes may contain `&amp;` etc. which must be
/// unescaped before using the value as an HTTP URL.
fn decode_xml_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

// ── Image download ──────────────────────────────────────────────────

/// Download an image, returning `(bytes, mime_type)`.
///
/// Returns `None` if the download fails, exceeds 2 MB, or the body is not
/// a recognised image format (validated by magic bytes).
async fn download_image(client: &reqwest::Client, url: &str) -> Option<(Vec<u8>, String)> {
    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            warn!("news: image download failed for {url}: {e}");
            return None;
        }
    };

    // Check Content-Length hint (if present) before reading the body
    if let Some(cl) = resp.content_length()
        && cl as usize > MAX_IMAGE_BYTES
    {
        warn!("news: image too large ({cl} bytes), skipping {url}");
        return None;
    }

    let data = match resp.bytes().await {
        Ok(b) => b.to_vec(),
        Err(e) => {
            warn!("news: failed to read image body from {url}: {e}");
            return None;
        }
    };

    if data.len() > MAX_IMAGE_BYTES {
        warn!(
            "news: image body too large ({} bytes), skipping {url}",
            data.len()
        );
        return None;
    }

    // Validate the body is actually an image by checking magic bytes.
    // This protects against CDN error pages (e.g. "URL signature expired")
    // that may be served with a misleading Content-Type or URL extension.
    let Some(mime) = mime_from_magic(&data) else {
        warn!("news: body is not a recognised image format, skipping {url}");
        return None;
    };

    Some((data, mime.to_string()))
}

/// Detect image MIME type from the first bytes of the body (magic bytes).
///
/// Returns `None` if the body doesn't start with a recognised image signature.
fn mime_from_magic(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else if data.len() >= 12 && &data[4..12] == b"ftypavif" {
        Some("image/avif")
    } else {
        None
    }
}

// ── HTML strip helper ───────────────────────────────────────────────

/// Strip HTML tags from a string and return clean plain text.
///
/// Uses ammonia (already a dependency) to strip everything, then
/// collapses whitespace.
fn strip_html_tags(raw: &str) -> String {
    // Replace <br> variants with a space so line breaks become word separators
    let html = raw
        .replace("<br>", " ")
        .replace("<br/>", " ")
        .replace("<br />", " ");
    // Build a sanitiser that strips *all* tags (empty allow-list)
    let mut builder = ammonia::Builder::new();
    builder.tags(std::collections::HashSet::new());
    let text = builder.clean(&html).to_string();
    // Collapse runs of whitespace and trim
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_basic() {
        let html = "<div><img src=\"x\" /><div>Hello <strong>world</strong></div></div>";
        assert_eq!(strip_html_tags(html), "Hello world");
    }

    #[test]
    fn test_strip_html_br() {
        let html = "Line one<br>Line two<br />Line three";
        assert_eq!(strip_html_tags(html), "Line one Line two Line three");
    }

    #[test]
    fn test_strip_html_empty() {
        assert_eq!(strip_html_tags(""), "");
    }

    #[test]
    fn test_strip_html_plain_text() {
        assert_eq!(strip_html_tags("Just text"), "Just text");
    }

    #[test]
    fn test_extract_img_src_basic() {
        let html =
            r#"<div><img src="https://example.com/photo.jpg" alt="test"><div>text</div></div>"#;
        assert_eq!(
            extract_img_src(html),
            Some("https://example.com/photo.jpg".to_string())
        );
    }

    #[test]
    fn test_extract_img_src_single_quotes() {
        let html = "<img src='https://example.com/img.png' />";
        assert_eq!(
            extract_img_src(html),
            Some("https://example.com/img.png".to_string())
        );
    }

    #[test]
    fn test_extract_img_src_none() {
        assert_eq!(extract_img_src("no image here"), None);
    }

    #[test]
    fn test_text_keeps_longest() {
        // Simulates the case where title is a truncated prefix of description
        let title = "Breaking news from the mountain...";
        let description = "Breaking news from the mountain resort: the new lift is now operational and open to the public.";
        let desc_stripped = strip_html_tags(description);
        let text = if desc_stripped.len() >= title.len() {
            desc_stripped
        } else {
            title.to_string()
        };
        assert_eq!(text, description);
    }

    #[test]
    fn test_decode_xml_entities() {
        assert_eq!(
            decode_xml_entities("https://example.com/img.jpg?a=1&amp;b=2&amp;c=3"),
            "https://example.com/img.jpg?a=1&b=2&c=3"
        );
    }

    #[test]
    fn test_decode_xml_entities_no_entities() {
        assert_eq!(
            decode_xml_entities("https://example.com/img.jpg"),
            "https://example.com/img.jpg"
        );
    }

    #[test]
    fn test_magic_png() {
        let data = b"\x89PNG\r\n\x1a\nrest of file";
        assert_eq!(mime_from_magic(data), Some("image/png"));
    }

    #[test]
    fn test_magic_jpeg() {
        let data = b"\xff\xd8\xff\xe0rest of file";
        assert_eq!(mime_from_magic(data), Some("image/jpeg"));
    }

    #[test]
    fn test_magic_gif89a() {
        let data = b"GIF89a...";
        assert_eq!(mime_from_magic(data), Some("image/gif"));
    }

    #[test]
    fn test_magic_webp() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&[0, 0, 0, 0]); // file size placeholder
        data.extend_from_slice(b"WEBP");
        assert_eq!(mime_from_magic(&data), Some("image/webp"));
    }

    #[test]
    fn test_magic_text_rejected() {
        // "URL signature expired" — should NOT be detected as an image
        assert_eq!(mime_from_magic(b"URL signature expired"), None);
    }

    #[test]
    fn test_magic_empty() {
        assert_eq!(mime_from_magic(b""), None);
    }
}
