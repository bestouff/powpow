use super::{NavKind, TodoItem, capitalize_words, format_phone_international, page};
use crate::models::{Atelier, Qualification, Role, Staff, StaffQualif};
use chrono::Datelike;
use maud::{Markup, PreEscaped, html};

/// Check if a qualification is expired given its duration (years) and obtained date.
/// Returns `true` if expired, `false` if still valid or lifelong.
fn is_qualification_expired(qual: &Qualification, obtained_date: chrono::NaiveDate) -> bool {
    if let Some(duration) = qual.duration {
        let months = u32::from(duration.unsigned_abs()) * 12;
        let expiry = obtained_date + chrono::Months::new(months);
        expiry < chrono::Utc::now().date_naive()
    } else {
        false
    }
}

#[allow(clippy::too_many_arguments)]
pub fn staff_list(
    staff_with_seasons: Vec<(Staff, Option<i16>)>,
    ateliers: &[Atelier],
    roles: &[Role],
    qualifications: &[Qualification],
    staff_qualifs: &[StaffQualif],
    current_season: i16,
    prefix: &str,
    show_contact: bool,
) -> Markup {
    let p = prefix;
    let count_all = staff_with_seasons.len();
    let count_members = staff_with_seasons
        .iter()
        .filter(|(_, s)| *s == Some(current_season))
        .count();
    let count_volunteers = staff_with_seasons
        .iter()
        .filter(|(staff, s)| {
            *s == Some(current_season) && roles.iter().any(|r| r.staff == staff.id)
        })
        .count();
    let count_chiefs = staff_with_seasons
        .iter()
        .filter(|(staff, s)| {
            *s == Some(current_season) && roles.iter().any(|r| r.staff == staff.id && r.chief)
        })
        .count();

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
                        div .tags {
                            span .tag.is-light.is-medium { (count_all) " Personnes" }
                            span .tag.is-success.is-medium { (count_members) " Adhérents à jour" }
                            span .tag.is-info.is-medium { (count_volunteers) " Bénévoles" }
                            span .tag.is-warning.is-medium { (count_chiefs) " Chefs" }
                        }
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
                    @if !qualifications.is_empty() {
                        p .mt-2 {
                            strong { "Légende formations:" }
                            span .tag.is-success.ml-2 { "Valide" }
                            span .tag.is-danger.ml-2 { "Expirée" }
                        }
                    }
                }

                div .box {
                    div .table-container.staff-table {
                        table .table.is-fullwidth.is-striped.is-hoverable {
                            thead {
                                tr {
                                    th .sticky-col { "Nom" }
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
                                    @if !qualifications.is_empty() {
                                        th { "Formations" }
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
                                        td .sticky-col {
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
                                        // Formations column
                                        @if !qualifications.is_empty() {
                                            td {
                                                div .tags {
                                                    @for qual in qualifications {
                                                        // Find the most recent record for this staff + qualification
                                                        @let latest = staff_qualifs.iter()
                                                            .filter(|sq| sq.staff == staff.id && sq.qualification == qual.id)
                                                            .max_by_key(|sq| sq.obtained_date);
                                                        @if let Some(sq) = latest {
                                                            @let tag_class = if is_qualification_expired(qual, sq.obtained_date) { "is-danger" } else { "is-success" };
                                                            span class={
                                                                "tag " (tag_class)
                                                            } { (qual.name) }
                                                        }
                                                    }
                                                }
                                            }
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
        "Liste des Staff - PowPow",
        prefix,
        &NavKind::Standard,
        "admin",
        extra_head,
        content,
        html! {},
    )
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::fn_params_excessive_bools)]
pub fn person_detail(
    staff: &Staff,
    ateliers: &[Atelier],
    roles: &[Role],
    current_season: i16,
    prefix: &str,
    is_self: bool,
    is_admin: bool,
    is_god: bool,
    show_contact: bool,
    todos: &[TodoItem],
    payment_history: &[crate::models::PaymentHistoryEntry],
    person_calendar: &[(crate::models::Need, String, String, String, bool, bool)],
    person_qualifications: &[(crate::models::StaffQualif, String, Option<i16>)],
    all_qualifications: &[crate::models::Qualification],
) -> Markup {
    let p = prefix;
    let can_edit_ateliers = is_self || is_admin;
    let can_edit_contact = is_self || is_admin;
    let can_manage_qualifs = is_self || is_admin;

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
                                        span .icon.has-text-warning { i .fa-solid.fa-crown {} }
                                        span { "Admin" }
                                    }
                                }
                                div .field {
                                    label .checkbox {
                                        input type="checkbox" #god-cb checked[staff.is_god];
                                        span .icon.has-text-danger { i .fa-solid.fa-hamsa {} }
                                        span { "God" }
                                    }
                            }
                        }
                    }

                        // Notifications box (self, admin only)
                        @if is_self && is_admin {
                            div .box {
                                h2 .title.is-4 {
                                    span .icon { i .fa-solid.fa-envelope {} }
                                    "\u{00a0}Notifications"
                                }
                                div .field.mb-3 {
                                    label .checkbox {
                                        input type="checkbox" #optout-import-cb checked[staff.no_import_emails];
                                        span { "Je ne veux plus recevoir de mails à propos des inscriptions à valider" }
                                    }
                                }
                                div .field {
                                    label .checkbox {
                                        input type="checkbox" #optout-weekly-cb checked[staff.no_weekly_emails];
                                        span { "Je ne veux plus recevoir de mails récapitulatif du lundi matin" }
                                    }
                                }
                            }
                        }

                        // Delete box (god only, never self)
                        @if is_god && !is_self {
                            div .box {
                                h2 .title.is-4 {
                                    span .icon.has-text-danger { i .fa-solid.fa-trash {} }
                                    "\u{00a0}Suppression"
                                }
                                div .content {
                                    p .has-text-grey { "Supprimer définitivement ce membre." }
                                }
                                button .button.is-danger.is-small #delete-staff-btn {
                                    span .icon { i .fa-solid.fa-trash {} }
                                    span { "Supprimer" }
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

                        // Formations box
                        @if can_manage_qualifs || !person_qualifications.is_empty() {
                            div .box {
                                h2 .title.is-4 {
                                    span .icon { i .fa-solid.fa-certificate {} }
                                    "\u{00a0}Formations"
                                }

                                @if !person_qualifications.is_empty() {
                                    div .table-container {
                                        table .table.is-striped.is-hoverable.is-fullwidth {
                                            thead {
                                                tr {
                                                    th { "Qualification" }
                                                    th { "Obtenu le" }
                                                    th { "Statut" }
                                                    th { "Justificatif" }
                                                    @if can_manage_qualifs {
                                                        th {}
                                                    }
                                                }
                                            }
                                            tbody {
                                                @for (sq, name, duration) in person_qualifications {
                                                    @let expired = duration.is_some_and(|d| {
                                                        let months = u32::from(d.unsigned_abs()) * 12;
                                                        let expiry = sq.obtained_date + chrono::Months::new(months);
                                                        expiry < chrono::Utc::now().date_naive()
                                                    });
                                                    @let (tag_class, tag_text) = if expired {
                                                        ("is-danger", "Expirée")
                                                    } else if duration.is_some() {
                                                        ("is-success", "Valide")
                                                    } else {
                                                        ("is-success", "Permanent")
                                                    };
                                                    tr {
                                                        td { (name) }
                                                        td { (sq.obtained_date.format("%d/%m/%Y")) }
                                                        td {
                                                            span class={"tag " (tag_class)} { (tag_text) }
                                                            @if let Some(d) = duration {
                                                                @let expiry_date = sq.obtained_date + chrono::Months::new(u32::from(d.unsigned_abs()) * 12);
                                                                small .ml-1.has-text-grey { " → " (expiry_date.format("%d/%m/%Y")) }
                                                            }
                                                        }
                                                        td .pq-proof-cell data-id=(sq.id) {
                                                            @if sq.has_training_proof {
                                                                a .button.is-small.is-info.is-outlined
                                                                    href={(p) "/staff-qualif/" (sq.id) "/proof"}
                                                                    target="_blank" {
                                                                    span .icon { i .fa-solid.fa-file-image {} }
                                                                    span { "Voir" }
                                                                }
                                                                @if can_manage_qualifs {
                                                                    button .button.is-small.is-danger.is-outlined.ml-1.pq-proof-delete-btn
                                                                        data-id=(sq.id) {
                                                                        span .icon { i .fa-solid.fa-xmark {} }
                                                                    }
                                                                }
                                                            } @else if can_manage_qualifs {
                                                                label .button.is-small.is-link.is-outlined {
                                                                    span .icon { i .fa-solid.fa-upload {} }
                                                                    span { "Ajouter" }
                                                                    input .pq-proof-upload type="file"
                                                                        data-id=(sq.id)
                                                                        accept="image/*,application/pdf"
                                                                        style="display:none";
                                                                }
                                                            } @else {
                                                                span .has-text-grey-light { "\u{2014}" }
                                                            }
                                                        }
                                                        @if can_manage_qualifs {
                                                            td {
                                                                button .button.is-small.is-danger.is-outlined.pq-delete-btn
                                                                    data-id=(sq.id) {
                                                                    span .icon { i .fa-solid.fa-trash {} }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } @else {
                                    p .has-text-grey-light.mb-4 { "Aucune qualification enregistrée" }
                                }

                                // Add form (self or admin)
                                @if can_manage_qualifs && !all_qualifications.is_empty() {
                                    hr;
                                    h3 .title.is-6 { "Ajouter une qualification" }
                                    div .columns.is-multiline {
                                        div .column.is-one-third {
                                            div .field {
                                                label .label { "Qualification" }
                                                div .control {
                                                    div .select.is-fullwidth {
                                                        select #pq-qual {
                                                            option value="" { "Choisir..." }
                                                            @for qual in all_qualifications {
                                                                option value=(qual.id) { (qual.name) }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        div .column.is-one-third {
                                            div .field {
                                                label .label { "Date d'obtention" }
                                                div .control {
                                                    input .input #pq-date type="date"
                                                        value=(chrono::Utc::now().date_naive());
                                                }
                                            }
                                        }
                                        div .column.is-narrow {
                                            div .field {
                                                label .label { "\u{00a0}" }
                                                div .control {
                                                    button .button.is-success #add-pq-btn {
                                                        span .icon { i .fa-solid.fa-plus {} }
                                                        span { "Ajouter" }
                                                    }
                                                }
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
                                    @if is_admin {
                                        div .column.is-narrow {
                                            button .button.is-danger.is-small.is-outlined
                                                data-payment-id=(entry.payment_id)
                                                data-staff-id=(staff.id)
                                                onclick="openUnimportModal(this)" {
                                                span .icon.is-small { i .fa-solid.fa-triangle-exclamation {} }
                                                span { "Annulation" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Unimport confirmation modal (admin only)
                @if is_admin && !payment_history.is_empty() {
                    div .modal #unimport-modal {
                        div .modal-background onclick="closeUnimportModal()" {}
                        div .modal-card {
                            header .modal-card-head {
                                p .modal-card-title {
                                    span .icon.has-text-danger { i .fa-solid.fa-triangle-exclamation {} }
                                    " Annulation d'adhésion"
                                }
                                button .delete onclick="closeUnimportModal()" aria-label="close" {}
                            }
                            section .modal-card-body {
                                div #unimport-loading .has-text-centered {
                                    span .icon.is-large { i .fa-solid.fa-spinner.fa-spin {} }
                                }
                                div #unimport-content .is-hidden {
                                    div .notification.is-warning.is-light {
                                        p {
                                            strong { "Attention" }
                                            " — ceci va annuler l'adhésion pour la saison "
                                            strong #unimport-season {}
                                            "."
                                        }
                                        p .mt-2 {
                                            "Vous pourrez la ré-importer dans "
                                            a #unimport-reimport-link href="" {}
                                            "."
                                        }
                                    }
                                    div #unimport-warnings {}
                                }
                                div #unimport-error .notification.is-danger.is-hidden {}
                            }
                            footer .modal-card-foot {
                                form #unimport-form method="post" {
                                    button .button.is-danger type="submit" #unimport-confirm disabled {
                                        span .icon { i .fa-solid.fa-trash {} }
                                        span { "Confirmer l'annulation" }
                                    }
                                }
                                button .button onclick="closeUnimportModal()" { "Annuler" }
                            }
                        }
                    }
                }

                    // Delete confirmation modal (god only)
                    @if is_god && !is_self {
                        div .modal #delete-staff-modal {
                            div .modal-background onclick="closeDeleteStaffModal()" {}
                            div .modal-card {
                                header .modal-card-head {
                                    p .modal-card-title {
                                        span .icon.has-text-danger { i .fa-solid.fa-trash {} }
                                        " Supprimer ce membre"
                                    }
                                    button .delete onclick="closeDeleteStaffModal()" aria-label="close" {}
                                }
                                section .modal-card-body {
                                    div .notification.is-warning.is-light {
                                        p {
                                            strong { "Attention" }
                                            " — cette action est définitive et supprimera "
                                            strong { (staff.first_name) " " (staff.last_name) }
                                            " de la base de données."
                                        }
                                    }
                                    div #delete-staff-error .notification.is-danger.is-hidden {}
                                    div #delete-staff-loading .has-text-centered.is-hidden {
                                        span .icon.is-large { i .fa-solid.fa-spinner.fa-spin {} }
                                    }
                                }
                                footer .modal-card-foot {
                                    button .button.is-danger #delete-staff-confirm {
                                        span .icon { i .fa-solid.fa-trash {} }
                                        span { "Supprimer définitivement" }
                                    }
                                button .button onclick="closeDeleteStaffModal()" { "Annuler" }
                            }
                        }
                    }
                }
            }
        }
    };

    let extra_scripts = html! {
        @if can_manage_qualifs {
            script {
                (maud::PreEscaped(format!(r#"(function() {{
    const PREFIX = "{}";
    const STAFF_ID = "{}";
    const IS_ADMIN = {};

    // Use admin API if admin, self-service API if self
    const addUrl = IS_ADMIN ? PREFIX + '/api/staff-qualif' : PREFIX + '/api/my/staff-qualif';
    const deleteUrl = function(id) {{
        return IS_ADMIN ? PREFIX + '/api/staff-qualif/' + id : PREFIX + '/api/my/staff-qualif/' + id;
    }};

    const addBtn = document.getElementById('add-pq-btn');
    if (addBtn) {{
        addBtn.addEventListener('click', async () => {{
            const qual = document.getElementById('pq-qual').value;
            const date = document.getElementById('pq-date').value;
            if (!qual || !date) {{ alert('Tous les champs sont requis'); return; }}
            const res = await fetch(addUrl, {{
                method: 'POST',
                headers: {{ 'Content-Type': 'application/json' }},
                body: JSON.stringify({{ staff_id: STAFF_ID, qualification_id: parseInt(qual), obtained_date: date }})
            }});
            if (res.ok) {{ location.reload(); }}
            else {{ const e = await res.json(); alert(e.error || 'Erreur'); }}
        }});
    }}

    document.querySelectorAll('.pq-delete-btn').forEach(btn => {{
        btn.addEventListener('click', async () => {{
            if (!confirm('Supprimer cette qualification ?')) return;
            const res = await fetch(deleteUrl(btn.dataset.id), {{ method: 'DELETE' }});
            if (res.ok) {{ location.reload(); }}
            else {{ const e = await res.json(); alert(e.error || 'Erreur'); }}
        }});
    }});

    document.querySelectorAll('.pq-proof-upload').forEach(inp => {{
        inp.addEventListener('change', async () => {{
            const file = inp.files[0];
            if (!file) return;
            const fd = new FormData();
            fd.append('proof', file);
            const res = await fetch(PREFIX + '/api/staff-qualif/' + inp.dataset.id + '/proof', {{
                method: 'POST', body: fd
            }});
            if (res.ok) {{ location.reload(); }}
            else {{ const e = await res.json(); alert(e.error || 'Erreur'); }}
        }});
    }});

    document.querySelectorAll('.pq-proof-delete-btn').forEach(btn => {{
        btn.addEventListener('click', async () => {{
            if (!confirm('Supprimer le justificatif ?')) return;
            const res = await fetch(PREFIX + '/api/staff-qualif/' + btn.dataset.id + '/proof', {{ method: 'DELETE' }});
            if (res.ok) {{ location.reload(); }}
            else {{ const e = await res.json(); alert(e.error || 'Erreur'); }}
        }});
    }});
}})();"#, p, staff.id, is_admin)))
            }
        }
        @if is_admin && !payment_history.is_empty() {
            script {
                (maud::PreEscaped(format!(r#"(function() {{
    const PREFIX = "{}";
    const modal = document.getElementById('unimport-modal');
    const form = document.getElementById('unimport-form');
    const loading = document.getElementById('unimport-loading');
    const content = document.getElementById('unimport-content');
    const errorDiv = document.getElementById('unimport-error');
    const seasonSpan = document.getElementById('unimport-season');
    const reimportLink = document.getElementById('unimport-reimport-link');
    const warningsDiv = document.getElementById('unimport-warnings');
    const confirmBtn = document.getElementById('unimport-confirm');

    window.openUnimportModal = async function(btn) {{
        const paymentId = btn.dataset.paymentId;
        const staffId = btn.dataset.staffId;
        modal.classList.add('is-active');
        loading.classList.remove('is-hidden');
        content.classList.add('is-hidden');
        errorDiv.classList.add('is-hidden');
        confirmBtn.disabled = true;
        warningsDiv.innerHTML = '';

        form.action = PREFIX + '/api/person/' + staffId + '/unimport/' + paymentId;

        try {{
            const res = await fetch(PREFIX + '/api/person/' + staffId + '/unimport/' + paymentId);
            if (!res.ok) {{
                const e = await res.json();
                throw new Error(e.error || 'Erreur serveur');
            }}
            const data = await res.json();
            seasonSpan.textContent = data.season;

            if (data.is_helloasso) {{
                reimportLink.href = PREFIX + '/online';
                reimportLink.textContent = 'les adhésions en ligne';
            }} else {{
                reimportLink.href = PREFIX + '/cash';
                reimportLink.textContent = 'les paiements espèces/chèques';
            }}

            let warnings = [];
            if (data.presence_count > 0) {{
                warnings.push('<span class="icon has-text-warning"><i class="fa-solid fa-calendar-xmark"></i></span> ' +
                    data.presence_count + ' créneau(x) de planning resteront enregistrés pour ce bénévole.');
            }}
            if (data.role_count > 0) {{
                warnings.push('<span class="icon has-text-info"><i class="fa-solid fa-users"></i></span> ' +
                    data.role_count + ' inscription(s) à des ateliers resteront enregistrées.');
            }}
            if (data.other_payment_count === 0) {{
                warnings.push('<span class="icon has-text-danger"><i class="fa-solid fa-user-slash"></i></span> ' +
                    'C\'est la dernière adhésion de ce bénévole — la fiche restera mais sans aucune cotisation.');
            }}
            if (warnings.length > 0) {{
                warningsDiv.innerHTML = '<div class="notification is-info is-light mt-3"><ul>' +
                    warnings.map(function(w) {{ return '<li class="mb-1">' + w + '</li>'; }}).join('') +
                    '</ul></div>';
            }}

            loading.classList.add('is-hidden');
            content.classList.remove('is-hidden');
            confirmBtn.disabled = false;
        }} catch (err) {{
            loading.classList.add('is-hidden');
            errorDiv.textContent = err.message;
            errorDiv.classList.remove('is-hidden');
        }}
    }};

    window.closeUnimportModal = function() {{
        modal.classList.remove('is-active');
    }};

    document.addEventListener('keydown', function(e) {{
        if (e.key === 'Escape') closeUnimportModal();
    }});
}})();"#, p)))
            }
        }
        @if is_god && !is_self {
            script {
                (maud::PreEscaped(format!(r#"(function() {{
    const PREFIX = "{}";
    const STAFF_ID = "{}";
    const modal = document.getElementById('delete-staff-modal');
    const errorDiv = document.getElementById('delete-staff-error');
    const loading = document.getElementById('delete-staff-loading');
    const confirmBtn = document.getElementById('delete-staff-confirm');

    window.openDeleteStaffModal = function() {{
        modal.classList.add('is-active');
        errorDiv.classList.add('is-hidden');
        loading.classList.add('is-hidden');
        confirmBtn.disabled = false;
    }};

    window.closeDeleteStaffModal = function() {{
        modal.classList.remove('is-active');
    }};

    confirmBtn.addEventListener('click', async () => {{
        confirmBtn.disabled = true;
        loading.classList.remove('is-hidden');
        errorDiv.classList.add('is-hidden');
        try {{
            const res = await fetch(PREFIX + '/api/person/' + STAFF_ID + '/delete', {{
                method: 'POST'
            }});
            const data = await res.json();
            if (res.ok) {{
                window.location.href = PREFIX + '/staff';
                return;
            }}
            let msg;
            if (res.status === 409 && Array.isArray(data.blockers)) {{
                msg = 'Ce membre ne peut pas être supprimé car il/elle :<ul class="mt-2">' +
                    data.blockers.map(function(b) {{ return '<li>' + b + '</li>'; }}).join('') +
                    '</ul>';
            }} else {{
                msg = data.error || 'Erreur serveur';
            }}
            errorDiv.innerHTML = msg;
            errorDiv.classList.remove('is-hidden');
            confirmBtn.disabled = false;
        }} catch (err) {{
            errorDiv.textContent = err.message || 'Erreur serveur';
            errorDiv.classList.remove('is-hidden');
            confirmBtn.disabled = false;
        }} finally {{
            loading.classList.add('is-hidden');
        }}
    }});

    const deleteBtn = document.getElementById('delete-staff-btn');
    if (deleteBtn) {{
        deleteBtn.addEventListener('click', openDeleteStaffModal);
    }}
}})();"#, p, staff.id)))
            }
        }
    };

    let title = format!("{} {} - PowPow", staff.first_name, staff.last_name);
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
