use crate::models::{
    Atelier, Cash, Membership, MembershipWithStatus, Need, PhotoMeta, Role, Staff, StaffMatchType,
    StaffWithSeason, User,
};
use chrono::Datelike;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use phonenumber::Mode;
use std::collections::HashMap;
use std::sync::RwLock;

/// Global photo-of-the-day URL + photographer name, updated when photos change.
static PHOTO_BG_URL: RwLock<Option<String>> = RwLock::new(None);
static PHOTO_BG_AUTHOR: RwLock<Option<String>> = RwLock::new(None);

pub fn set_photo_bg(url: String, photographer: String) {
    if let Ok(mut w) = PHOTO_BG_URL.write() {
        *w = Some(url);
    }
    if let Ok(mut w) = PHOTO_BG_AUTHOR.write() {
        *w = Some(photographer);
    }
}

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
    Full,      // Adhésions, Cash, Staff, API, Login
    Standard,  // Adhésions, Cash, Staff, Login
    LoginOnly, // Only Login button
    StaffOnly, // Only Staff-related items
}

fn navbar(prefix: &str, kind: &NavKind, active: &str) -> Markup {
    let admin_hide = matches!(kind, NavKind::LoginOnly);
    let p = prefix;

    html! {
        nav .navbar.is-dark role="navigation" aria-label="main navigation" {
            div .container.is-fluid {
                div .navbar-brand {
                    a .navbar-item href={(p) "/"} {
                        span .icon.mr-2 { i .fa-solid.fa-person-skiing {} }
                        strong { "PowPow pour AGH'IL" }
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
                        a .navbar-item.navbar-admin .is-active[active == "users"]
                          href={(p) "/users"}
                          style=[admin_hide.then_some("display:none")] {
                            span .icon.mr-1 { i .fa-solid.fa-ticket {} }
                            "Adhésions"
                            span .nav-badge.d-none data-badge="users" {}
                        }
                        a .navbar-item.navbar-admin .is-active[active == "cash"]
                          href={(p) "/cash"}
                          style=[admin_hide.then_some("display:none")] {
                            span .icon.mr-1 { i .fa-solid.fa-money-bill-wave {} }
                            "Espèces / Chèques"
                            span .nav-badge.d-none data-badge="cash" {}
                        }
                        a .navbar-item.navbar-admin .is-active[active == "staff"]
                          href={(p) "/staff"}
                          style=[admin_hide.then_some("display:none")] {
                            span .icon.mr-1 { i .fa-solid.fa-user-group {} }
                            "Staff"
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

fn page(
    title: &str,
    prefix: &str,
    nav_kind: &NavKind,
    active: &str,
    extra_head: Markup,
    content: Markup,
    extra_scripts: Markup,
) -> Markup {
    let p = prefix;
    let nav = navbar(prefix, nav_kind, active);

    let photo_bg_css = PHOTO_BG_URL
        .read()
        .ok()
        .and_then(|r| r.clone())
        .map(|url| {
            format!(
                "body{{\
                    background-image:linear-gradient(rgba(255,255,255,0.15),rgba(255,255,255,0.15)),url('{p}{url}');\
                    background-size:cover;\
                    background-position:center;\
                    background-attachment:fixed;\
                    min-height:100vh;\
                }}\
                .section,.box,.footer{{\
                    background-color:rgba(255,255,255,0.65);\
                }}"
            )
        });

    let photo_credit = PHOTO_BG_AUTHOR.read().ok().and_then(|r| r.clone());

    html! {
        (DOCTYPE)
        html lang="fr" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                meta name="google-site-verification" content="S04nKUrv5gsWl0VqBBdd9Q6zS7rxLWHJLc2aFftaD4E";
                title { (title) }
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bulma@1.0.4/css/bulma.min.css";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@fortawesome/fontawesome-free@7.2.0/css/fontawesome.min.css";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@fortawesome/fontawesome-free@7.2.0/css/solid.min.css";
                link rel="stylesheet" href={(p) "/static/powpow.css"};
                @if let Some(ref bg_css) = photo_bg_css {
                    style { (PreEscaped(bg_css)) }
                }
                (extra_head)
            }
            body data-prefix=(p) {
                (nav)
                (content)
                (extra_scripts)
                script defer src={(p) "/static/powpow.js"} {}
                footer .footer.py-4 {
                    div .content.has-text-centered {
                        p .is-size-7.has-text-grey {
                            "PowPow v" (env!("CARGO_PKG_VERSION")) " pour AG'HIL, \u{00a9}2026 Xavier Bestel <xav@bes.tel> \u{2014} "
                            a href={(p) "/privacy"} { "Confidentialité" }
                            " \u{00b7} "
                            a href={(p) "/tos"} { "CGU" }
                        }
                        @if let Some(ref name) = photo_credit {
                            p .is-size-7.has-text-grey {
                                "photo \u{00a9} " (name)
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn index(
    prefix: &str,
    staff: Option<&Staff>,
    current_season: i16,
    has_paid: bool,
    chief_ateliers: &[Atelier],
    upcoming: &[(chrono::NaiveDate, String, i16, i64)],
) -> Markup {
    let p = prefix;
    let season_display = format!("{}-{}", current_season - 1, current_season);

    let extra_head = html! {};

    let content = html! {
        // Hero
        section .hero.is-info {
            div .hero-body {
                div .container.has-text-centered {
                    h1 .title.is-2.mb-4 {
                        span .icon.is-large { i .fa-solid.fa-person-skiing.fa-2x {} }
                        br;
                        "Gestionnaire de plannings bénévoles"
                    }
                    h2 .subtitle.is-5 {
                        "PowPow (Pistes, Organisation, Week-end, Planning, Optimisation, Wouah!) pour AG'HIL"
                    }
                }
            }
        }

        @if let Some(staff) = staff {
            // Membership status
            section .section.py-4 {
                div .container.is-fluid {
                    @if has_paid {
                        div .notification.is-success.is-light {
                            span .icon { i .fa-solid.fa-circle-check {} }
                            " Ta cotisation est à jour pour la saison " (season_display) "."
                        }
                    } @else {
                        div .notification.is-warning.is-light {
                            span .icon { i .fa-solid.fa-triangle-exclamation {} }
                            " Ta cotisation n'est pas à jour pour la saison " (season_display)
                            " \u{2014} "
                            a href="https://www.helloasso.com/associations/agir-pour-la-station-de-ski-de-st-hil"
                              target="_blank" { "inscris-toi sur HelloAsso" }
                            "."
                        }
                    }
                }
            }

            // My profile
            section .section.py-4 {
                div .container.is-fluid {
                    a .box href={(p) "/person/" (staff.id)} {
                        span .icon.mr-2 { i .fa-solid.fa-user-gear {} }
                        strong { "Gérer mes ateliers et mes préférences" }
                    }
                }
            }

            // Chief ateliers
            @if !chief_ateliers.is_empty() {
                section .section.py-4 {
                    div .container.is-fluid {
                        div .box {
                            h3 .title.is-5.mb-3 {
                                span .icon.mr-2 { i .fa-solid.fa-user-shield {} }
                                "Mes ateliers"
                            }
                            div .buttons {
                                @for a in chief_ateliers {
                                    a .button.is-link.is-light.mr-2.mb-2
                                      href={(p) "/calendar/" (a.slug)} {
                                        span .icon { i class={"fa-solid fa-" (a.icon)} {} }
                                        "\u{00a0}"
                                        span { (a.name) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Upcoming week
        section .section.py-4 {
            div .container.is-fluid {
                a .box.box-link href={(p) "/calendar"} {
                    h3 .title.is-5.mb-3 {
                        span .icon.mr-2 { i .fa-solid.fa-calendar-week {} }
                        "Semaine à venir"
                    }
                    (render_upcoming_week(upcoming))
                }
            }
        }

        @if let Some(staff) = staff {
            // Admin sections
            @if staff.is_admin {
                section .section.py-4 {
                    div .container.is-fluid {
                        div .box {
                            h3 .title.is-5.mb-3 {
                                span .icon.mr-2 { i .fa-solid.fa-ticket {} }
                                "Gestion des adhésions"
                            }
                            div .buttons {
                                a .button.is-primary href={(p) "/users"} {
                                    span .icon { i .fa-solid.fa-ticket {} }
                                    span { "Adhésions HelloAsso" }
                                    span .nav-badge.d-none data-badge="users" {}
                                }
                                a .button.is-primary.is-light href={(p) "/cash"} {
                                    span .icon { i .fa-solid.fa-money-bill-wave {} }
                                    span { "Espèces / Chèques" }
                                    span .nav-badge.d-none data-badge="cash" {}
                                }
                            }
                        }
                    }
                }
                section .section.py-4 {
                    div .container.is-fluid {
                        div .box {
                            h3 .title.is-5.mb-3 {
                                span .icon.mr-2 { i .fa-solid.fa-user-group {} }
                                "Gestion du staff"
                            }
                            div .buttons {
                                a .button.is-link href={(p) "/staff"} {
                                    span .icon { i .fa-solid.fa-user-group {} }
                                    span { "Voir le staff" }
                                }
                                a .button.is-link.is-light href={(p) "/export/mailchimp"} {
                                    span .icon { i .fa-solid.fa-file-csv {} }
                                    span { "Export Mailchimp" }
                                }
                                a .button.is-light href={(p) "/audit"} {
                                    span .icon { i .fa-solid.fa-clipboard-list {} }
                                    span { "Journal d'audit" }
                                }
                            }
                        }
                    }
                }
            }

            // God: backup/restore
            @if staff.is_god {
                section .section.py-4 {
                    div .container.is-fluid {
                        div .box {
                            h3 .title.is-5.mb-3 {
                                span .icon.mr-2 { i .fa-solid.fa-database {} }
                                "Sauvegarde / Restauration"
                            }
                            div .buttons {
                                a .button.is-warning href={(p) "/backup"} {
                                    span .icon { i .fa-solid.fa-download {} }
                                    span { "Télécharger la sauvegarde" }
                                }
                                a .button.is-danger href={(p) "/restore"} {
                                    span .icon { i .fa-solid.fa-upload {} }
                                    span { "Restaurer" }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    page(
        "PowPow for AGH'IL",
        prefix,
        &NavKind::LoginOnly,
        "",
        extra_head,
        content,
        html! {},
    )
}

#[allow(clippy::too_many_arguments)]
pub fn membership_list_with_filters(
    memberships_with_status: Vec<(User, MembershipWithStatus)>,
    search: Option<String>,
    only_not_imported: bool,
    total_count: usize,
    imported_count: usize,
    not_imported_count: usize,
    current_season: i16,
    prefix: &str,
) -> Markup {
    let p = prefix;
    let displayed_count = memberships_with_status.len();
    let search_value = search.as_deref().unwrap_or("");
    let has_filters = search.is_some() || only_not_imported;

    let total_card_active = if !only_not_imported && search.is_none() {
        " is-active"
    } else {
        ""
    };
    let not_imported_card_active = if only_not_imported { " is-active" } else { "" };
    let filter_all_class = if only_not_imported {
        "is-light"
    } else {
        "is-primary"
    };
    let filter_not_imported_class = if only_not_imported {
        "is-primary"
    } else {
        "is-light"
    };

    let extra_head = html! {};

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .level.mb-4 {
                    div .level-left {
                        h1 .title.is-3 { "Adhésions HelloAsso" }
                    }
                    div .level-right {
                        a .button.is-primary href={(p) "/sync"} {
                            span .icon { i .fa-solid.fa-arrows-rotate {} }
                            span { "Synchronisation manuelle" }
                        }
                    }
                }

                // Stats Cards
                div .columns.mb-4 {
                    div .column.is-4 {
                        a class={"box stat-card has-text-centered" (total_card_active)}
                          href={(p) "/users"} {
                            span .icon.is-large.has-text-info {
                                i .fa-solid.fa-ticket.fa-2x {}
                            }
                            p .stat-number.has-text-info.mt-2 { (total_count) }
                            p .has-text-grey { "Total adhésions" }
                        }
                    }
                    div .column.is-4 {
                        a .box.stat-card.has-text-centered href={(p) "/users?filter=all"} {
                            span .icon.is-large.has-text-success {
                                i .fa-solid.fa-circle-check.fa-2x {}
                            }
                            p .stat-number.has-text-success.mt-2 { (imported_count) }
                            p .has-text-grey { "Importées" }
                        }
                    }
                    div .column.is-4 {
                        a class={"box stat-card has-text-centered" (not_imported_card_active)}
                          href={(p) "/users?filter=not_imported"} {
                            span .icon.is-large.has-text-warning {
                                i .fa-solid.fa-circle-exclamation.fa-2x {}
                            }
                            p .stat-number.has-text-warning.mt-2 { (not_imported_count) }
                            p .has-text-grey { "À importer" }
                        }
                    }
                }

                // Search and Filter Box
                div .box.mb-4 {
                    form #filterForm method="GET" action={(p) "/users"} {
                        div .columns.is-vcentered {
                            div .column.is-6 {
                                div .field.has-addons {
                                    div .control.is-expanded.has-icons-left {
                                        input .input #searchInput type="text" name="search"
                                            placeholder="Rechercher par email, nom ou prénom..."
                                            value=(search_value);
                                        span .icon.is-left {
                                            i .fa-solid.fa-magnifying-glass {}
                                        }
                                    }
                                    div .control {
                                        button .button.is-info type="submit" {
                                            span .icon { i .fa-solid.fa-magnifying-glass {} }
                                        }
                                    }
                                }
                            }
                            div .column.is-6 {
                                div .field.is-grouped.is-grouped-right {
                                    div .control {
                                        div .buttons.has-addons {
                                            a class={"button " (filter_all_class) " is-medium"}
                                              href={(p) "/users?search=" (search_value)} {
                                                span .icon { i .fa-solid.fa-list {} }
                                                span { "Toutes" }
                                            }
                                            a class={"button " (filter_not_imported_class) " is-medium"}
                                              href={(p) "/users?search=" (search_value) "&filter=not_imported"} {
                                                span .icon { i .fa-solid.fa-circle-exclamation {} }
                                                span { "À importer" }
                                            }
                                        }
                                    }
                                    @if has_filters {
                                        a .button.is-light.is-small.ml-2 href={(p) "/users"} {
                                            span .icon { i .fa-solid.fa-xmark {} }
                                            span { "Effacer filtres" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Results Box
                div .box {
                    div .level.mb-3 {
                        div .level-left {
                            p { strong { (displayed_count) } " adhésion(s) affichée(s)" }
                        }
                    }
                    div .table-container {
                        table .table.is-fullwidth.is-striped.is-hoverable {
                            thead {
                                tr {
                                    th { "Bénéficiaire" }
                                    th { "Email" }
                                    th { "Téléphone" }
                                    th { "Type" }
                                    th .has-text-right { "Montant" }
                                    th { "Date" }
                                    th { "Saison" }
                                    th { "Statut" }
                                }
                            }
                            tbody {
                                @for (_user, membership_with_status) in &memberships_with_status {
                                    @let membership = &membership_with_status.membership;
                                    @let beneficiary_name = {
                                        let n = format!(
                                            "{} {}",
                                            membership.beneficiary_first_name.as_deref().unwrap_or(""),
                                            membership.beneficiary_last_name.as_deref().unwrap_or("")
                                        );
                                        n
                                    };
                                    @let (type_label, type_class) = match membership.item_type.as_deref() {
                                        Some("Donation") => ("Don", "is-info"),
                                        Some("Membership") => ("Adhésion", "is-primary"),
                                        _ => ("?", "is-light"),
                                    };
                                    @let amount = membership.amount.map_or_else(
                                        || "N/A".to_string(),
                                        |a| format!("{:.2}€", a as f32 / 100.0),
                                    );
                                    @let order_date = membership.order_date.map_or_else(
                                        || "N/A".to_string(),
                                        |d| d.format("%d/%m/%Y").to_string(),
                                    );
                                    @let season = membership_with_status.season;
                                    @let season_tag_class = if season == current_season { "is-primary" } else { "is-light" };
                                    tr {
                                        td { strong { (beneficiary_name.trim()) } }
                                        td { (membership.email.as_deref().unwrap_or("")) }
                                        td { (membership.phone.as_deref().unwrap_or("")) }
                                        td { span class={"tag " (type_class)} { (type_label) } }
                                        td .has-text-right { strong .has-text-success { (amount) } }
                                        td { (order_date) }
                                        td { span class={"tag " (season_tag_class)} { (season) } }
                                        td {
                                            @if membership_with_status.is_double_subscription {
                                                span .tag.is-danger { "Double adhésion" }
                                            } @else if membership_with_status.has_staff {
                                                span .tag.is-success { "Importé" }
                                            } @else {
                                                a .tag.is-warning
                                                  href={(p) "/import/" (membership.helloasso_item_id)} {
                                                    "À importer"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let extra_scripts = html! {};

    page(
        "Liste des Adhésions - HelloAsso",
        prefix,
        &NavKind::Full,
        "users",
        extra_head,
        content,
        extra_scripts,
    )
}

pub fn already_imported_page(membership: Membership, season: i16, prefix: &str) -> Markup {
    let p = prefix;
    let beneficiary_first = membership.beneficiary_first_name.as_deref().unwrap_or("");
    let beneficiary_last = membership.beneficiary_last_name.as_deref().unwrap_or("");
    let beneficiary_name = format!("{beneficiary_first} {beneficiary_last}");
    let beneficiary_name = beneficiary_name.trim();

    let email = membership.email.as_deref().unwrap_or("N/A");
    let item_name = membership.item_name.as_deref().unwrap_or("N/A");
    let amount = membership.amount.map_or_else(
        || "N/A".to_string(),
        |a| format!("{:.2}€", a as f32 / 100.0),
    );
    let order_date = membership
        .order_date
        .map_or_else(|| "N/A".to_string(), |d| d.format("%d/%m/%Y").to_string());

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .columns.is-centered {
                    div .column.is-8 {
                        div .box.has-text-centered {
                            span .icon.is-large.has-text-success.mb-4 {
                                i .fa-solid.fa-circle-check.fa-4x {}
                            }
                            h1 .title.is-3.has-text-success { "Adhésion déjà importée" }
                            p .subtitle.is-5.mb-5 {
                                "Cette adhésion a déjà été importée dans le système pour la saison " (season) "."
                            }

                            div .box.box-alt-bg {
                                h2 .title.is-5.mb-4 { "Détails de l'adhésion" }
                                table .table.is-fullwidth {
                                    tbody {
                                        tr {
                                            th { "Bénéficiaire" }
                                            td { strong { (beneficiary_name) } }
                                        }
                                        tr {
                                            th { "Email" }
                                            td { (email) }
                                        }
                                        tr {
                                            th { "Article" }
                                            td { (item_name) }
                                        }
                                        tr {
                                            th { "Montant" }
                                            td { (amount) }
                                        }
                                        tr {
                                            th { "Date" }
                                            td { (order_date) }
                                        }
                                        tr {
                                            th { "Saison" }
                                            td { span .tag.is-success.is-medium { (season) } }
                                        }
                                    }
                                }
                            }

                            a .button.is-primary.is-medium.mt-4 href={(p) "/users"} {
                                span .icon { i .fa-solid.fa-arrow-left {} }
                                span { "Retour aux adhésions" }
                            }
                        }
                    }
                }
            }
        }
    };

    page(
        "Adhésion déjà importée - AGHIL",
        prefix,
        &NavKind::Standard,
        "",
        html! {},
        content,
        html! {},
    )
}

pub fn import_staff_form(
    membership: Membership,
    season: i16,
    candidates: Vec<StaffWithSeason>,
    payer_email: Option<&str>,
    name_already_exists: bool,
    prefix: &str,
) -> Markup {
    let beneficiary_first =
        capitalize_words(membership.beneficiary_first_name.as_deref().unwrap_or(""));
    let beneficiary_last =
        capitalize_words(membership.beneficiary_last_name.as_deref().unwrap_or(""));
    let beneficiary_name = format!("{} {}", beneficiary_first, beneficiary_last)
        .trim()
        .to_string();

    let membership_email = membership.email.as_deref().unwrap_or("").to_lowercase();
    let payer_email = payer_email.unwrap_or("").to_lowercase();
    let default_email = if membership_email.is_empty() {
        &payer_email
    } else {
        &membership_email
    };
    let phone = format_phone_international(membership.phone.as_deref().unwrap_or(""));
    let comment = membership.comment.as_deref().unwrap_or("");
    let item_name = membership.item_name.as_deref().unwrap_or("N/A");
    let amount = membership.amount.map_or_else(
        || "N/A".to_string(),
        |a| format!("{:.2}\u{20ac}", a as f32 / 100.0),
    );
    let order_date = membership
        .order_date
        .map_or_else(|| "N/A".to_string(), |d| d.format("%d/%m/%Y").to_string());

    let is_donation = membership.item_type.as_deref() == Some("Donation");

    let has_exact_match = candidates.iter().any(|c| {
        matches!(
            c.match_type,
            StaffMatchType::ExactBoth | StaffMatchType::ExactEmail | StaffMatchType::ExactName
        )
    });

    let has_double_subscription = candidates
        .iter()
        .any(|c| c.match_type == StaffMatchType::DoubleSubscription);

    let recommend_double_subscription = is_donation && has_double_subscription;
    let recommend_create =
        !has_exact_match && !recommend_double_subscription && !name_already_exists;
    let allow_create = !name_already_exists;

    let mut is_first = !recommend_create && !recommend_double_subscription;
    let mut is_first_double_subscription = recommend_double_subscription;
    let mut option_index = 0usize;

    // Render candidate options
    let candidates_markup = html! {
        @for candidate in &candidates {
            @let staff = &candidate.staff;
            @let match_label = match candidate.match_type {
                StaffMatchType::ExactBoth => "Email et nom identiques",
                StaffMatchType::ExactName => "Nom identique",
                StaffMatchType::ExactEmail => "Email identique",
                StaffMatchType::PayerEmailMatch => "Email payeur identique",
                StaffMatchType::SimilarEmail => "Email similaire",
                StaffMatchType::SimilarName => "Nom similaire",
                StaffMatchType::DoubleSubscription => {
                    if is_donation { "Adhésion + don détecté" } else { "Double adhésion probable" }
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
            @let names_match = beneficiary_first.to_lowercase() == staff.first_name.to_lowercase()
                && beneficiary_last.to_lowercase() == staff.last_name.to_lowercase();
            @let staff_email_lower = staff.email.to_lowercase();
            @let membership_email_lower = membership_email.to_lowercase();
            @let payer_email_lower = payer_email.to_lowercase();
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
                        input type="hidden" name="first_name" value=(beneficiary_first);
                        input type="hidden" name="last_name" value=(beneficiary_last);
                    } @else {
                        div .field {
                            label .label { "Garder le prénom et nom" }
                            div .control {
                                label .radio {
                                    input type="radio" name="name_choice" value="membership" checked
                                        onchange=(format!("updateNameFields(this.form, '{}', '{}')", beneficiary_first, beneficiary_last));
                                    " De l'adhésion: " strong { (beneficiary_first) " " (beneficiary_last) }
                                }
                                br;
                                label .radio {
                                    input type="radio" name="name_choice" value="staff"
                                        onchange=(format!("updateNameFields(this.form, '{}', '{}')", staff.first_name, staff.last_name));
                                    " Du staff: " strong { (staff.first_name) " " (staff.last_name) }
                                }
                            }
                        }
                        input type="hidden" name="first_name" value=(beneficiary_first);
                        input type="hidden" name="last_name" value=(beneficiary_last);
                    }

                    // Email choice - collect unique emails
                    @let unique_emails = {
                        let mut emails: Vec<(&str, &str, &str)> = Vec::new();
                        if !membership_email.is_empty() {
                            emails.push(("membership", "Du bénéficiaire", &membership_email));
                        }
                        if !payer_email.is_empty() && payer_email_lower != membership_email_lower {
                            emails.push(("payer", "Du payeur", &payer_email));
                        }
                        if staff_email_lower != membership_email_lower && staff_email_lower != payer_email_lower {
                            emails.push(("staff", "Du staff", &staff.email));
                        }
                        emails
                    };

                    @if unique_emails.len() <= 1 {
                        @let email_value = if !membership_email.is_empty() {
                            &membership_email
                        } else if !payer_email.is_empty() {
                            &payer_email
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
                        @let default_email_val = unique_emails.first().map_or(default_email.as_str(), |(_, _, d)| *d);
                        input type="hidden" name="email" value=(default_email_val);
                    }

                    input type="hidden" name="phone" value=(phone);

                    div .field {
                        label .label { "Commentaire" }
                        div .control {
                            textarea .textarea name="comment" rows="2" { (comment) }
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

            // Side-effect: advance mutable state after rendering each candidate
            // We use a dummy let to execute this
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
                                input .input type="text" name="first_name" value=(beneficiary_first);
                            }
                        }
                    }
                    div .column {
                        div .field {
                            label .label { "Nom" }
                            div .control {
                                input .input type="text" name="last_name" value=(beneficiary_last);
                            }
                        }
                    }
                }

                // Email choice for create form
                @if !membership_email.is_empty() && !payer_email.is_empty() && membership_email != payer_email {
                    div .field {
                        label .label { "Email" }
                        div .control {
                            label .radio {
                                input type="radio" name="email_choice" value="membership" checked
                                    onchange=(format!("document.getElementById('create_email').value='{}'", membership_email));
                                " Du bénéficiaire: " strong { (membership_email) }
                            }
                            br;
                            label .radio {
                                input type="radio" name="email_choice" value="payer"
                                    onchange=(format!("document.getElementById('create_email').value='{}'", payer_email));
                                " Du payeur: " strong { (payer_email) }
                            }
                        }
                        input type="hidden" #create_email name="email" value=(membership_email);
                    }
                } @else if membership_email.is_empty() && !payer_email.is_empty() {
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
                            input .input type="email" name="email" value=(default_email);
                        }
                    }
                }

                div .field {
                    label .label { "Téléphone" }
                    div .control {
                        input .input type="tel" name="phone" value=(phone);
                    }
                }

                div .field {
                    label .label { "Commentaire" }
                    div .control {
                        textarea .textarea name="comment" rows="2" { (comment) }
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

    let membership_email_display = if membership_email.is_empty() {
        "N/A"
    } else {
        &membership_email
    };

    let extra_head = html! {};

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .level.mb-5 {
                    div .level-left {
                        h1 .title.is-3 { "Importer un Staff" }
                    }
                    div .level-right {
                        a href=(format!("{prefix}/users")) .button.is-light {
                            span .icon { i .fa-solid.fa-arrow-left {} }
                            span { "Retour" }
                        }
                    }
                }

                div .columns {
                    div .column.is-5 {
                        div .box {
                            h2 .title.is-4.mb-4 { "Détails de l'Adhésion" }
                            div .content {
                                table .table.is-fullwidth {
                                    tbody {
                                        tr { th { "Bénéficiaire" } td { strong { (beneficiary_name) } } }
                                        tr { th { "Email bénéficiaire" } td { (membership_email_display) } }
                                        tr { th { "Email payeur" } td { (payer_email) } }
                                        tr { th { "Téléphone" } td { (phone) } }
                                        tr { th { "Article" } td { (item_name) } }
                                        tr { th { "Montant" } td { (amount) } }
                                        tr { th { "Date" } td { (order_date) } }
                                        tr { th { "Saison" } td { span .tag.is-info.is-medium { (season) } } }
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
        "Importer Staff - AGHIL",
        prefix,
        &NavKind::Standard,
        "",
        extra_head,
        content,
        html! {},
    )
}

pub fn user_detail(user: User, prefix: &str) -> Markup {
    let p = prefix;
    let full_name = format!(
        "{} {}",
        user.first_name.as_deref().unwrap_or(""),
        user.last_name.as_deref().unwrap_or("")
    );
    let full_name = full_name.trim();

    let email = &user.email;
    let phone = user.phone.as_deref().unwrap_or("N/A");
    let address = format!(
        "{} {}",
        user.address.as_deref().unwrap_or(""),
        user.city.as_deref().unwrap_or("")
    );
    let address = address.trim();
    let zip_code = user.zip_code.as_deref().unwrap_or("");
    let country = user.country.as_deref().unwrap_or("");
    let birth_date = user
        .birth_date
        .map_or_else(|| "N/A".to_string(), |d| d.format("%d/%m/%Y").to_string());
    let created_at = user.created_at.format("%d/%m/%Y à %H:%M").to_string();
    let updated_at = user.updated_at.format("%d/%m/%Y à %H:%M").to_string();
    let last_sync = user.last_sync_at.map_or_else(
        || "Jamais".to_string(),
        |d| d.format("%d/%m/%Y à %H:%M").to_string(),
    );

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .level.mb-5 {
                    div .level-left {
                        h1 .title.is-3 { "Détails de l'Utilisateur" }
                    }
                    div .level-right {
                        a .button.is-light href={(p) "/users"} {
                            span .icon { i .fa-solid.fa-arrow-left {} }
                            span { "Retour à la liste" }
                        }
                    }
                }

                div .box {
                    div .columns {
                        div .column.is-4 {
                            div .has-text-centered {
                                div .avatar-circle.is-size-1.mb-4.user-avatar {
                                    span .icon.is-large { i .fa-solid.fa-user.fa-3x {} }
                                }
                                h2 .title.is-4 { (full_name) }
                                p .subtitle.is-6.has-text-grey { (email) }
                            }
                        }
                        div .column.is-8 {
                            div .content {
                                h3 .title.is-5.mb-3 { "Informations Personnelles" }
                                div .columns.is-multiline {
                                    div .column.is-6 {
                                        div .field {
                                            label .label { "Téléphone" }
                                            div .control { (phone) }
                                        }
                                    }
                                    div .column.is-6 {
                                        div .field {
                                            label .label { "Date de naissance" }
                                            div .control { (birth_date) }
                                        }
                                    }
                                    div .column.is-12 {
                                        div .field {
                                            label .label { "Adresse" }
                                            div .control {
                                                (address) br; (zip_code) " " (country)
                                            }
                                        }
                                    }
                                }

                                h3 .title.is-5.mb-3.mt-5 { "Informations Système" }
                                div .columns.is-multiline {
                                    div .column.is-6 {
                                        div .field {
                                            label .label { "Email" }
                                            div .control { (email) }
                                        }
                                    }
                                    div .column.is-6 {
                                        div .field {
                                            label .label { "Créé le" }
                                            div .control { (created_at) }
                                        }
                                    }
                                    div .column.is-6 {
                                        div .field {
                                            label .label { "Dernière mise à jour" }
                                            div .control { (updated_at) }
                                        }
                                    }
                                    div .column.is-6 {
                                        div .field {
                                            label .label { "Dernière synchronisation" }
                                            div .control { (last_sync) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let title = format!("Détails de l'Utilisateur - {full_name}");
    page(
        &title,
        prefix,
        &NavKind::Full,
        "",
        html! {},
        content,
        html! {},
    )
}

pub fn restore_page(prefix: &str) -> Markup {
    let p = prefix;

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .columns.is-centered {
                    div .column.is-8 {
                        div .box {
                            h1 .title.is-3.has-text-centered {
                                span .icon.has-text-warning { i .fa-solid.fa-upload {} }
                                "Restaurer la base de données"
                            }

                            div .notification.is-warning.is-light {
                                p {
                                    strong { "Attention:" }
                                    " Cette opération va remplacer toutes les données actuelles par celles du fichier de sauvegarde."
                                }
                            }

                            form method="POST" enctype="multipart/form-data" action={(p) "/restore"} {
                                div .field {
                                    label .label { "Fichier de sauvegarde (.sql)" }
                                    div .control {
                                        div .file.has-name.is-fullwidth.is-boxed {
                                            label .file-label {
                                                input .file-input type="file" name="backup_file" accept=".sql" required
                                                    onchange="updateFileName(this)";
                                                span .file-cta {
                                                    span .file-icon {
                                                        i .fa-solid.fa-upload {}
                                                    }
                                                    span .file-label { "Choisir un fichier..." }
                                                }
                                                span .file-name #file-name { "Aucun fichier sélectionné" }
                                            }
                                        }
                                    }
                                }

                                div .field.is-grouped.is-grouped-centered.mt-5 {
                                    div .control {
                                        button .button.is-danger.is-medium type="submit" {
                                            span .icon { i .fa-solid.fa-database {} }
                                            span { "Restaurer la base de données" }
                                        }
                                    }
                                    div .control {
                                        a .button.is-light.is-medium href={(p) "/"} {
                                            span .icon { i .fa-solid.fa-xmark {} }
                                            span { "Annuler" }
                                        }
                                    }
                                }
                            }
                        }

                        div .box {
                            h2 .title.is-5 {
                                span .icon.has-text-info { i .fa-solid.fa-download {} }
                                "Créer une sauvegarde"
                            }
                            p .mb-4 { "Téléchargez une copie de la base de données actuelle avant de restaurer." }
                            a .button.is-info href={(p) "/backup"} {
                                span .icon { i .fa-solid.fa-download {} }
                                span { "Télécharger la sauvegarde" }
                            }
                        }
                    }
                }
            }
        }
    };

    let extra_scripts = html! {};

    page(
        "Restaurer la base de données - AGHIL",
        prefix,
        &NavKind::Standard,
        "",
        html! {},
        content,
        extra_scripts,
    )
}

pub fn restore_result(prefix: &str, success: bool, message: &str) -> Markup {
    let p = prefix;
    let (icon_class, title, notification_class) = if success {
        ("has-text-success", "Restauration réussie", "is-success")
    } else {
        ("has-text-danger", "Erreur de restauration", "is-danger")
    };

    let icon = if success {
        "check-circle"
    } else {
        "exclamation-triangle"
    };

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .columns.is-centered {
                    div .column.is-8 {
                        div .box.has-text-centered {
                            span class={"icon is-large " (icon_class) " mb-4"} {
                                i class={"fa-solid fa-" (icon) " fa-4x"} {}
                            }
                            h1 .title.is-3 { (title) }
                            div class={"notification " (notification_class) " is-light"} {
                                p { (message) }
                            }
                            div .buttons.is-centered.mt-5 {
                                a .button.is-primary.is-medium href={(p) "/"} {
                                    span .icon { i .fa-solid.fa-house {} }
                                    span { "Retour à l'accueil" }
                                }
                                a .button.is-info.is-medium href={(p) "/users"} {
                                    span .icon { i .fa-solid.fa-users {} }
                                    span { "Voir les adhésions" }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let page_title = format!("{title} - AGHIL");
    page(
        &page_title,
        prefix,
        &NavKind::Standard,
        "",
        html! {},
        content,
        html! {},
    )
}

pub fn import_result(success: bool, message: &str, prefix: &str) -> Markup {
    let p = prefix;
    let (title, icon, notification_class) = if success {
        ("Import réussi", "check-circle", "is-success")
    } else {
        ("Erreur d'import", "exclamation-triangle", "is-danger")
    };

    let content = html! {
        section .section {
            div .container.is-fluid {
                div class={"notification " (notification_class)} {
                    p .title.is-4 {
                        span .icon { i class={"fa-solid fa-" (icon)} {} }
                        (title)
                    }
                    p { (message) }
                }
                div .buttons.mt-4 {
                    a .button.is-primary href={(p) "/users?filter=not_imported"} {
                        span .icon { i .fa-solid.fa-arrow-left {} }
                        span { "Retour aux adhésions à importer" }
                    }
                    a .button.is-light href={(p) "/users"} {
                        span .icon { i .fa-solid.fa-list {} }
                        span { "Voir toutes les adhésions" }
                    }
                }
            }
        }
    };

    let page_title = format!("{title} - AGHIL");
    page(
        &page_title,
        prefix,
        &NavKind::Standard,
        "",
        html! {},
        content,
        html! {},
    )
}

pub fn staff_list(
    staff_with_seasons: Vec<(Staff, Option<i16>)>,
    ateliers: &[Atelier],
    roles: &[Role],
    current_season: i16,
    prefix: &str,
    show_contact: bool,
) -> Markup {
    let p = prefix;
    let staff_count = staff_with_seasons.len();

    let extra_head = html! {};

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .level.mb-4 {
                    div .level-left {
                        h1 .title.is-3 {
                            span .icon { i .fa-solid.fa-user-group {} }
                            " Liste des Staff"
                        }
                    }
                    div .level-right {
                        span .tag.is-info.is-medium { (staff_count) " membres" }
                    }
                }

                div .notification.is-info.is-light.mb-4 {
                    p {
                        span .icon { i .fa-solid.fa-circle-info {} }
                        strong { "Légende saison:" }
                        span .tag.is-success.ml-2 { "Saison courante (" (current_season) ")" }
                        span .tag.is-danger.ml-2 { "Saison précédente" }
                        span .tag.is-light.ml-2 { "Aucun paiement" }
                    }
                    p .mt-2 {
                        strong { "Légende ateliers:" }
                        span .tag.is-warning.ml-2 {
                            span .icon { i .fa-solid.fa-crown {} }
                            " Chef"
                        }
                        span .tag.is-info.ml-2 {
                            span .icon { i .fa-solid.fa-check {} }
                            " Validé"
                        }
                        span .tag.is-grey.ml-2 {
                            span .icon { i .fa-solid.fa-clock {} }
                            " En attente"
                        }
                    }
                }

                div .box {
                    div .table-container {
                        table .table.is-fullwidth.is-striped.is-hoverable {
                            thead {
                                tr {
                                    th { "Nom" }
                                    @if show_contact {
                                        th { "Email" }
                                        th { "Téléphone" }
                                    }
                                    th .has-text-centered.atelier-col {
                                        span .vertical-text { "Dernière saison" }
                                    }
                                    @for atelier in ateliers {
                                        th .has-text-centered.atelier-col {
                                            span .vertical-text { (atelier.name) }
                                        }
                                    }
                                    th .has-text-centered.atelier-col {
                                        span .vertical-text { "Admin" }
                                    }
                                    th { "Commentaire" }
                                }
                            }
                            tbody {
                                @for (staff, latest_season) in &staff_with_seasons {
                                    @let row_class = match latest_season {
                                        Some(s) if *s == current_season => "",
                                        _ => "inactive-staff",
                                    };
                                    @let (season_tag_class, season_display) = match latest_season {
                                        Some(s) if *s == current_season => ("is-success", s.to_string()),
                                        Some(s) => ("is-danger", s.to_string()),
                                        None => ("is-light", "\u{2014}".to_string()),
                                    };
                                    tr class=(row_class) {
                                        td {
                                            a href={(p) "/person/" (staff.id)} {
                                                strong { (staff.first_name) " " (staff.last_name) }
                                            }
                                        }
                                        @if show_contact {
                                            td { (staff.email) }
                                            td { (staff.phone.as_deref().unwrap_or("")) }
                                        }
                                        td {
                                            span class={"tag " (season_tag_class)} { (season_display) }
                                        }
                                        @for atelier in ateliers {
                                            @let role = roles.iter().find(|r| r.staff == staff.id && r.atelier == atelier.id);
                                            @if let Some(r) = role {
                                                @if r.chief {
                                                    td .has-text-centered.atelier-col.has-background-warning {
                                                        span .icon.has-text-black { i .fa-solid.fa-crown {} }
                                                    }
                                                } @else if r.validated {
                                                    td .has-text-centered.atelier-col.has-background-info {
                                                        span .icon.has-text-white { i .fa-solid.fa-check {} }
                                                    }
                                                } @else {
                                                    td .has-text-centered.atelier-col.has-background-grey {
                                                        span .icon.has-text-grey-dark { i .fa-solid.fa-clock {} }
                                                    }
                                                }
                                            } @else {
                                                td .has-text-centered.atelier-col {}
                                            }
                                        }
                                        // Admin column
                                        @if staff.is_god {
                                            td .has-text-centered.has-background-warning {
                                                span .icon.has-text-black { i .fa-solid.fa-crown {} }
                                            }
                                        } @else if staff.is_admin {
                                            td .has-text-centered.has-background-info {
                                                span .icon.has-text-white { i .fa-solid.fa-check {} }
                                            }
                                        } @else {
                                            td {}
                                        }
                                        td { small { (staff.comment) } }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    page(
        "Liste des Staff - AGHIL",
        prefix,
        &NavKind::Standard,
        "staff",
        extra_head,
        content,
        html! {},
    )
}

#[allow(clippy::too_many_arguments)]
pub fn person_detail(
    staff: &Staff,
    ateliers: &[Atelier],
    roles: &[Role],
    current_season: i16,
    prefix: &str,
    is_self: bool,
    is_admin: bool,
    show_contact: bool,
    todos: &[TodoItem],
    payment_history: &[crate::models::PaymentHistoryEntry],
    person_calendar: &[(crate::models::Need, String, String, String, bool, bool)],
) -> Markup {
    let p = prefix;
    let can_edit_ateliers = is_self || is_admin;
    let can_edit_contact = is_self || is_admin;

    let comment_display = if staff.comment.is_empty() {
        "\u{2014}"
    } else {
        &staff.comment
    };

    let info_text = if is_admin {
        "Cochez les ateliers auxquels ce membre participe pour la saison en cours."
    } else if is_self {
        "Cochez les ateliers auxquels vous participez pour la saison en cours."
    } else {
        "Ateliers auxquels ce membre participe pour la saison en cours."
    };

    // Build calendar data structures
    let mut days: Vec<chrono::NaiveDate> = person_calendar
        .iter()
        .map(|(n, _, _, _, _, _)| n.day)
        .collect();
    days.sort();
    days.dedup();

    let mut atelier_order: Vec<(uuid::Uuid, String, String, String)> = Vec::new();
    for (need, name, slug, icon, _, _) in person_calendar {
        if !atelier_order
            .iter()
            .any(|(id, _, _, _)| *id == need.atelier)
        {
            atelier_order.push((need.atelier, name.clone(), slug.clone(), icon.clone()));
        }
    }

    let extra_head = html! {};

    let content = html! {
        div #notification-container {}
        div .d-none #person-data data-staff-id=(staff.id) {}

        section .section {
            div .container.is-fluid {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href={(p) "/"} { "Accueil" } }
                        li { a href={(p) "/staff"} { "Staff" } }
                        li .is-active { a href="#" aria-current="page" { (staff.first_name) " " (staff.last_name) } }
                    }
                }

                // Todo box
                @if !todos.is_empty() {
                    div .box.mb-4.box-danger-accent {
                        h2 .title.is-5 {
                            span .icon.has-text-danger { i .fa-solid.fa-clipboard-list {} }
                            "\u{00a0}À faire"
                        }
                        ul .ml-2.is-unstyled {
                            @for item in todos {
                                li .mb-2 {
                                    span .{"icon has-text-" (item.color)} { i class={"fa-solid " (item.icon)} {} }
                                    (PreEscaped(&item.html))
                                }
                            }
                        }
                    }
                }

                div .columns {
                    // Left column: info + admin
                    div .column.is-one-third {
                        div .box {
                            h2 .title.is-4 {
                                span .icon { i .fa-solid.fa-user {} }
                                "\u{00a0}Informations"
                            }
                            div .content {
                                p {
                                    strong { "Nom complet:" } br;
                                    span .is-size-5 { (staff.first_name) " " (staff.last_name) }
                                }
                                // Contact section
                                @if can_edit_contact && show_contact {
                                    div .field {
                                        label .label { "Email:" }
                                        div .control.has-icons-left {
                                            input .input type="email" #edit-email value=(staff.email);
                                            span .icon.is-left { i .fa-solid.fa-envelope {} }
                                        }
                                    }
                                    div .field {
                                        label .label { "Téléphone:" }
                                        div .control.has-icons-left {
                                            input .input type="tel" #edit-phone value=(staff.phone.as_deref().unwrap_or(""));
                                            span .icon.is-left { i .fa-solid.fa-phone {} }
                                        }
                                    }
                                    div .control.mt-2 {
                                        button .button.is-small.is-info #save-contact-btn {
                                            span .icon { i .fa-solid.fa-floppy-disk {} }
                                            span { "Enregistrer" }
                                        }
                                    }
                                } @else if show_contact {
                                    p {
                                        strong { "Email:" } br;
                                        a href={"mailto:" (staff.email)} { (staff.email) }
                                    }
                                    p {
                                        strong { "Téléphone:" } br;
                                        (staff.phone.as_deref().unwrap_or("\u{2014}"))
                                    }
                                }
                                // Comment section
                                @if is_admin {
                                    div .field {
                                        label .label { "Commentaire:" }
                                        div .control {
                                            textarea .textarea #comment-input rows="3" { (comment_display) }
                                        }
                                        div .control.mt-2 {
                                            button .button.is-small.is-info #save-comment-btn {
                                                span .icon { i .fa-solid.fa-floppy-disk {} }
                                                span { "Enregistrer" }
                                            }
                                        }
                                    }
                                } @else if !staff.comment.is_empty() {
                                    p {
                                        strong { "Commentaire:" } br;
                                        (comment_display)
                                    }
                                }
                            }
                        }

                        // Admin box
                        @if is_admin {
                            div .box {
                                h2 .title.is-4 {
                                    span .icon { i .fa-solid.fa-shield-halved {} }
                                    "\u{00a0}Administration"
                                }
                                div .field.mb-3 {
                                    label .checkbox {
                                        input type="checkbox" #admin-cb checked[staff.is_admin];
                                        span .icon.has-text-info { i .fa-solid.fa-check {} }
                                        span { "Admin" }
                                    }
                                }
                                div .field {
                                    label .checkbox {
                                        input type="checkbox" #god-cb checked[staff.is_god];
                                        span .icon.has-text-warning { i .fa-solid.fa-crown {} }
                                        span { "God" }
                                    }
                                }
                            }
                        }
                    }

                    // Right column: ateliers + plannings + calendar
                    div .column {
                        div .box {
                            h2 .title.is-4 {
                                span .icon { i .fa-solid.fa-screwdriver-wrench {} }
                                "\u{00a0}Ateliers (Saison " (current_season) ")"
                            }
                            div .notification.is-info.is-light.mb-4 {
                                span .icon { i .fa-solid.fa-circle-info {} }
                                " " (info_text)
                            }
                            div .ateliers-list {
                                @for atelier in ateliers {
                                    @let role = roles.iter().find(|r| r.atelier == atelier.id);
                                    div .field.mb-4 {
                                        div .is-flex.is-align-items-center {
                                            label .checkbox.is-flex.is-align-items-center {
                                                input .atelier-checkbox.mr-2 type="checkbox"
                                                    data-atelier-id=(atelier.id)
                                                    checked[role.is_some()]
                                                    disabled[!can_edit_ateliers];
                                                span .is-size-5 { (atelier.name) }
                                                @if atelier.needs_validation {
                                                    span .tag.is-warning.is-light.ml-2 { "Validation requise" }
                                                }
                                            }
                                        }
                                        // Role options
                                        @if let Some(r) = role {
                                            @if is_admin {
                                                div .ml-5.mt-1 {
                                                    label .checkbox.mr-4 {
                                                        input .role-validated-checkbox type="checkbox"
                                                            data-atelier-id=(atelier.id)
                                                            checked[r.validated || r.chief]
                                                            disabled[r.chief];
                                                        span .icon.has-text-info { i .fa-solid.fa-check {} }
                                                        span { "Validé" }
                                                    }
                                                    label .checkbox {
                                                        input .role-chief-checkbox type="checkbox"
                                                            data-atelier-id=(atelier.id)
                                                            checked[r.chief];
                                                        span .icon.has-text-warning { i .fa-solid.fa-crown {} }
                                                        span { "Chef" }
                                                    }
                                                }
                                            } @else if r.chief {
                                                span .tag.is-warning.ml-5.mt-1 {
                                                    i .fa-solid.fa-crown.mr-1 {}
                                                    " Chef"
                                                }
                                            } @else if r.validated {
                                                span .tag.is-success.ml-5.mt-1 {
                                                    i .fa-solid.fa-check.mr-1 {}
                                                    " Validé"
                                                }
                                            } @else if atelier.needs_validation {
                                                span .tag.is-warning.is-light.ml-5.mt-1 {
                                                    i .fa-solid.fa-clock.mr-1 {}
                                                    " En attente de validation"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Plannings box
                        @let has_plannings = ateliers.iter().any(|a| roles.iter().any(|r| r.atelier == a.id) && !a.slug.is_empty());
                        @if has_plannings {
                            div .box {
                                h2 .title.is-4 {
                                    span .icon { i .fa-solid.fa-calendar-days {} }
                                    "\u{00a0}Mes plannings"
                                }
                                div .buttons {
                                    @for atelier in ateliers {
                                        @if roles.iter().any(|r| r.atelier == atelier.id) && !atelier.slug.is_empty() {
                                            a .button.is-link.is-outlined.mr-2.mb-2 href={(p) "/calendar/" (atelier.slug)} {
                                                span .icon { i class={"fa-solid fa-" (atelier.icon)} {} }
                                                "\u{00a0}"
                                                span { (atelier.name) }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Personal calendar widget
                        @if !person_calendar.is_empty() {
                            div .box {
                                h2 .title.is-4 {
                                    span .icon { i .fa-solid.fa-calendar-days {} }
                                    @if is_self {
                                        "\u{00a0}Mon calendrier"
                                    } @else {
                                        "\u{00a0}Calendrier de " (staff.first_name)
                                    }
                                }
                                div .pcal-scroll {
                                    table .pcal-table.table.is-bordered.is-narrow.is-hoverable {
                                        thead {
                                            tr {
                                                th .pcal-atelier-col { "Atelier" }
                                                @for day in &days {
                                                    @let day_abbrev = day.format("%a").to_string();
                                                    @let day_name = match day_abbrev.as_str() {
                                                        "Mon" => "lun.",
                                                        "Tue" => "mar.",
                                                        "Wed" => "mer.",
                                                        "Thu" => "jeu.",
                                                        "Fri" => "ven.",
                                                        "Sat" => "sam.",
                                                        "Sun" => "dim.",
                                                        _ => &day_abbrev,
                                                    };
                                                    @let day_date = day.format("%d/%m").to_string();
                                                    @let is_sunday = day.weekday() == chrono::Weekday::Sun;
                                                    th .pcal-day-col.has-text-centered.pcal-sunday[is_sunday] {
                                                        div .pcal-day-name { (day_name) }
                                                        div .pcal-day-date { (day_date) }
                                                    }
                                                }
                                            }
                                        }
                                        tbody {
                                            @for (atelier_id, atelier_name, atelier_slug, atelier_icon) in &atelier_order {
                                                tr {
                                                    td .pcal-atelier-col {
                                                        a href={(p) "/calendar/" (atelier_slug)} {
                                                            span .icon { i class={"fa-solid fa-" (atelier_icon)} {} }
                                                            "\u{00a0}" (atelier_name)
                                                        }
                                                    }
                                                    @for day in &days {
                                                        @let entry = person_calendar.iter().find(|(n, _, _, _, _, _)| n.atelier == *atelier_id && n.day == *day);
                                                        @if let Some((need, _, _, _, first_half, second_half)) = entry {
                                                            @let (first_label, second_label) = if need.nightly { ("soir", "nuit") } else { ("matin", "a-m") };
                                                            @let is_active = *first_half || *second_half;
                                                            @let is_sunday = day.weekday() == chrono::Weekday::Sun;
                                                            @let first_title = if need.nightly { "Soirée" } else { "Matin" };
                                                            @let second_title = if need.nightly { "Nuit" } else { "Après-midi" };
                                                            td .pcal-cell.has-text-centered.pcal-active[is_active].pcal-sunday[is_sunday] {
                                                                @if is_self {
                                                                    label .pcal-check title=(first_title) {
                                                                        input .pcal-presence-cb type="checkbox"
                                                                            data-need=(need.id)
                                                                            data-staff=(staff.id)
                                                                            data-half="first"
                                                                            checked[*first_half];
                                                                        span { (first_label) }
                                                                    }
                                                                    label .pcal-check title=(second_title) {
                                                                        input .pcal-presence-cb type="checkbox"
                                                                            data-need=(need.id)
                                                                            data-staff=(staff.id)
                                                                            data-half="second"
                                                                            checked[*second_half];
                                                                        span { (second_label) }
                                                                    }
                                                                } @else {
                                                                    span .pcal-check title=(first_title) {
                                                                        @if *first_half {
                                                                            span .icon.has-text-success { i .fa-solid.fa-check {} }
                                                                        } @else {
                                                                            span .icon.has-text-grey-lighter { i .fa-solid.fa-xmark {} }
                                                                        }
                                                                        " " span { (first_label) }
                                                                    }
                                                                    span .pcal-check title=(second_title) {
                                                                        @if *second_half {
                                                                            span .icon.has-text-success { i .fa-solid.fa-check {} }
                                                                        } @else {
                                                                            span .icon.has-text-grey-lighter { i .fa-solid.fa-xmark {} }
                                                                        }
                                                                        " " span { (second_label) }
                                                                    }
                                                                }
                                                            }
                                                        } @else {
                                                            @let is_sunday = day.weekday() == chrono::Weekday::Sun;
                                                            td .pcal-cell.has-text-centered.has-text-grey-lighter.pcal-sunday[is_sunday] { "\u{2014}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Payment history
                @if !payment_history.is_empty() {
                    div .box {
                        h2 .title.is-4 {
                            span .icon { i .fa-solid.fa-clock-rotate-left {} }
                            "\u{00a0}Historique des cotisations"
                        }
                        @for entry in payment_history {
                            @let (icon_color, icon_name) = match entry.source.as_str() {
                                "helloasso" => ("link", "fa-ticket"),
                                "check" => ("success", "fa-money-check"),
                                _ => ("warning", "fa-coins"),
                            };
                            @let date_display = entry.date.as_deref().unwrap_or("\u{2014}");
                            @let amount_display = entry.amount.map_or_else(
                                || "\u{2014}".to_string(),
                                |a| {
                                    if entry.source == "helloasso" {
                                        format!("{:.2}\u{20ac}", a as f32 / 100.0)
                                    } else {
                                        format!("{a}\u{20ac}")
                                    }
                                },
                            );
                            @let name = format!("{} {}", capitalize_words(&entry.first_name), capitalize_words(&entry.last_name));
                            @let email_display = entry.email.as_deref().unwrap_or("\u{2014}");
                            @let phone_display = entry.phone.as_deref().map_or_else(|| "\u{2014}".to_string(), format_phone_international);
                            @let source_label = match entry.source.as_str() {
                                "helloasso" => "HelloAsso",
                                "check" => "Chèque",
                                _ => "Liquide",
                            };
                            div .box.mb-3.p-3 {
                                div .columns.is-mobile.is-vcentered.is-multiline {
                                    div .column.is-narrow {
                                        span .{"icon has-text-" (icon_color)} { i class={"fa-solid " (icon_name)} {} }
                                    }
                                    div .column {
                                        strong { (entry.item_type) " " (source_label) }
                                        " \u{2014} Saison " (entry.season) br;
                                        span .is-size-7.has-text-grey { (date_display) " \u{2014} " (amount_display) }
                                    }
                                    div .column.is-5-tablet.is-12-mobile {
                                        span .is-size-7 { (name) } br;
                                        span .is-size-7 { (email_display) } br;
                                        span .is-size-7 { (phone_display) } br;
                                        @if let Some(ref payer) = entry.payer_email {
                                            @if entry.email.as_deref() != Some(payer.as_str()) {
                                                span .is-size-7.has-text-grey { "Payeur: " (payer) } br;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let extra_scripts = html! {};

    let title = format!("{} {} - AGHIL", staff.first_name, staff.last_name);
    page(
        &title,
        p,
        &NavKind::Standard,
        "",
        extra_head,
        content,
        extra_scripts,
    )
}

pub fn cash_list(cash_payments: Vec<(Cash, bool)>, current_season: i16, prefix: &str) -> Markup {
    let p = prefix;
    let total_count = cash_payments.len();
    let imported_count = cash_payments
        .iter()
        .filter(|(_, imported)| *imported)
        .count();
    let not_imported_count = total_count - imported_count;

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .level.mb-4 {
                    div .level-left {
                        h1 .title.is-3 {
                            span .icon { i .fa-solid.fa-money-bill-wave {} }
                            " Paiements espèces / chèques"
                        }
                    }
                    div .level-right {
                        a .button.is-primary href={(p) "/cash?form=1"} {
                            span .icon { i .fa-solid.fa-plus {} }
                            span { "Nouveau paiement" }
                        }
                    }
                }

                div .tags.mb-4 {
                    span .tag.is-medium { "Total: " (total_count) }
                    span .tag.is-success.is-medium { "Importés: " (imported_count) }
                    span .tag.is-warning.is-medium { "À importer: " (not_imported_count) }
                    span .tag.is-info.is-medium { "Saison: " (current_season) }
                }

                div .box {
                    table .table.is-fullwidth.is-striped.is-hoverable {
                        thead {
                            tr {
                                th { "Nom" }
                                th { "Email" }
                                th { "Téléphone" }
                                th { "Moyen" }
                                th { "Type" }
                                th .has-text-right { "Montant" }
                                th { "Date" }
                                th { "Saison" }
                                th { "Statut" }
                            }
                        }
                        tbody {
                            @for (cash, has_staff) in &cash_payments {
                                @let full_name = format!("{} {}", capitalize_words(&cash.first_name), capitalize_words(&cash.last_name));
                                @let email = cash.email.as_deref().unwrap_or("\u{2014}");
                                @let phone = cash.phone.as_deref().map_or_else(|| "\u{2014}".to_string(), format_phone_international);
                                @let date = cash.date.format("%d/%m/%Y").to_string();
                                @let season: i16 = if cash.date.month() >= 6 { cash.date.year() as i16 + 1 } else { cash.date.year() as i16 };
                                @let amount = format!("{}€", cash.amount);
                                @let (type_label, type_class) = if cash.is_membership { ("Adhésion", "is-primary") } else { ("Autre", "is-info") };
                                @let (method_label, method_icon) = if cash.payment_method == "check" { ("Chèque", "fa-money-check") } else { ("Espèces", "fa-coins") };
                                @let season_tag_class = if season == current_season { "is-primary" } else { "is-light" };
                                tr {
                                    td { strong { (full_name) } }
                                    td { (email) }
                                    td { (phone) }
                                    td {
                                        span .icon-text {
                                            span .icon { i class={"fa-solid " (method_icon)} {} }
                                            span { (method_label) }
                                        }
                                    }
                                    td { span class={"tag " (type_class)} { (type_label) } }
                                    td .has-text-right { strong .has-text-success { (amount) } }
                                    td { (date) }
                                    td { span class={"tag " (season_tag_class)} { (season) } }
                                    td {
                                        @if *has_staff {
                                            span .tag.is-success { "Importé" }
                                        } @else {
                                            a .tag.is-warning href={(p) "/cash-import/" (cash.id)} { "À importer" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    page(
        "Paiements espèces / chèques - AGHIL",
        prefix,
        &NavKind::Standard,
        "cash",
        html! {},
        content,
        html! {},
    )
}

pub fn cash_form(prefix: &str) -> Markup {
    let p = prefix;

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .columns.is-centered {
                    div .column.is-6 {
                        div .level.mb-5 {
                            div .level-left {
                                h1 .title.is-3 { "Nouveau paiement" }
                            }
                            div .level-right {
                                a .button.is-light href={(p) "/cash"} {
                                    span .icon { i .fa-solid.fa-arrow-left {} }
                                    span { "Retour" }
                                }
                            }
                        }

                        div .box {
                            form method="POST" action={(p) "/cash"} {
                                div .columns {
                                    div .column {
                                        div .field {
                                            label .label { "Prénom *" }
                                            div .control {
                                                input .input type="text" name="first_name" required;
                                            }
                                        }
                                    }
                                    div .column {
                                        div .field {
                                            label .label { "Nom *" }
                                            div .control {
                                                input .input type="text" name="last_name" required;
                                            }
                                        }
                                    }
                                }

                                div .field {
                                    label .label { "Email" }
                                    div .control {
                                        input .input type="email" name="email";
                                    }
                                }

                                div .field {
                                    label .label { "Téléphone" }
                                    div .control {
                                        input .input type="tel" name="phone";
                                    }
                                }

                                div .columns {
                                    div .column {
                                        div .field {
                                            label .label { "Date *" }
                                            div .control {
                                                input .input type="date" name="date" required;
                                            }
                                        }
                                    }
                                    div .column {
                                        div .field {
                                            label .label { "Montant (euros) *" }
                                            div .control {
                                                input .input type="number" name="amount" min="1" required;
                                            }
                                        }
                                    }
                                }

                                div .field {
                                    label .label { "Moyen de paiement *" }
                                    div .control {
                                        div .select.is-fullwidth {
                                            select name="payment_method" required {
                                                option value="cash" { "Espèces" }
                                                option value="check" { "Chèque" }
                                            }
                                        }
                                    }
                                }

                                div .field {
                                    label .checkbox {
                                        input type="checkbox" name="is_membership" value="true" checked;
                                        " Adhésion (cotisation)"
                                    }
                                    p .help { "Décochez si ce n'est pas une cotisation (ex: don, participation aux frais...)" }
                                }

                                div .field.mt-5 {
                                    div .control {
                                        button .button.is-primary.is-fullwidth type="submit" {
                                            span .icon { i .fa-solid.fa-floppy-disk {} }
                                            span { "Enregistrer le paiement" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    page(
        "Nouveau paiement - AGHIL",
        prefix,
        &NavKind::Standard,
        "cash",
        html! {},
        content,
        html! {},
    )
}

pub fn cash_import_form(
    cash: &Cash,
    season: i16,
    candidates: Vec<StaffWithSeason>,
    prefix: &str,
) -> Markup {
    let beneficiary_first = capitalize_words(&cash.first_name);
    let beneficiary_last = capitalize_words(&cash.last_name);
    let cash_email = cash.email.as_deref().unwrap_or("").to_lowercase();
    let default_email = &cash_email;
    let phone = cash
        .phone
        .as_deref()
        .map(format_phone_international)
        .unwrap_or_default();
    let amount = format!("{}\u{20ac}", cash.amount);
    let date = cash.date.format("%d/%m/%Y").to_string();
    let type_label = if cash.is_membership {
        "Adhésion"
    } else {
        "Autre"
    };
    let method_label = if cash.payment_method == "check" {
        "Chèque"
    } else {
        "Espèces"
    };

    let has_exact_match = candidates.iter().any(|c| {
        matches!(
            c.match_type,
            StaffMatchType::ExactBoth | StaffMatchType::ExactEmail | StaffMatchType::ExactName
        )
    });
    let recommend_create = !has_exact_match;

    let mut is_first = !recommend_create;
    let mut option_index = 0usize;

    let candidates_markup = html! {
        @for candidate in &candidates {
            @let staff = &candidate.staff;
            @let match_label = match candidate.match_type {
                StaffMatchType::ExactBoth => "Email et nom identiques",
                StaffMatchType::ExactName => "Nom identique",
                StaffMatchType::ExactEmail => "Email identique",
                StaffMatchType::PayerEmailMatch => "Email payeur identique",
                StaffMatchType::SimilarEmail => "Email similaire",
                StaffMatchType::SimilarName => "Nom similaire",
                StaffMatchType::DoubleSubscription => "Double adhésion probable",
            };
            @let season_info = candidate.latest_season.map_or_else(
                || "Aucune saison".to_string(),
                |s| format!("Dernière saison: {}", s),
            );
            @let is_exact_match = matches!(
                candidate.match_type,
                StaffMatchType::ExactBoth | StaffMatchType::ExactEmail | StaffMatchType::ExactName
            );
            @let (highlight, recommended_tag, border_color) = if candidate.match_type == StaffMatchType::DoubleSubscription {
                ("is-danger", Some(("is-danger", "Double adhésion")), "var(--bulma-danger)")
            } else if is_first && is_exact_match {
                ("is-primary", Some(("is-success", "Probable meilleure option")), "var(--bulma-primary)")
            } else if is_exact_match {
                ("is-info", Some(("is-warning", "Option envisageable")), "var(--bulma-info)")
            } else {
                ("is-light", None, "var(--bulma-border)")
            };
            @let names_match = beneficiary_first.to_lowercase() == staff.first_name.to_lowercase()
                && beneficiary_last.to_lowercase() == staff.last_name.to_lowercase();
            @let staff_email_lower = staff.email.to_lowercase();
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
                        input type="hidden" name="first_name" value=(beneficiary_first);
                        input type="hidden" name="last_name" value=(beneficiary_last);
                    } @else {
                        div .field {
                            label .label { "Garder le prénom et nom" }
                            div .control {
                                label .radio {
                                    input type="radio" name="name_choice" value="cash" checked
                                        onchange=(format!("updateNameFields(this.form, '{}', '{}')", beneficiary_first, beneficiary_last));
                                    " Du paiement: " strong { (beneficiary_first) " " (beneficiary_last) }
                                }
                                br;
                                label .radio {
                                    input type="radio" name="name_choice" value="staff"
                                        onchange=(format!("updateNameFields(this.form, '{}', '{}')", staff.first_name, staff.last_name));
                                    " Du staff: " strong { (staff.first_name) " " (staff.last_name) }
                                }
                            }
                        }
                        input type="hidden" name="first_name" value=(beneficiary_first);
                        input type="hidden" name="last_name" value=(beneficiary_last);
                    }

                    // Email choice
                    @if cash_email.is_empty() || cash_email == staff_email_lower {
                        @let email_value = if cash_email.is_empty() { &staff.email } else { &cash_email };
                        input type="hidden" name="email" value=(email_value);
                    } @else {
                        div .field {
                            label .label { "Garder l'email" }
                            div .control {
                                label .radio {
                                    input type="radio" name="email_choice" value="cash" checked
                                        onchange=(format!("updateEmailField(this.form, '{}')", cash_email));
                                    " Du paiement: " strong { (cash_email) }
                                }
                                br;
                                label .radio {
                                    input type="radio" name="email_choice" value="staff"
                                        onchange=(format!("updateEmailField(this.form, '{}')", staff.email));
                                    " Du staff: " strong { (staff.email) }
                                }
                            }
                        }
                        input type="hidden" name="email" value=(cash_email);
                    }

                    input type="hidden" name="phone" value=(phone);

                    div .field {
                        label .label { "Commentaire" }
                        div .control {
                            textarea .textarea name="comment" rows="2" {}
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
                option_index += 1;
            };
        }
    };

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
                                input .input type="text" name="first_name" value=(beneficiary_first);
                            }
                        }
                    }
                    div .column {
                        div .field {
                            label .label { "Nom" }
                            div .control {
                                input .input type="text" name="last_name" value=(beneficiary_last);
                            }
                        }
                    }
                }

                div .field {
                    label .label { "Email" }
                    div .control {
                        input .input type="email" name="email" value=(default_email);
                    }
                }

                div .field {
                    label .label { "Téléphone" }
                    div .control {
                        input .input type="tel" name="phone" value=(phone);
                    }
                }

                div .field {
                    label .label { "Commentaire" }
                    div .control {
                        textarea .textarea name="comment" rows="2" {}
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

    let options_markup = if recommend_create {
        html! { (create_markup) (candidates_markup) }
    } else {
        html! { (candidates_markup) (create_markup) }
    };

    let total_options = candidates.len() + 1;
    let email_display = if cash_email.is_empty() {
        "N/A"
    } else {
        &cash_email
    };

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .level.mb-5 {
                    div .level-left {
                        h1 .title.is-3 { "Importer un paiement" }
                    }
                    div .level-right {
                        a href=(format!("{prefix}/cash")) .button.is-light {
                            span .icon { i .fa-solid.fa-arrow-left {} }
                            span { "Retour" }
                        }
                    }
                }

                div .columns {
                    div .column.is-5 {
                        div .box {
                            h2 .title.is-4.mb-4 { "Détails du paiement" }
                            div .content {
                                table .table.is-fullwidth {
                                    tbody {
                                        tr { th { "Nom" } td { strong { (beneficiary_first) " " (beneficiary_last) } } }
                                        tr { th { "Email" } td { (email_display) } }
                                        tr { th { "Téléphone" } td { (phone) } }
                                        tr { th { "Moyen" } td { (method_label) } }
                                        tr { th { "Type" } td { (type_label) } }
                                        tr { th { "Montant" } td { (amount) } }
                                        tr { th { "Date" } td { (date) } }
                                        tr { th { "Saison" } td { span .tag.is-info.is-medium { (season) } } }
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
        "Importer paiement - AGHIL",
        prefix,
        &NavKind::Standard,
        "cash",
        html! {},
        content,
        html! {},
    )
}

#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]
pub fn calendar(
    atelier: &Atelier,
    needs: &[Need],
    staff_list: &[Staff],
    presence: &HashMap<(uuid::Uuid, uuid::Uuid), (bool, bool)>,
    all_ateliers: &[Atelier],
    prefix: &str,
    viewer_id: Option<uuid::Uuid>,
    _is_admin: bool,
    opening_days: &[crate::models::OpeningDay],
) -> Markup {
    let p = prefix;

    // Precompute which needs are complete (both halves individually >= quantity)
    let complete_needs: std::collections::HashSet<uuid::Uuid> = needs
        .iter()
        .filter(|need| {
            let filled_first: i16 = staff_list
                .iter()
                .filter(|s| presence.get(&(need.id, s.id)).is_some_and(|(f, _)| *f))
                .count() as i16;
            let filled_second: i16 = staff_list
                .iter()
                .filter(|s| presence.get(&(need.id, s.id)).is_some_and(|(_, s)| *s))
                .count() as i16;
            filled_first >= need.quantity && filled_second >= need.quantity
        })
        .map(|n| n.id)
        .collect();

    // Build a lookup map for opening days
    let opening_map: std::collections::HashMap<chrono::NaiveDate, &crate::models::OpeningDay> =
        opening_days.iter().map(|od| (od.day, od)).collect();

    let content = html! {
        div #notification-container {}

        section .section.pt-4.pb-4 {
            div .container.is-fluid {
                h1 .title.is-4.mb-3 {
                    span .icon { i .fa-solid.fa-calendar-days {} }
                    " Planning \u{2014} " (atelier.name)
                }

                div .atelier-nav {
                    @for a in all_ateliers {
                        a .navbar-item.is-active[a.id == atelier.id] href={(p) "/calendar/" (a.slug)} {
                            span .icon { i class={"fa-solid fa-" (a.icon)} {} }
                            "\u{00a0}" (a.name)
                        }
                    }
                }

                div .cal-scroll {
                    table .cal-table.table.is-bordered.is-narrow.is-hoverable {
                        thead {
                            // Header row with day columns
                            tr {
                                th .cal-name-col { "Nom" }
                                @for need in needs {
                                    @let day_abbrev = need.day.format("%a").to_string();
                                    @let day_name = match day_abbrev.as_str() {
                                        "Mon" => "lun.",
                                        "Tue" => "mar.",
                                        "Wed" => "mer.",
                                        "Thu" => "jeu.",
                                        "Fri" => "ven.",
                                        "Sat" => "sam.",
                                        "Sun" => "dim.",
                                        _ => &day_abbrev,
                                    };
                                    @let day_date = need.day.format("%d/%m").to_string();
                                    @let is_sunday = need.day.weekday() == chrono::Weekday::Sun;
                                    @let filled_first: i16 = staff_list.iter().filter(|s| presence.get(&(need.id, s.id)).is_some_and(|(f, _)| *f)).count() as i16;
                                    @let filled_second: i16 = staff_list.iter().filter(|s| presence.get(&(need.id, s.id)).is_some_and(|(_, se)| *se)).count() as i16;
                                    @let both_complete = filled_first >= need.quantity && filled_second >= need.quantity;
                                    @let (first_label_h, second_label_h) = if need.nightly { ("soir", "nuit") } else { ("matin", "après-midi") };
                                    th .cal-day-col.has-text-centered.cal-sunday[is_sunday].cal-complete[both_complete].cal-danger[!both_complete] {
                                        div .cal-day-name { (day_name) }
                                        div .cal-day-date { (day_date) }
                                        div .cal-day-count {
                                            span .has-text-success[filled_first >= need.quantity].has-text-danger[filled_first < need.quantity] {
                                                (first_label_h) " " (filled_first) "/" (need.quantity)
                                            }
                                            " "
                                            span .has-text-success[filled_second >= need.quantity].has-text-danger[filled_second < need.quantity] {
                                                (second_label_h) " " (filled_second) "/" (need.quantity)
                                            }
                                        }
                                    }
                                }
                            }
                            // Opening day status row
                            tr .cal-opening-row {
                                td .cal-name-col { strong { "Ouverture" } }
                                @for need in needs {
                                    @let is_sunday = need.day.weekday() == chrono::Weekday::Sun;
                                    td .has-text-centered.cal-sunday[is_sunday] {
                                        @if let Some(od) = opening_map.get(&need.day) {
                                            @match od.status {
                                                crate::models::OpeningDayStatus::Reserved => {
                                                    span .tag.is-info { "Prévu" }
                                                },
                                                crate::models::OpeningDayStatus::Validated => {
                                                    span .tag.is-success { "Confirmé" }
                                                },
                                                crate::models::OpeningDayStatus::Canceled => {
                                                    span .tag.is-danger { "Annulé" }
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        tbody {
                            @for staff in staff_list {
                                @let can_toggle = viewer_id.is_some_and(|vid| staff.id == vid);
                                @let name = format!("{} {}", capitalize_words(&staff.first_name), capitalize_words(&staff.last_name));
                                tr .cal-me[can_toggle] {
                                    td .cal-name-col {
                                        a href={(p) "/person/" (staff.id)} { (name) }
                                    }
                                    @for need in needs {
                                        @let (first_half, second_half) = presence.get(&(need.id, staff.id)).copied().unwrap_or((false, false));
                                        @let (first_label, second_label) = if need.nightly { ("soir", "nuit") } else { ("matin", "après-midi") };
                                        @let is_active = first_half || second_half;
                                        @let is_sunday = need.day.weekday() == chrono::Weekday::Sun;
                                        @let is_complete = complete_needs.contains(&need.id);
                                        td .cal-cell.has-text-centered.cal-active[is_active].cal-sunday[is_sunday].cal-complete[is_complete].cal-danger[!is_complete] {
                                            label .cal-check title=(if need.nightly { "Soirée" } else { "Matin" }) {
                                                input .presence-cb type="checkbox"
                                                    data-need=(need.id)
                                                    data-staff=(staff.id)
                                                    data-half="first"
                                                    checked[first_half]
                                                    disabled[!can_toggle];
                                                span { (first_label) }
                                            }
                                            label .cal-check title=(if need.nightly { "Nuit" } else { "Après-midi" }) {
                                                input .presence-cb type="checkbox"
                                                    data-need=(need.id)
                                                    data-staff=(staff.id)
                                                    data-half="second"
                                                    checked[second_half]
                                                    disabled[!can_toggle];
                                                span { (second_label) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Empty messages
                @if needs.is_empty() {
                    div .notification.is-warning.is-light.mt-4 {
                        span .icon { i .fa-solid.fa-triangle-exclamation {} }
                        " Aucun besoin déclaré pour cet atelier."
                    }
                } @else if staff_list.is_empty() {
                    div .notification.is-info.is-light.mt-4 {
                        span .icon { i .fa-solid.fa-circle-info {} }
                        " Aucun bénévole assigné à cet atelier."
                    }
                }
            }
        }
    };

    let title = format!("Planning {} - AGHIL", atelier.name);
    page(
        &title,
        p,
        &NavKind::Standard,
        "",
        html! {},
        content,
        html! {},
    )
}

/// Render the "Semaine à venir" (upcoming week needs) HTML snippet.
/// Used on both the index page and the calendar editor page.
fn render_upcoming_week(upcoming: &[(chrono::NaiveDate, String, i16, i64)]) -> Markup {
    if upcoming.is_empty() {
        return html! {
            p .has-text-grey-light { "Aucun besoin déclaré pour les 7 prochains jours." }
        };
    }

    // Group entries by day
    let mut days: Vec<(chrono::NaiveDate, Vec<(String, i64)>)> = Vec::new();
    for (day, atelier_name, quantity, filled) in upcoming {
        let missing = (i64::from(*quantity) - filled).max(0);
        if days.last().is_none_or(|(d, _)| d != day) {
            days.push((*day, Vec::new()));
        }
        if let Some(last) = days.last_mut() {
            last.1.push((atelier_name.clone(), missing));
        }
    }

    let month_names = [
        "",
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ];

    html! {
        @for (day, deficits) in &days {
            @let day_abbrev = day.format("%a").to_string();
            @let day_name = match day_abbrev.as_str() {
                "Mon" => "Lundi",
                "Tue" => "Mardi",
                "Wed" => "Mercredi",
                "Thu" => "Jeudi",
                "Fri" => "Vendredi",
                "Sat" => "Samedi",
                "Sun" => "Dimanche",
                _ => &day_abbrev,
            };
            @let month_name = month_names[day.month() as usize];
            @let date_str = format!("{} {} {}", day_name, day.day(), month_name);
            @let missing_parts: Vec<String> = deficits.iter()
                .filter(|(_, m)| *m > 0)
                .map(|(name, m)| format!("{} {}", m, name.to_lowercase()))
                .collect();
            @if missing_parts.is_empty() {
                div .week-day.week-day-ok {
                    span .icon.has-text-success { i .fa-solid.fa-circle-check {} }
                    " "
                    strong { (date_str) }
                    " \u{2014} complet"
                }
            } @else {
                div .week-day.week-day-missing {
                    span .icon.has-text-danger { i .fa-solid.fa-circle-exclamation {} }
                    " "
                    strong { (date_str) }
                    " \u{2014} il manque " (missing_parts.join(", "))
                }
            }
        }
    }
}

/// Email-friendly variant of `render_upcoming_week`: plain HTML without Bulma/FontAwesome,
/// using Unicode symbols instead.
pub fn render_upcoming_week_email(upcoming: &[(chrono::NaiveDate, String, i16, i64)]) -> String {
    let mut html = String::new();
    if upcoming.is_empty() {
        html.push_str("<p>Aucun besoin déclaré pour les 7 prochains jours.</p>");
        return html;
    }

    let mut current_day: Option<chrono::NaiveDate> = None;
    let mut day_deficits: Vec<(String, i64)> = Vec::new();

    let flush_day = |day: chrono::NaiveDate, deficits: &[(String, i64)], out: &mut String| {
        let day_abbrev = day.format("%a").to_string();
        let day_name = match day_abbrev.as_str() {
            "Mon" => "Lundi",
            "Tue" => "Mardi",
            "Wed" => "Mercredi",
            "Thu" => "Jeudi",
            "Fri" => "Vendredi",
            "Sat" => "Samedi",
            "Sun" => "Dimanche",
            _ => &day_abbrev,
        };
        let month_names = [
            "",
            "janvier",
            "février",
            "mars",
            "avril",
            "mai",
            "juin",
            "juillet",
            "août",
            "septembre",
            "octobre",
            "novembre",
            "décembre",
        ];
        let month_name = month_names[day.month() as usize];
        let date_str = format!("{} {} {}", day_name, day.day(), month_name);

        let missing_parts: Vec<String> = deficits
            .iter()
            .filter(|(_, missing)| *missing > 0)
            .map(|(name, missing)| format!("{} {}", missing, name.to_lowercase()))
            .collect();

        if missing_parts.is_empty() {
            out.push_str(&format!(
                "<p>\u{2713} <strong>{}</strong> — complet</p>\n",
                date_str
            ));
        } else {
            out.push_str(&format!(
                "<p>\u{26A0} <strong>{}</strong> — il manque {}</p>\n",
                date_str,
                missing_parts.join(", "),
            ));
        }
    };

    for (day, atelier_name, quantity, filled) in upcoming {
        let missing = i64::from(*quantity) - filled;
        if current_day != Some(*day) {
            if let Some(prev_day) = current_day {
                flush_day(prev_day, &day_deficits, &mut html);
            }
            current_day = Some(*day);
            day_deficits.clear();
        }
        day_deficits.push((atelier_name.clone(), missing.max(0)));
    }
    if let Some(prev_day) = current_day {
        flush_day(prev_day, &day_deficits, &mut html);
    }

    html
}

pub fn calendar_editor(
    all_ateliers: &[Atelier],
    editable_ids: &[uuid::Uuid],
    future_needs: &[(Need, i64, i64)],
    prefix: &str,
    logged_in: bool,
    is_admin: bool,
    opening_days: &[crate::models::OpeningDay],
) -> Markup {
    use std::collections::{BTreeMap, BTreeSet};

    let p = prefix;

    // Collect unique sorted days
    let days: Vec<chrono::NaiveDate> = {
        let mut s = BTreeSet::new();
        for (n, _, _) in future_needs {
            s.insert(n.day);
        }
        s.into_iter().collect()
    };

    // For each day, determine (has_day_need, has_night_need)
    let mut day_types: BTreeMap<chrono::NaiveDate, (bool, bool)> = BTreeMap::new();
    for (n, _, _) in future_needs {
        let entry = day_types.entry(n.day).or_insert((false, false));
        if n.nightly {
            entry.1 = true;
        } else {
            entry.0 = true;
        }
    }

    // Build needs_map: (atelier_id, day) -> (&Need, h1_count, h2_count)
    let mut needs_map: HashMap<(uuid::Uuid, chrono::NaiveDate), (&Need, i64, i64)> = HashMap::new();
    for (n, h1, h2) in future_needs {
        needs_map.insert((n.atelier, n.day), (n, *h1, *h2));
    }

    // French day-of-week abbreviations
    let day_abbrev = |d: chrono::NaiveDate| -> &'static str {
        match d.weekday() {
            chrono::Weekday::Mon => "lun.",
            chrono::Weekday::Tue => "mar.",
            chrono::Weekday::Wed => "mer.",
            chrono::Weekday::Thu => "jeu.",
            chrono::Weekday::Fri => "ven.",
            chrono::Weekday::Sat => "sam.",
            chrono::Weekday::Sun => "dim.",
        }
    };

    // Sub-column count for a day
    let subcols = |d: &chrono::NaiveDate| -> usize {
        let (has_day, has_night) = day_types.get(d).copied().unwrap_or((false, false));
        if has_day && has_night { 4 } else { 2 }
    };

    // Opening day map
    let opening_map: HashMap<chrono::NaiveDate, &crate::models::OpeningDay> =
        opening_days.iter().map(|od| (od.day, od)).collect();

    // Build editable atelier IDs as JSON array for JS
    let editable_json: String = format!(
        "[{}]",
        editable_ids
            .iter()
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(",")
    );

    // Build atelier cards data as JSON for JS (used in the modal)
    let ateliers_json: String = format!(
        "[{}]",
        all_ateliers
            .iter()
            .map(|a| format!(
                "{{\"id\":\"{}\",\"name\":\"{}\",\"slug\":\"{}\",\"icon\":\"{}\",\"default_nightly\":{}}}",
                a.id, a.name, a.slug, a.icon, a.default_nightly
            ))
            .collect::<Vec<_>>()
            .join(",")
    );

    let extra_head = html! {
        link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bulma-calendar-js@7.1.2/dist/css/bulma-calendar.min.css";
    };

    let content = html! {
        div #notification-container {}
        script #ateliers-data type="application/json" { (PreEscaped(&ateliers_json)) }
        script #editable-data type="application/json" { (PreEscaped(&editable_json)) }

        section .section.pt-4.pb-4 {
            div .container.is-fluid {
                h1 .title.is-4.mb-3 {
                    span .icon { i .fa-solid.fa-calendar-days {} }
                    " Planning des besoins"
                }

                // Calendar links section
                @if logged_in {
                    div .calendar-links {
                        span .has-text-grey.mr-1.cal-label { "Plannings :" }
                        @for a in all_ateliers {
                            a .tag.is-medium.is-link.is-light href={(p) "/calendar/" (a.slug)} {
                                span .icon { i class={"fa-solid fa-" (a.icon)} {} }
                                "\u{00a0}" (a.name)
                            }
                        }
                    }
                }

                // Add buttons section
                @if !editable_ids.is_empty() {
                    div .mb-4.buttons {
                        @if is_admin {
                            button .button.is-info #open-add-opening-day-modal {
                                span .icon { i .fa-solid.fa-sun {} }
                                span { "Ajouter un jour d'ouverture" }
                            }
                        }
                        button .button.is-primary #open-add-modal {
                            span .icon { i .fa-solid.fa-pen-to-square {} }
                            span { "Modifier des besoins en bénévoles" }
                        }
                    }
                }

                // Main table
                div .cal-scroll {
                    table .cal-table.table.is-bordered.is-narrow.is-hoverable {
                        thead {
                            // Header row 1: Atelier + date columns
                            tr {
                                th .cal-name-col rowspan="2" { "Atelier" }
                                @for d in &days {
                                    th .day-start colspan=(subcols(d)) {
                                        (day_abbrev(*d)) " " (format!("{:02}", d.day())) "/" (format!("{:02}", d.month()))
                                    }
                                }
                            }
                            // Header row 2: sub-column labels
                            tr {
                                @for d in &days {
                                    @let (has_day, has_night) = day_types.get(d).copied().unwrap_or((false, false));
                                    @if has_day && has_night {
                                        th .day-start { "matin" }
                                        th { "a-m" }
                                        th { "soir" }
                                        th { "nuit" }
                                    } @else if has_night {
                                        th .day-start { "soir" }
                                        th { "nuit" }
                                    } @else {
                                        th .day-start { "matin" }
                                        th { "a-m" }
                                    }
                                }
                            }
                            // Opening day row
                            tr .cal-opening-row {
                                td .cal-name-col { strong { "Ouverture" } }
                                @for d in &days {
                                    @let n_sub = subcols(d);
                                    td .day-start.has-text-centered colspan=(n_sub) {
                                        @if let Some(od) = opening_map.get(d) {
                                            @let (tag_class, tag_label) = match od.status {
                                                crate::models::OpeningDayStatus::Reserved => ("is-info", "Prévu"),
                                                crate::models::OpeningDayStatus::Validated => ("is-success", "Confirmé"),
                                                crate::models::OpeningDayStatus::Canceled => ("is-danger", "Annulé"),
                                            };
                                            @let day_str = d.format("%Y-%m-%d").to_string();
                                            @if is_admin && od.status == crate::models::OpeningDayStatus::Reserved {
                                                span class={"tag " (tag_class) " opening-tag is-clickable"} data-day=(day_str) { (tag_label) }
                                            } @else {
                                                span class={"tag " (tag_class)} { (tag_label) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        tbody {
                            @if days.is_empty() {
                                tr {
                                    td .cal-name-col colspan="100%" {
                                        em { "Aucun besoin à venir. Utilisez le bouton ci-dessus pour en créer." }
                                    }
                                }
                            }
                            @for atelier in all_ateliers {
                                tr {
                                    td .cal-name-col { (atelier.name) }
                                    @for d in &days {
                                        @let (has_day, has_night) = day_types.get(d).copied().unwrap_or((false, false));
                                        @let mixed = has_day && has_night;
                                        @let n_subcols = if mixed { 4_usize } else { 2_usize };
                                        @let day_str = d.format("%Y-%m-%d").to_string();
                                        @let entry = needs_map.get(&(atelier.id, *d));
                                        @match entry {
                                            None => {
                                                @for idx in 0..n_subcols {
                                                    @let cls = if idx == 0 { "day-cell day-start" } else { "day-cell" };
                                                    td class=(cls) data-day=(&day_str) {}
                                                }
                                            },
                                            Some((need, h1, h2)) => {
                                                @let qty = i64::from(need.quantity);
                                                @let pad_before = if mixed && need.nightly { 2_usize } else { 0_usize };
                                                @let pad_after = if mixed && !need.nightly { 2_usize } else { 0_usize };
                                                // Padding cells before (for nightly needs in mixed days)
                                                @for idx in 0..pad_before {
                                                    @let cls = if idx == 0 { "day-cell day-start" } else { "day-cell" };
                                                    td class=(cls) data-day=(&day_str) {}
                                                }
                                                // First half cell
                                                @let style_h1 = if *h1 >= qty { "cell-ok" } else { "cell-deficit" };
                                                @let cls_h1 = if pad_before == 0 {
                                                    format!("day-cell has-text-centered {style_h1} day-start")
                                                } else {
                                                    format!("day-cell has-text-centered {style_h1}")
                                                };
                                                td class=(&cls_h1) data-day=(&day_str) { (h1) "/" (qty) }
                                                // Second half cell
                                                @let style_h2 = if *h2 >= qty { "cell-ok" } else { "cell-deficit" };
                                                @let cls_h2 = format!("day-cell has-text-centered {style_h2}");
                                                td class=(&cls_h2) data-day=(&day_str) { (h2) "/" (qty) }
                                                // Padding cells after (for day needs in mixed days)
                                                @for _idx in 0..pad_after {
                                                    td .day-cell data-day=(&day_str) {}
                                                }
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Modal: day editor (opened by clicking a cell)
        div .modal #day-modal {
            div .modal-background {}
            div .modal-card.modal-card-wide {
                header .modal-card-head {
                    p .modal-card-title #day-modal-title { "\u{2014}" }
                    button .delete aria-label="close" #close-day-modal {}
                }
                section .modal-card-body {
                    div .atelier-cards #day-atelier-cards {}
                }
            }
        }

        // Modal: add needs via calendar picker
        div .modal #add-modal {
            div .modal-background {}
            div .modal-card.modal-card-wide {
                header .modal-card-head {
                    p .modal-card-title { "Modifier des besoins en bénévoles" }
                    button .delete aria-label="close" #close-add-modal {}
                }
                section .modal-card-body {
                    div .editor-columns {
                        div .editor-left {
                            input type="date" #calendar-widget;
                        }
                        div .editor-right {
                            div #add-edit-panel .d-none {
                                h2 .subtitle.is-5.mb-3 #add-panel-title { "\u{2014}" }
                                div .atelier-cards #add-atelier-cards {}
                            }
                            div .notification.is-info.is-light #add-no-selection {
                                span .icon { i .fa-solid.fa-hand-pointer {} }
                                " Sélectionnez une date sur le calendrier."
                            }
                        }
                    }
                }
            }
        }

        // Modal: add opening day via calendar picker
        div .modal #opening-day-modal {
            div .modal-background {}
            div .modal-card.modal-card-medium {
                header .modal-card-head {
                    p .modal-card-title { "Ajouter un jour d'ouverture" }
                    button .delete aria-label="close" #close-opening-day-modal {}
                }
                section .modal-card-body {
                    div .has-text-centered {
                        input type="date" #opening-day-picker;
                    }
                    div .mt-4.has-text-centered.d-none #opening-day-confirm {
                        p .mb-3 #opening-day-confirm-text {}
                        button .button.is-info #opening-day-submit {
                            span .icon { i .fa-solid.fa-check {} }
                            span { "Créer le jour d'ouverture" }
                        }
                    }
                }
            }
        }

        // Modal: Go / NoGo for a reserved opening day
        div .modal #gonogo-modal {
            div .modal-background {}
            div .modal-card.modal-card-small {
                header .modal-card-head {
                    p .modal-card-title #gonogo-title { "\u{2014}" }
                    button .delete aria-label="close" #close-gonogo-modal {}
                }
                section .modal-card-body.has-text-centered {
                    p .mb-4 { "Que souhaitez-vous faire pour cette journée ?" }
                    div .buttons.is-centered {
                        button .button.is-success.is-medium #gonogo-go {
                            span .icon { i .fa-solid.fa-circle-check {} }
                            span { "Go" }
                        }
                        button .button.is-danger.is-medium #gonogo-nogo {
                            span .icon { i .fa-solid.fa-circle-xmark {} }
                            span { "NO Go" }
                        }
                        button .button.is-medium #gonogo-cancel {
                            span { "Ne rien faire" }
                        }
                    }
                }
            }
        }
    };

    let extra_scripts = html! {
        script src="https://cdn.jsdelivr.net/npm/bulma-calendar-js@7.1.2/dist/js/bulma-calendar.min.js" {}
    };

    page(
        "Gestion des besoins - AGHIL",
        prefix,
        &NavKind::Standard,
        "",
        extra_head,
        content,
        extra_scripts,
    )
}

pub fn login_page(prefix: &str) -> Markup {
    let content = html! {
        section .section {
            div .container {
                div .columns.is-centered {
                    div .column.is-5 {
                        div .card {
                            div .card-content {
                                h2 .title.is-4.has-text-centered {
                                    span .icon { i .fa-solid.fa-right-to-bracket {} }
                                    "Connexion"
                                }
                                div .field {
                                    label .label { "Rechercher votre nom" }
                                    div .control.has-icons-left {
                                        input .input type="text" #search-input
                                            placeholder="Tapez au moins 4 caractères..."
                                            autocomplete="off";
                                        span .icon.is-left { i .fa-solid.fa-magnifying-glass {} }
                                    }
                                    p .help { "Entrez votre prénom ou nom de famille" }
                                }
                                nav .panel.d-none #results-panel {}
                                div #confirm-box .d-none.notification.is-info.is-light.mt-4 {
                                    p #confirm-text {}
                                    button .button.is-primary.mt-3 #send-btn {
                                        span .icon { i .fa-solid.fa-envelope {} }
                                        span { "Envoyer le lien de connexion" }
                                    }
                                }
                                div #success-box .d-none.notification.is-success.is-light.mt-4 {
                                    p {
                                        span .icon { i .fa-solid.fa-check {} }
                                        " Un email de connexion a été envoyé. Vérifiez votre boîte de réception."
                                    }
                                }
                                div #error-box .d-none.notification.is-danger.is-light.mt-4 {
                                    p #error-text {}
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let extra_scripts = html! {};

    page(
        "Connexion - AGHIL",
        prefix,
        &NavKind::LoginOnly,
        "",
        html! {},
        content,
        extra_scripts,
    )
}

pub fn audit_page(
    entries: &[crate::database::AuditEntry],
    current_page: i64,
    total_pages: i64,
    prefix: &str,
) -> Markup {
    let content = html! {
        section .section {
            div .container.is-fluid {
                h1 .title.is-4 {
                    span .icon.mr-2 { i .fa-solid.fa-clipboard-list {} }
                    "Journal d'audit"
                }
                div .table-container {
                    table .table.is-striped.is-hoverable.is-fullwidth {
                        thead {
                            tr {
                                th { "Date" }
                                th { "Qui" }
                                th { "Opération" }
                                th { "Détail" }
                            }
                        }
                        tbody {
                            @if entries.is_empty() {
                                tr {
                                    td colspan="4" .has-text-centered.has-text-grey-light {
                                        "Aucune entrée"
                                    }
                                }
                            }
                            @for e in entries {
                                @let ts = e.created_at.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string();
                                tr {
                                    td .is-size-7.is-nowrap { (ts) }
                                    td { (e.staff_name) }
                                    td { (e.operation) }
                                    td .is-size-7 { (e.detail) }
                                }
                            }
                        }
                    }
                }
                @if total_pages > 1 {
                    nav .pagination.is-centered.mt-4 role="navigation" {
                        ul .pagination-list {
                            @for p in 1..=total_pages {
                                li {
                                    @if p == current_page {
                                        a .pagination-link.is-current { (p) }
                                    } @else {
                                        a .pagination-link href=(format!("{prefix}/audit?page={p}")) { (p) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    page(
        "Journal d'audit - AGHIL",
        prefix,
        &NavKind::Full,
        "",
        html! {},
        content,
        html! {},
    )
}

pub fn validation_page(pending: &[(Staff, Atelier)], prefix: &str) -> Markup {
    let content = html! {
        section .section {
            div .container.is-fluid {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href=(format!("{prefix}/")) { "Accueil" } }
                        li .is-active { a href="#" aria-current="page" { "Validations" } }
                    }
                }

                h1 .title.is-4 {
                    span .icon.mr-2 { i .fa-solid.fa-user-check {} }
                    "Demandes en attente de validation"
                }
                div .table-container {
                    table .table.is-striped.is-hoverable.is-fullwidth {
                        thead {
                            tr {
                                th { "Bénévole" }
                                th { "Atelier" }
                                th { "Actions" }
                            }
                        }
                        tbody {
                            @for (staff, atelier) in pending {
                                tr id=(format!("row-{}-{}", staff.id, atelier.id)) {
                                    td {
                                        a href=(format!("{prefix}/person/{}", staff.id)) {
                                            (staff.first_name) " " (staff.last_name)
                                        }
                                    }
                                    td { (atelier.name) }
                                    td {
                                        div .buttons.are-small {
                                            button .button.is-success
                                                onclick=(format!("doValidate('{}', '{}', true)", staff.id, atelier.id)) {
                                                span .icon { i .fa-solid.fa-check {} }
                                                span { "Valider" }
                                            }
                                            button .button.is-danger.is-outlined
                                                onclick=(format!("doValidate('{}', '{}', false)", staff.id, atelier.id)) {
                                                span .icon { i .fa-solid.fa-xmark {} }
                                                span { "Refuser" }
                                            }
                                        }
                                    }
                                }
                            }
                            @if pending.is_empty() {
                                tr {
                                    td colspan="3" .has-text-centered.has-text-grey-light.py-5 {
                                        "Aucune demande en attente de validation"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let script = html! {};

    page(
        "Validations - AGHIL",
        prefix,
        &NavKind::LoginOnly,
        "",
        html! {},
        content,
        script,
    )
}

pub fn photo_page(prefix: &str, photos: &[(PhotoMeta, String)], is_admin: bool) -> Markup {
    let content = html! {
        section .section {
            div .container.is-fluid {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href=(format!("{prefix}/")) { "Accueil" } }
                        li .is-active { a href="#" aria-current="page" { "Photos" } }
                    }
                }

                h1 .title.is-4 {
                    span .icon.mr-2 { i .fa-solid.fa-images {} }
                    "Gestion des photos"
                }

                @if is_admin {
                    div .box {
                        form #photo-upload-form action=(format!("{prefix}/photos/upload")) method="post" enctype="multipart/form-data" {
                            input type="hidden" name="photographer_id" #photographer_id;
                            div .field {
                                label .label { "Photographe" }
                                div .control.has-icons-left {
                                    input .input type="text" #photographer-search
                                        placeholder="Rechercher un bénévole (4 car. min)"
                                        autocomplete="off";
                                    span .icon.is-left { i .fa-solid.fa-user {} }
                                }
                                nav .panel.search-dropdown #photographer-results {}
                                p .help.d-none #photographer-selected {
                                    span .tag.is-success.is-medium #photographer-selected-tag {}
                                    a #photographer-clear .ml-2.is-clickable { "Changer" }
                                }
                            }
                            div #create-staff-box .d-none.notification.is-light.mt-2.mb-4 {
                                p .mb-2 { strong { "Créer un nouveau bénévole" } }
                                div .field.is-horizontal {
                                    div .field-body {
                                        div .field {
                                            div .control {
                                                input .input type="text" #new-staff-first placeholder="Prénom";
                                            }
                                        }
                                        div .field {
                                            div .control {
                                                input .input type="text" #new-staff-last placeholder="Nom";
                                            }
                                        }
                                    }
                                }
                                div .field.is-horizontal.mt-2 {
                                    div .field-body {
                                        div .field {
                                            div .control.has-icons-left {
                                                input .input type="email" #new-staff-email placeholder="Email";
                                                span .icon.is-left { i .fa-solid.fa-envelope {} }
                                            }
                                        }
                                        div .field {
                                            div .control.has-icons-left {
                                                input .input type="tel" #new-staff-phone placeholder="Téléphone";
                                                span .icon.is-left { i .fa-solid.fa-phone {} }
                                            }
                                        }
                                        div .field {
                                            div .control {
                                                button type="button" .button.is-info #create-staff-btn { "Créer" }
                                            }
                                        }
                                    }
                                }
                                p .help.is-danger.d-none #create-staff-error {}
                            }
                            div .field {
                                label .label { "Photo" }
                                div .control {
                                    div .file.has-name.is-primary {
                                        label .file-label {
                                            input .file-input type="file" name="photo" accept="image/*" required;
                                            span .file-cta {
                                                span .file-icon { i .fa-solid.fa-upload {} }
                                                span .file-label { "Choisir un fichier..." }
                                            }
                                            span .file-name { "Aucun fichier sélectionné" }
                                        }
                                    }
                                }
                            }
                            div .field {
                                div .control {
                                    button type="submit" .button.is-primary #upload-btn disabled {
                                        span .icon { i .fa-solid.fa-cloud-arrow-up {} }
                                        span { "Télécharger" }
                                    }
                                }
                            }
                        }
                    }
                }

                h2 .title.is-5.mt-6 { "Photos disponibles" }
                div .columns.is-multiline {
                    @if photos.is_empty() {
                        div .column {
                            div .notification.is-info { "Aucune photo disponible" }
                        }
                    }
                    @for (photo, photographer_name) in photos {
                        @let photo_url = format!("{}/photos/{}", prefix, photo.id);
                        @let icon = if photo.mime_type.starts_with("image/") {
                            "fa-image"
                        } else if photo.mime_type.starts_with("video/") {
                            "fa-video"
                        } else {
                            "fa-file"
                        };
                        div .column.is-one-quarter {
                            div .card {
                                div .card-image {
                                    figure .image.is-4by3 {
                                        a href=(photo_url) target="_blank" {
                                            @if photo.mime_type.starts_with("image/") {
                                                img .image-cover src=(photo_url) alt=(format!("Photo par {photographer_name}"));
                                            } @else {
                                                span .icon.is-large.has-text-link {
                                                    i class={"fa-solid " (icon) " fa-4x"} {}
                                                }
                                            }
                                        }
                                    }
                                }
                                div .card-content {
                                    div .media {
                                        div .media-content {
                                            p .title.is-6 { (photographer_name) }
                                        }
                                    }
                                }
                                @if is_admin {
                                    footer .card-footer {
                                        form .is-fullwidth action=(format!("{}/photos/{}/delete", prefix, photo.id))
                                            method="post"
                                            onsubmit="return confirm('Supprimer cette photo ?')" {
                                            button type="submit" .card-footer-item.has-text-danger.button-unstyled {
                                                span .icon { i .fa-solid.fa-trash {} }
                                                span { "Supprimer" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let script = html! {};

    page(
        "Photos - AGHIL",
        prefix,
        &NavKind::StaffOnly,
        "",
        html! {},
        content,
        script,
    )
}

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
