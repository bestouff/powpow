use super::{NavKind, page};
use crate::models::ContentBlock;
use maud::{Markup, PreEscaped, html};

/// Render a CMS-driven legal page (privacy policy or terms of service).
///
/// If the content block is `None` or has an empty body, a placeholder message
/// is shown so the admin knows to fill it in via the CMS.
pub fn legal_page(prefix: &str, title: &str, block: Option<&ContentBlock>) -> Markup {
    let p = prefix;
    let body_html = block
        .filter(|b| !b.body.is_empty())
        .map(ContentBlock::render_body);

    let content = html! {
        section .section {
            div .container.container-narrow {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href={(p) "/"} { "Accueil" } }
                        li .is-active { a href="#" aria-current="page" { (title) } }
                    }
                }
                div .box.content {
                    h1 { (title) }
                    @if let Some(ref html) = body_html {
                        (PreEscaped(html))
                    } @else {
                        p .has-text-grey-light { "[contenu à rédiger]" }
                    }
                }
            }
        }
    };

    page(
        &format!("{title} - PowPow"),
        prefix,
        &NavKind::LoginOnly,
        "",
        html! {},
        content,
        html! {},
    )
}
