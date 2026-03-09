use super::{NavKind, page};
use maud::{Markup, PreEscaped, html};

/// Convert Markdown to sanitised HTML using `pulldown-cmark` + `ammonia`.
fn md_to_html(md: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let parser = Parser::new_ext(md, Options::all());
    let mut raw_html = String::new();
    html::push_html(&mut raw_html, parser);
    ammonia::clean(&raw_html)
}

pub fn static_page(prefix: &str, title: &str, markdown: &str) -> Markup {
    let p = prefix;
    let body = md_to_html(markdown);

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
                    (PreEscaped(body))
                }
            }
        }
    };

    page(
        &format!("{title} - AGHIL"),
        prefix,
        &NavKind::LoginOnly,
        "",
        html! {},
        content,
        html! {},
    )
}
