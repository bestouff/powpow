mod admin;
mod auth;
mod calendar;
mod cash;
mod content;
mod content_admin;
mod home;
mod membership;
mod photos;
mod staff;
mod static_pages;

pub use admin::{
    admin_page, audit_page, qualifications_page, restore_page, restore_result, validation_page,
};
pub use auth::login_page;
pub use calendar::{calendar, calendar_editor, render_upcoming_week_email};
pub use cash::{cash_form, cash_import_form, cash_list};
pub use content::render_content_block;
pub use content_admin::{content_edit_page, content_list_page};
pub use home::index;
pub use membership::{
    already_imported_page, import_result, import_staff_form, membership_list_with_filters,
    user_detail,
};
pub use photos::photo_page;
pub use staff::{person_detail, staff_list};
pub use static_pages::static_page;

use crate::models::{ContentBlock, ContentMap, StaffMatchType, StaffWithSeason};
use maud::{DOCTYPE, Markup, PreEscaped, html};

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
use phonenumber::Mode;

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
    match phonenumber::parse(Some(phonenumber::country::Id::FR), phone) {
        Ok(number) => number.format().mode(Mode::International).to_string(),
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
                          href={(p) "/calendar"} {
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
                            // Column 1: Contact + partners
                            div .column.is-one-third {
                                @if let Some(contents) = effective_footer {
                                    (render_content_block(contents.get("footer-contact"), p, "h4", "title is-5 footer-heading"))
                                }
                                h4 .title.is-6.footer-heading.mt-4 { "Liens utiles" }
                                p {
                                    a href={(p) "/privacy"} { "Politique de confidentialité" }
                                    " · "
                                    a href={(p) "/tos"} { "Conditions d'utilisation" }
                                }
                            }
                            // Column 2: Calendar
                            div .column.is-one-third {
                                @if let Some(contents) = effective_footer {
                                    (render_content_block(contents.get("footer-calendar"), p, "h4", "title is-5 footer-heading"))
                                }
                            }
                            // Column 3: Summer link
                            div .column.is-one-third {
                                @if let Some(contents) = effective_footer {
                                    (render_content_block(contents.get("footer-summer"), p, "h4", "title is-5 footer-heading"))
                                }
                            }
                        }
                        div .content.has-text-centered.mt-4 {
                            p .is-size-7 {
                                a href="https://codeberg.org/bestouff/powpow" {
                                    "PowPow"
                                } " v" (env!("CARGO_PKG_VERSION")) " pour AG'HIL"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Shared context for rendering an import form (membership or cash).
struct ImportContext {
    /// Capitalized first name from the source data.
    first_name: String,
    /// Capitalized last name from the source data.
    last_name: String,
    /// Primary email from the source (beneficiary / cash).
    primary_email: String,
    /// Optional secondary payer email (membership only).
    payer_email: String,
    /// Default email to pre-fill when there is only one choice.
    default_email: String,
    /// Formatted phone number.
    phone: String,
    /// Default comment pre-filled in the comment textarea.
    default_comment: String,
    /// Whether this is a donation (affects double-subscription label).
    is_donation: bool,
    /// Whether creating a new staff is disallowed (name already exists).
    allow_create: bool,
    /// Radio value for the "source" name choice ("membership" / "cash").
    name_choice_value: &'static str,
    /// Label for the source name choice ("De l'adhésion:" / "Du paiement:").
    name_choice_label: &'static str,
    /// Page title in the browser tab.
    page_title: &'static str,
    /// Heading shown at the top of the page.
    page_heading: &'static str,
    /// Detail box title ("Détails de l'Adhésion" / "Détails du paiement").
    detail_title: &'static str,
    /// Back-link target path suffix ("/online" / "/cash").
    back_suffix: &'static str,
    /// Active nav item ("" / "cash").
    nav_active: &'static str,
    /// Rows for the left-column detail table: (label, value).
    detail_rows: Vec<(&'static str, String)>,
}

/// Shared rendering for both membership and cash import forms.
#[allow(clippy::too_many_lines)]
fn render_import_form(ctx: &ImportContext, candidates: &[StaffWithSeason], prefix: &str) -> Markup {
    let has_exact_match = candidates.iter().any(|c| {
        matches!(
            c.match_type,
            StaffMatchType::ExactBoth | StaffMatchType::ExactEmail | StaffMatchType::ExactName
        )
    });

    let has_double_subscription = candidates
        .iter()
        .any(|c| c.match_type == StaffMatchType::DoubleSubscription);

    let recommend_double_subscription = ctx.is_donation && has_double_subscription;
    let recommend_create = !has_exact_match && !recommend_double_subscription && ctx.allow_create;
    let allow_create = ctx.allow_create;

    let mut is_first = !recommend_create && !recommend_double_subscription;
    let mut is_first_double_subscription = recommend_double_subscription;
    let mut option_index = 0usize;

    let primary_email = &ctx.primary_email;
    let payer_email = &ctx.payer_email;

    // Render candidate options
    let candidates_markup = html! {
        @for candidate in candidates {
            @let staff = &candidate.staff;
            @let match_label = match candidate.match_type {
                StaffMatchType::ExactBoth => "Email et nom identiques",
                StaffMatchType::ExactName => "Nom identique",
                StaffMatchType::ExactEmail => "Email identique",
                StaffMatchType::PayerEmailMatch => "Email payeur identique",
                StaffMatchType::SimilarEmail => "Email similaire",
                StaffMatchType::SimilarName => "Nom similaire",
                StaffMatchType::DoubleSubscription => {
                    if ctx.is_donation { "Adhésion + don détecté" } else { "Double adhésion probable" }
                },
            };
            @let season_info = candidate.latest_season.map_or_else(
                || "Aucune saison".to_string(),
                |s| format!("Dernière saison: {}", s),
            );
            @let is_exact_match = matches!(
                candidate.match_type,
                StaffMatchType::ExactBoth | StaffMatchType::ExactEmail | StaffMatchType::ExactName
            );
            @let (highlight, recommended_tag, border_color) = if candidate.match_type == StaffMatchType::DoubleSubscription && is_first_double_subscription {
                ("is-primary", Some(("is-success", "Probable meilleure option")), "var(--bulma-primary)")
            } else if candidate.match_type == StaffMatchType::DoubleSubscription {
                ("is-danger", Some(("is-danger", "Double adhésion")), "var(--bulma-danger)")
            } else if is_first && is_exact_match {
                ("is-primary", Some(("is-success", "Probable meilleure option")), "var(--bulma-primary)")
            } else if is_exact_match {
                ("is-info", Some(("is-warning", "Option envisageable")), "var(--bulma-info)")
            } else {
                ("is-light", None, "var(--bulma-border)")
            };
            @let names_match = ctx.first_name.to_lowercase() == staff.first_name.to_lowercase()
                && ctx.last_name.to_lowercase() == staff.last_name.to_lowercase();
            @let staff_email_lower = staff.email.to_lowercase();
            @let primary_lower = primary_email.to_lowercase();
            @let payer_lower = payer_email.to_lowercase();
            @let bg_color = if option_index.is_multiple_of(2) { "var(--bulma-scheme-main)" } else { "var(--bulma-scheme-main-bis)" };

            div .box.mb-4.candidate-card style=(format!("--card-border:{};--card-bg:{}", border_color, bg_color)) {
                form method="POST" {
                    input type="hidden" name="action" value="update";
                    input type="hidden" name="staff_id" value=(staff.id);

                    div .level.mb-3 {
                        div .level-left {
                            span class={"tag " (highlight)} { (match_label) }
                            @if let Some((tag_class, tag_text)) = recommended_tag {
                                span class={"tag " (tag_class) " ml-2"} { (tag_text) }
                            }
                        }
                        div .level-right {
                            span .tag.is-info.is-light { (season_info) }
                        }
                    }

                    p .mb-3 {
                        strong { "Staff existant:" }
                        " " (staff.first_name) " " (staff.last_name) " <" (staff.email) ">"
                    }

                    // Name choice
                    @if names_match {
                        input type="hidden" name="first_name" value=(ctx.first_name);
                        input type="hidden" name="last_name" value=(ctx.last_name);
                    } @else {
                        div .field {
                            label .label { "Garder le prénom et nom" }
                            div .control {
                                label .radio {
                                    input type="radio" name="name_choice" value=(ctx.name_choice_value) checked
                                        onchange=(format!("updateNameFields(this.form, '{}', '{}')", ctx.first_name, ctx.last_name));
                                    " " (ctx.name_choice_label) " " strong { (ctx.first_name) " " (ctx.last_name) }
                                }
                                br;
                                label .radio {
                                    input type="radio" name="name_choice" value="staff"
                                        onchange=(format!("updateNameFields(this.form, '{}', '{}')", staff.first_name, staff.last_name));
                                    " Du staff: " strong { (staff.first_name) " " (staff.last_name) }
                                }
                            }
                        }
                        input type="hidden" name="first_name" value=(ctx.first_name);
                        input type="hidden" name="last_name" value=(ctx.last_name);
                    }

                    // Email choice - collect unique emails
                    @let unique_emails = {
                        let mut emails: Vec<(&str, &str, &str)> = Vec::new();
                        if !primary_email.is_empty() {
                            emails.push((ctx.name_choice_value, "Du bénéficiaire", primary_email));
                        }
                        if !payer_email.is_empty() && payer_lower != primary_lower {
                            emails.push(("payer", "Du payeur", payer_email));
                        }
                        if staff_email_lower != primary_lower && staff_email_lower != payer_lower {
                            emails.push(("staff", "Du staff", &staff.email));
                        }
                        emails
                    };

                    @if unique_emails.len() <= 1 {
                        @let email_value = if !primary_email.is_empty() {
                            primary_email.as_str()
                        } else if !payer_email.is_empty() {
                            payer_email.as_str()
                        } else {
                            &staff.email
                        };
                        input type="hidden" name="email" value=(email_value);
                    } @else {
                        div .field {
                            label .label { "Garder l'email" }
                            div .control {
                                @for (i, (value, label, display)) in unique_emails.iter().enumerate() {
                                    label .radio {
                                        input type="radio" name="email_choice" value=(value)
                                            checked[i == 0]
                                            onchange=(format!("updateEmailField(this.form, '{}')", display));
                                        " " (label) ": " strong { (display) }
                                    }
                                    br;
                                }
                            }
                        }
                        @let default_email_val = unique_emails.first().map_or(ctx.default_email.as_str(), |(_, _, d)| *d);
                        input type="hidden" name="email" value=(default_email_val);
                    }

                    input type="hidden" name="phone" value=(ctx.phone);

                    div .field {
                        label .label { "Commentaire" }
                        div .control {
                            textarea .textarea name="comment" rows="2" { (ctx.default_comment) }
                        }
                    }

                    div .field {
                        div .control {
                            button type="submit" class={"button " (highlight) " is-fullwidth"} {
                                span .icon { i .fa-solid.fa-arrows-rotate {} }
                                span { "Mettre à jour ce staff" }
                            }
                        }
                    }
                }
            }

            @let () = {
                is_first = false;
                if candidate.match_type == StaffMatchType::DoubleSubscription {
                    is_first_double_subscription = false;
                }
                option_index += 1;
            };
        }
    };

    // Create new staff option
    let create_highlight = if recommend_create {
        "is-primary"
    } else {
        "is-light"
    };
    let create_border = if recommend_create {
        "var(--bulma-primary)"
    } else {
        "var(--bulma-border)"
    };
    let create_bg_color = if option_index.is_multiple_of(2) {
        "var(--bulma-scheme-main)"
    } else {
        "var(--bulma-scheme-main-bis)"
    };

    let create_markup = html! {
        div .box.mb-4.candidate-card style=(format!("--card-border:{};--card-bg:{}", create_border, create_bg_color)) {
            form method="POST" {
                input type="hidden" name="action" value="create";

                div .level.mb-3 {
                    div .level-left {
                        span class={"tag " (create_highlight)} { "Nouveau staff" }
                        @if recommend_create {
                            span .tag.is-success.ml-2 { "Probable meilleure option" }
                        }
                    }
                }

                div .columns {
                    div .column {
                        div .field {
                            label .label { "Prénom" }
                            div .control {
                                input .input type="text" name="first_name" value=(ctx.first_name);
                            }
                        }
                    }
                    div .column {
                        div .field {
                            label .label { "Nom" }
                            div .control {
                                input .input type="text" name="last_name" value=(ctx.last_name);
                            }
                        }
                    }
                }

                // Email choice for create form
                @if !primary_email.is_empty() && !payer_email.is_empty() && primary_email != payer_email {
                    div .field {
                        label .label { "Email" }
                        div .control {
                            label .radio {
                                input type="radio" name="email_choice" value=(ctx.name_choice_value) checked
                                    onchange=(format!("document.getElementById('create_email').value='{}'", primary_email));
                                " Du bénéficiaire: " strong { (primary_email) }
                            }
                            br;
                            label .radio {
                                input type="radio" name="email_choice" value="payer"
                                    onchange=(format!("document.getElementById('create_email').value='{}'", payer_email));
                                " Du payeur: " strong { (payer_email) }
                            }
                        }
                        input type="hidden" #create_email name="email" value=(primary_email);
                    }
                } @else if primary_email.is_empty() && !payer_email.is_empty() {
                    div .field {
                        label .label { "Email (du payeur)" }
                        div .control {
                            input .input type="email" name="email" value=(payer_email);
                        }
                    }
                } @else {
                    div .field {
                        label .label { "Email" }
                        div .control {
                            input .input type="email" name="email" value=(ctx.default_email);
                        }
                    }
                }

                div .field {
                    label .label { "Téléphone" }
                    div .control {
                        input .input type="tel" name="phone" value=(ctx.phone);
                    }
                }

                div .field {
                    label .label { "Commentaire" }
                    div .control {
                        textarea .textarea name="comment" rows="2" { (ctx.default_comment) }
                    }
                }

                div .field {
                    div .control {
                        button type="submit" class={"button " (create_highlight) " is-fullwidth"} {
                            span .icon { i .fa-solid.fa-plus {} }
                            span { "Créer un nouveau staff" }
                        }
                    }
                }
            }
        }
    };

    // Combine options in the right order based on recommendation
    let options_markup = if !allow_create {
        html! { (candidates_markup) }
    } else if recommend_create {
        html! { (create_markup) (candidates_markup) }
    } else {
        html! { (candidates_markup) (create_markup) }
    };

    let total_options = candidates.len() + usize::from(allow_create);

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .level.mb-5 {
                    div .level-left {
                        h1 .title.is-3 { (ctx.page_heading) }
                    }
                    div .level-right {
                        a .button.is-light href=(format!("{prefix}{}", ctx.back_suffix)) {
                            span .icon { i .fa-solid.fa-arrow-left {} }
                            span { "Retour" }
                        }
                    }
                }

                div .columns {
                    div .column.is-5 {
                        div .box {
                            h2 .title.is-4.mb-4 { (ctx.detail_title) }
                            div .content {
                                table .table.is-fullwidth {
                                    tbody {
                                        @for (label, value) in &ctx.detail_rows {
                                            tr { th { (label) } td { (PreEscaped(value)) } }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div .column.is-7 {
                        h2 .title.is-4.mb-4 { "Options d'import" }
                        @if total_options > 1 {
                            div .notification.is-danger.mb-4 {
                                span .icon { i .fa-solid.fa-triangle-exclamation {} }
                                strong { "Attention" }
                                ", il y a plusieurs possibilités, examinez-les bien avant de choisir la bonne."
                            }
                        }
                        (options_markup)
                    }
                }
            }
        }
    };

    page(
        ctx.page_title,
        prefix,
        &NavKind::Standard,
        ctx.nav_active,
        html! {},
        content,
        html! {},
    )
}
