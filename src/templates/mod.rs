mod admin;
mod auth;
mod calendar;
mod cash;
mod content;
mod content_admin;
mod home;
mod legal;
mod mailchimp;
mod manual_import;
mod membership;
mod photos;
mod settings;
mod staff;

pub use admin::{
    admin_page, audit_page, qualifications_page, restore_page, restore_result, validation_page,
};
pub use auth::login_page;
pub use calendar::{calendar, calendar_editor, render_upcoming_week_email};
pub use cash::{cash_form, cash_list};
pub use content::render_content_block;
pub use content_admin::{content_edit_page, content_list_page};
pub use home::index;
pub use legal::legal_page;
pub use mailchimp::{mailchimp_page, mailchimp_sync_result};
pub use manual_import::manual_import_page;
pub use membership::{import_result, membership_list_with_filters, user_detail};
pub use photos::photo_page;
pub use settings::settings_page;
pub use staff::{person_detail, staff_list};

use crate::models::{ContentBlock, ContentMap};
use maud::{DOCTYPE, Markup, html};

pub fn membership_notice(association_slug: &str, is_admin: bool) -> Markup {
    let mut url = reqwest::Url::parse("https://www.helloasso.com/associations")
        .expect("HelloAsso base URL is valid");
    url.path_segments_mut()
        .expect("HelloAsso URL supports path segments")
        .push(association_slug);
    html! {
        div .notification.is-warning role="alert" {
            p { "Votre adhésion n'est pas à jour pour la saison actuelle. Veuillez régler votre adhésion sur HelloAsso pour accéder à votre espace bénévole." }
            @if is_admin {
                p .mt-2 { "En attendant, votre accès administrateur est limité à la gestion des adhésions." }
            }
            a .button.is-link.mt-3 href=(url.as_str()) target="_blank" rel="noopener noreferrer" { "Payer mon adhésion sur HelloAsso" }
        }
    }
}

/// Cached navbar content block, shared across all pages.
static NAVBAR_BLOCK: std::sync::RwLock<Option<ContentBlock>> = std::sync::RwLock::new(None);

/// Cached favicon content block, shared across all pages.
static FAVICON_BLOCK: std::sync::RwLock<Option<ContentBlock>> = std::sync::RwLock::new(None);

/// Cached footer content blocks, shared across all pages.
/// Stores blocks for slugs: `footer-contact`, `footer-calendar`, `footer-summer`.
static FOOTER_BLOCKS: std::sync::RwLock<Option<ContentMap>> = std::sync::RwLock::new(None);

/// Footer content block slugs that trigger a cache refresh when saved.
const FOOTER_SLUGS: &[&str] = &["footer-contact", "footer-calendar", "footer-summer"];

/// Update the cached navbar content block (call at startup and when CMS content is saved).
pub fn set_navbar_block(block: Option<ContentBlock>) {
    if let Ok(mut guard) = NAVBAR_BLOCK.write() {
        *guard = block;
    }
}

/// Read the cached navbar content block.
fn get_navbar_block() -> Option<ContentBlock> {
    NAVBAR_BLOCK.read().ok().and_then(|g| g.clone())
}

/// Update the cached favicon content block (call at startup and when CMS content is saved).
pub fn set_favicon_block(block: Option<ContentBlock>) {
    if let Ok(mut guard) = FAVICON_BLOCK.write() {
        *guard = block;
    }
}

/// Read the cached favicon content block.
fn get_favicon_block() -> Option<ContentBlock> {
    FAVICON_BLOCK.read().ok().and_then(|g| g.clone())
}

/// Update the cached footer content blocks (call at startup and when a footer slug is saved).
pub fn set_footer_blocks(blocks: ContentMap) {
    if let Ok(mut guard) = FOOTER_BLOCKS.write() {
        *guard = Some(blocks);
    }
}

/// Read the cached footer content blocks.
fn get_footer_blocks() -> Option<ContentMap> {
    FOOTER_BLOCKS.read().ok().and_then(|g| g.clone())
}

/// Check whether a given slug is a footer-related slug that should trigger a cache refresh.
pub fn is_footer_slug(slug: &str) -> bool {
    FOOTER_SLUGS.contains(&slug)
}

/// Cached entity name, used in the page footer and email signatures.
static ENTITY_NAME: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

