use crate::models::ContentBlock;
use maud::{Markup, PreEscaped, html};

/// Render a content block's body (markdown → HTML), optional image, and optional link button.
/// The `prefix` is the URL prefix for content image URLs.
/// The `heading_level` controls the tag used for the title (e.g. "h2", "h3", "h4").
pub fn render_content_block(
    block: &ContentBlock,
    prefix: &str,
    heading_level: &str,
    heading_class: &str,
) -> Markup {
    let body_html = block.render_body();

    html! {
        @if !block.title.is_empty() {
            @match heading_level {
                "h2" => {
                    h2 class=(heading_class) { (block.title) }
                },
                "h3" => {
                    h3 class=(heading_class) { (block.title) }
                },
                "h4" => {
                    h4 class=(heading_class) { (block.title) }
                },
                "h5" => {
                    h5 class=(heading_class) { (block.title) }
                },
                _ => {
                    h3 class=(heading_class) { (block.title) }
                },
            }
        }
        @if let Some(img_id) = block.image_id {
            div .mt-3.mb-3 {
                img src=(format!("{prefix}/content-images/{img_id}"))
                    alt=(block.title)
                    style="max-width:100%;border-radius:6px;";
            }
        }
        @if !body_html.is_empty() {
            div .content { (PreEscaped(body_html)) }
        }
        @if let Some(ref url) = block.link_url {
            @let label = block.link_label.as_deref().unwrap_or(url);
            a .btn-station.btn-station-primary href=(url) target="_blank" {
                (label)
            }
        }
    }
}
