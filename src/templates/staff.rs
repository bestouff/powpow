use super::{NavKind, TodoItem, capitalize_words, format_phone_international, page};
use crate::models::{Atelier, Role, Staff};
use chrono::Datelike;
use maud::{Markup, PreEscaped, html};

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
                    div .table-container.staff-table {
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
        "admin",
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