/// Set the entity name (call once at startup from config).
pub fn set_entity_name(name: String) {
    if let Ok(mut guard) = ENTITY_NAME.write() {
        *guard = name;
    }
}

/// Read the entity name, falling back to `"AG'HIL"` if unset.
fn get_entity_name() -> String {
    ENTITY_NAME
        .read()
        .ok()
        .filter(|g| !g.is_empty())
        .map_or_else(|| "AG'HIL".into(), |g| g.clone())
}

/// Short hex hash of embedded CSS+JS for cache-busting query params.
/// Computed once at startup; changes whenever the file content changes.
fn static_version() -> &'static str {
    use std::hash::{DefaultHasher, Hash, Hasher};
    use std::sync::OnceLock;

    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        let mut hasher = DefaultHasher::new();
        crate::POWPOW_CSS.hash(&mut hasher);
        crate::POWPOW_JS.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    })
}
use rlibphonenumber::{PHONE_NUMBER_UTIL, PhoneNumberFormat, Region};

/// Simple HTML escaping for minimal security (kept for email template which returns String)
pub fn escape_html_public(s: &str) -> String {
    escape_html(s)
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub struct TodoItem {
    pub icon: &'static str,
    pub color: &'static str,
    pub html: String,
}

/// Format a phone number to international format
/// Assumes French numbers if no country code is present
pub fn format_phone_international(phone: &str) -> String {
    if phone.is_empty() {
        return String::new();
    }

    // Try to parse with France as default country
    match PHONE_NUMBER_UTIL.parse(phone, Some(Region::FR)) {
        Ok(number) => PHONE_NUMBER_UTIL
            .format(&number, PhoneNumberFormat::International)
            .into_owned(),
        Err(_) => phone.to_string(), // Return original if parsing fails
    }
}

/// Capitalize each word in a string (first letter uppercase, rest lowercase)
/// Handles both spaces and hyphens as word separators
fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            // Handle hyphenated words like "Jean-Pierre"
            word.split('-')
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            first.to_uppercase().collect::<String>()
                                + &chars.as_str().to_lowercase()
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join("-")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

enum NavKind {
    Full,      // Administration, Login
    Standard,  // Administration, Login
    LoginOnly, // Only Login button
    StaffOnly, // Only Staff-related items
}

fn navbar(prefix: &str, kind: &NavKind, active: &str, block: Option<&ContentBlock>) -> Markup {
    let admin_hide = matches!(kind, NavKind::LoginOnly);
    let p = prefix;

    html! {
        nav .navbar.navbar-station role="navigation" aria-label="main navigation" {
            div .container.is-fluid {
                div .navbar-brand {
                    a .navbar-item href={(p) "/"} {
                        @if let Some(b) = block {
                            @if let Some(img_id) = b.image_id {
                                img src=(format!("{p}/content-images/{img_id}"))
                                    alt=(b.title.clone())
                                    style="max-height: 2.5rem;";
                            }
                            @if !b.title.is_empty() {
                                span .navbar-logo-title { (b.title) }
                            }
                        }
                    }
                    a .navbar-burger role="button" aria-label="menu" aria-expanded="false"
                      data-target="main-navbar" {
                        span aria-hidden="true" {}
                        span aria-hidden="true" {}
                        span aria-hidden="true" {}
                    }
                }
                div #main-navbar .navbar-menu {
                    div .navbar-end {
                        a .navbar-item .is-active[active == "calendar"]
                          href={(p) "/calendar"}
                          style=[admin_hide.then_some("display:none")] {
                            span .icon.mr-1 { i .fa-solid.fa-calendar-days {} }
                            "Planning"
                        }
                        a .navbar-item.navbar-admin .is-active[active == "admin"]
                          href={(p) "/admin"}
                          style=[admin_hide.then_some("display:none")] {
                            span .icon.mr-1 { i .fa-solid.fa-screwdriver-wrench {} }
                            "Administration"
                            span .nav-badge.d-none data-badge="admin" {}
                        }
                        a .navbar-item #login-btn href={(p) "/login"} {
                            i .fa-solid.fa-right-to-bracket {}
                            "\u{00a0}Se connecter"
                        }
                    }
                }
            }
        }

    }
}

