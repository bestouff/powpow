use super::{NavKind, escape_html, page};
use maud::{Markup, PreEscaped, html};

fn simple_md_to_html(md: &str) -> String {
    let mut html = String::new();
    let mut in_list = false;

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            continue;
        }

        // Headings
        if let Some(rest) = trimmed.strip_prefix("## ") {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            html.push_str(&format!(
                "<h2 class=\"title is-5 mt-5\">{}</h2>\n",
                escape_html(rest)
            ));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            html.push_str(&format!(
                "<h1 class=\"title is-4\">{}</h1>\n",
                escape_html(rest)
            ));
        } else if trimmed.starts_with("    ") || trimmed.starts_with("- ") {
            // List items
            let item = trimmed
                .strip_prefix("    ")
                .or_else(|| trimmed.strip_prefix("- "))
                .unwrap_or(trimmed);
            if !in_list {
                html.push_str("<ul class=\"ml-5 mb-3\">\n");
                in_list = true;
            }
            html.push_str(&format!("<li>{}</li>\n", escape_html(item)));
        } else {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            // Inline email links: Text <email> → clickable mailto
            let text = if let (Some(start), Some(end)) = (trimmed.find('<'), trimmed.find('>')) {
                let email_addr = &trimmed[start + 1..end];
                if email_addr.contains('@') {
                    let before = escape_html(&trimmed[..start]);
                    let after = escape_html(&trimmed[end + 1..]);
                    format!(
                        "{}<a href=\"mailto:{}\">{}</a>{}",
                        before,
                        escape_html(email_addr),
                        escape_html(email_addr),
                        after
                    )
                } else {
                    escape_html(trimmed)
                }
            } else {
                escape_html(trimmed)
            };
            html.push_str(&format!("<p class=\"mb-3\">{}</p>\n", text));
        }
    }
    if in_list {
        html.push_str("</ul>\n");
    }
    html
}

pub fn static_page(prefix: &str, title: &str, markdown: &str) -> Markup {
    let p = prefix;
    let body = simple_md_to_html(markdown);

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
