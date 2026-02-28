use super::{NavKind, page};
use crate::models::{Atelier, Equipment, EquipmentType, Staff};
use maud::{Markup, html};

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
        "admin",
        html! {},
        content,
        html! {},
    )
}

pub fn admin_page(prefix: &str, is_admin: bool, is_god: bool, equipments: &[Equipment]) -> Markup {
    let p = prefix;
    let slopes: Vec<&Equipment> = equipments
        .iter()
        .filter(|e| e.equipment_type == EquipmentType::SkiSlope)
        .collect();
    let tows: Vec<&Equipment> = equipments
        .iter()
        .filter(|e| e.equipment_type == EquipmentType::SkiTow)
        .collect();

    let content = html! {
        section .section {
            div .container.is-fluid {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href=(format!("{p}/")) { "Accueil" } }
                        li .is-active { a href="#" aria-current="page" { "Administration" } }
                    }
                }

                h1 .title.is-3 {
                    span .icon.mr-2 { i .fa-solid.fa-screwdriver-wrench {} }
                    "Administration"
                }

                // Gestion des adhésions (admin only)
                @if is_admin {
                    section .section.py-4 {
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

                // Gestion du staff (chief + admin)
                section .section.py-4 {
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
                            a .button.is-link.is-light href={(p) "/validation"} {
                                span .icon { i .fa-solid.fa-user-check {} }
                                span { "Validations" }
                                span .nav-badge.d-none data-badge="validations" {}
                            }
                            @if is_admin {
                                a .button.is-light href={(p) "/export/mailchimp"} {
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

                // État des équipements (admin only)
                @if is_admin {
                    section .section.py-4 {
                        div .box {
                            h3 .title.is-5.mb-3 {
                                span .icon.mr-2 { i .fa-solid.fa-mountain-sun {} }
                                "État des équipements"
                            }

                            div .columns {
                                // Pistes de ski
                                div .column {
                                    h4 .title.is-6.mb-3 {
                                        span .icon.mr-1 { i .fa-solid.fa-person-skiing {} }
                                        "Pistes de ski"
                                    }
                                    @for eq in &slopes {
                                        (equipment_toggle(p, eq))
                                    }
                                }
                                // Téléskis
                                div .column {
                                    h4 .title.is-6.mb-3 {
                                        span .icon.mr-1 { i .fa-solid.fa-cable-car {} }
                                        "Téléskis"
                                    }
                                    @for eq in &tows {
                                        (equipment_toggle(p, eq))
                                    }
                                }
                            }
                        }
                    }
                }

                // Sauvegarde / Restauration (god only)
                @if is_god {
                    section .section.py-4 {
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
        "Administration - AGHIL",
        prefix,
        &NavKind::Standard,
        "admin",
        html! {},
        content,
        html! {},
    )
}

/// Render a single equipment toggle row.
fn equipment_toggle(prefix: &str, eq: &Equipment) -> Markup {
    let checked = eq.in_service;
    html! {
        div .field.is-flex.is-align-items-center.is-justify-content-space-between.mb-2 {
            span { (eq.name) }
            label .switch {
                input type="checkbox"
                    checked[checked]
                    data-id=(eq.id)
                    data-prefix=(prefix)
                    onchange="toggleEquipment(this)";
                span .slider.is-rounded {}
            }
        }
    }
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