/// 7-arg convenience wrapper: renders the page with the default hardcoded footer.
fn page(
    title: &str,
    prefix: &str,
    nav_kind: &NavKind,
    active: &str,
    extra_head: Markup,
    content: Markup,
    extra_scripts: Markup,
) -> Markup {
    page_with_footer(
        Some(title),
        prefix,
        nav_kind,
        active,
        extra_head,
        content,
        extra_scripts,
        None,
    )
}

/// Full page renderer with optional CMS footer content blocks.
///
/// When `footer_contents` is `Some(...)` (e.g. the home page), those blocks are
/// used directly. Otherwise the globally cached footer blocks are used so that
/// every page displays the footer content.
#[allow(clippy::too_many_arguments)]
fn page_with_footer(
    custom_title: Option<&str>,
    prefix: &str,
    nav_kind: &NavKind,
    active: &str,
    extra_head: Markup,
    content: Markup,
    extra_scripts: Markup,
    footer_contents: Option<&ContentMap>,
) -> Markup {
    let p = prefix;
    // Use navbar block from footer_contents if available, otherwise from the global cache
    let cached_block = get_navbar_block();
    let navbar_block = footer_contents
        .map(|c| c.get("navbar"))
        .or(cached_block.as_ref());
    let nav = navbar(prefix, nav_kind, active, navbar_block);
    let v = static_version();

    // Resolve favicon: use explicit parameter or fall back to global cache
    let cached_favicon = get_favicon_block();
    let favicon_block = footer_contents
        .and_then(|c| {
            let b = c.get("favicon");
            b.image_id.map(|_| b)
        })
        .or(cached_favicon.as_ref().filter(|b| b.image_id.is_some()));

    // Resolve footer content: use explicit parameter or fall back to global cache
    let cached_footer = get_footer_blocks();
    let effective_footer: Option<&ContentMap> = footer_contents.or(cached_footer.as_ref());

    let title: &str = custom_title
        .or(favicon_block.map(|fav| fav.title.as_str()))
        .unwrap_or("[missing title in favicon section]");

    html! {
        (DOCTYPE)
        html lang="fr" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                meta name="google-site-verification" content="S04nKUrv5gsWl0VqBBdd9Q6zS7rxLWHJLc2aFftaD4E";
                title { (title) }
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bulma@1.0.4/css/bulma.min.css" integrity="sha384-DCY3M8xLkMu6c9IKcKbe+jHKMjelnwC0p+SBaxfHxoBYZWdJF2X400UdBCgATtAB" crossorigin="anonymous";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@fortawesome/fontawesome-free@7.2.0/css/fontawesome.min.css" integrity="sha384-mj4mLShEAyWi4Bui9LmFkAjPYWof6WrG8DfS8ebHhjm4/MClMqMMHpQzehNk5HeM" crossorigin="anonymous";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@fortawesome/fontawesome-free@7.2.0/css/solid.min.css" integrity="sha384-LpGibDKKReBRP3epUOaN9WBgkrQ1pJIvrrTugkYxVQgDvYuMCRaXk6YkrZ/h3aWk" crossorigin="anonymous";
                link rel="stylesheet" href={(p) "/static/powpow.css?v=" (v)};
                @if let Some(fav) = favicon_block {
                    @if let Some(img_id) = fav.image_id {
                        link rel="icon" href=(format!("{p}/content-images/{img_id}"));
                    }
                }
                (extra_head)
            }
            body data-prefix=(p) {
                (nav)
                (content)
                (extra_scripts)
                script defer src={(p) "/static/powpow.js?v=" (v)} {}
                footer .footer.footer-station {
                    div .container {
                        div .columns {
                            // Column 1: Summer link
                            div .column.is-one-third {
                                @if let Some(contents) = effective_footer {
                                    (render_content_block(contents.get("footer-summer"), p, "h4", "title is-5 footer-heading"))
                                }
                            }
                            // Column 2: Calendar
                            div .column.is-one-third {
                                @if let Some(contents) = effective_footer {
                                    (render_content_block(contents.get("footer-calendar"), p, "h4", "title is-5 footer-heading"))
                                }
                            }
                            // Column 3: Contact + partners
                            div .column.is-one-third {
                                @if let Some(contents) = effective_footer {
                                    (render_content_block(contents.get("footer-contact"), p, "h4", "title is-5 footer-heading"))
                                }
                                p .mt-3 {
                                    a #open-contact-modal .contact-link href="#" {
                                        span .icon.mr-1 { i .fa-solid.fa-envelope {} }
                                        "Envoyez-nous un mail !"
                                    }
                                }
                                h4 .title.is-6.footer-heading.mt-4 { "Liens utiles" }
                                p {
                                    a href={(p) "/privacy"} { "Politique de confidentialité" }
                                    " · "
                                    a href={(p) "/tos"} { "Conditions d'utilisation" }
                                }
                            }
                        }
                        div .content.has-text-centered.mt-4 {
                            p .is-size-7 {
                                a href="https://codeberg.org/bestouff/powpow" {
                                    "PowPow"
                                } " v" (env!("CARGO_PKG_VERSION")) " pour " (get_entity_name())
                            }
                        }
                    }
                }

                // ── Contact email modal ──────────────────────────────────
                div #contact-modal .modal {
                    div .modal-background {}
                    div .modal-card.contact-modal-card {
                        header .modal-card-head.contact-modal-head {
                            p .modal-card-title { "Envoyez-nous un message" }
                            button #close-contact-modal .delete aria-label="Fermer" {}
                        }
                        section .modal-card-body.contact-modal-body {
                            form #contact-form {
                                div .field {
                                    label .label.contact-label { "Votre nom" }
                                    div .control.has-icons-left {
                                        input #contact-name .input name="name" type="text"
                                              placeholder="Prénom Nom" required="true";
                                        span .icon.is-left { i .fa-solid.fa-user {} }
                                    }
                                }
                                div .field {
                                    label .label.contact-label { "Votre email" }
                                    div .control.has-icons-left {
                                        input #contact-email .input name="email" type="email"
                                              placeholder="vous@exemple.fr" required="true";
                                        span .icon.is-left { i .fa-solid.fa-envelope {} }
                                    }
                                }
                                div .field {
                                    label .label.contact-label { "Objet" }
                                    div .control.has-icons-left {
                                        input #contact-subject .input name="subject" type="text"
                                              placeholder="Objet de votre message" required="true";
                                        span .icon.is-left { i .fa-solid.fa-tag {} }
                                    }
                                }
                                div .field {
                                    label .label.contact-label { "Message" }
                                    div .control {
                                        textarea #contact-message .textarea name="message"
                                                 placeholder="Votre message\u{2026}" rows="5" required="true" {}
                                    }
                                }
                            }
                        }
                        footer .modal-card-foot.contact-modal-foot {
                            div .buttons {
                                button #contact-submit .button.is-link {
                                    span .icon { i .fa-solid.fa-paper-plane {} }
                                    span { "Envoyer" }
                                }
                                button #cancel-contact-modal .button.is-danger { "Annuler" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_phone_international;

    #[test]
    fn french_mobile_national_format() {
        // 10-digit French mobile without country code
        assert_eq!(
            format_phone_international("0612345678"),
            "+33 6 12 34 56 78"
        );
    }

    #[test]
    fn french_mobile_with_country_code() {
        assert_eq!(
            format_phone_international("+33612345678"),
            "+33 6 12 34 56 78"
        );
    }

    #[test]
    fn french_landline() {
        // French landline (01 = Île-de-France)
        assert_eq!(
            format_phone_international("0145678901"),
            "+33 1 45 67 89 01"
        );
    }

    #[test]
    fn french_number_with_spaces() {
        assert_eq!(
            format_phone_international("06 12 34 56 78"),
            "+33 6 12 34 56 78"
        );
    }

    #[test]
    fn french_number_with_dots() {
        assert_eq!(
            format_phone_international("06.12.34.56.78"),
            "+33 6 12 34 56 78"
        );
    }

    #[test]
    fn international_non_french() {
        // Swiss number — should be parsed correctly since it has +41 prefix
        assert_eq!(
            format_phone_international("+41446681800"),
            "+41 44 668 18 00"
        );
    }

    #[test]
    fn empty_string() {
        assert_eq!(format_phone_international(""), "");
    }

    #[test]
    fn garbage_returns_original() {
        assert_eq!(format_phone_international("not a phone"), "not a phone");
    }
}
