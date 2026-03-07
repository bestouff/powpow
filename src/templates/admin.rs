use super::{NavKind, page};
use crate::models::{
    Atelier, Equipment, EquipmentStatus, EquipmentType, Qualification, Staff, StaffQualif,
};
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
                                a .button.is-info.is-medium href={(p) "/online"} {
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
                                a .button.is-primary href={(p) "/online"} {
                                    span .icon { i .fa-solid.fa-ticket {} }
                                    span { "Adhésions HelloAsso" }
                                    span .nav-badge.d-none data-badge="online" {}
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
                                a .button.is-link.is-light href={(p) "/qualifications"} {
                                    span .icon { i .fa-solid.fa-certificate {} }
                                    span { "Qualifications" }
                                }
                            }
                        }
                    }
                }

                // État des équipements (admin/god only)
                @if is_admin || is_god {
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

                // Gestion des photos (admin only)
                @if is_admin {
                    section .section.py-4 {
                        div .box {
                            h3 .title.is-5.mb-3 {
                                span .icon.mr-2 { i .fa-solid.fa-images {} }
                                "Photos"
                            }
                            div .buttons {
                                a .button.is-info href={(p) "/photos"} {
                                    span .icon { i .fa-solid.fa-images {} }
                                    span { "Gérer les photos" }
                                }
                            }
                        }
                    }
                }

                // Contenu éditorial (admin only)
                @if is_admin {
                    section .section.py-4 {
                        div .box {
                            h3 .title.is-5.mb-3 {
                                span .icon.mr-2 { i .fa-solid.fa-pen-to-square {} }
                                "Contenu éditorial"
                            }
                            div .buttons {
                                a .button.is-info href={(p) "/admin/contents"} {
                                    span .icon { i .fa-solid.fa-pen-to-square {} }
                                    span { "Gérer le contenu du site" }
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

/// Render a single equipment status row with a clickable 3-state button.
fn equipment_toggle(prefix: &str, eq: &Equipment) -> Markup {
    let (btn_class, label) = match eq.status {
        EquipmentStatus::Open => ("is-success", "Ouvert"),
        EquipmentStatus::Closed => ("is-danger", "Fermé"),
        EquipmentStatus::Partial => ("is-warning", "Partiel"),
    };
    html! {
        div .field.is-flex.is-align-items-center.is-justify-content-space-between.mb-2 {
            span { (eq.name) }
            button class={"button is-small equip-cycle-btn " (btn_class)}
                data-id=(eq.id)
                data-prefix=(prefix)
                data-status=(eq.status)
                onclick="cycleEquipment(this)" {
                (label)
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

#[allow(clippy::too_many_arguments)]
pub fn qualifications_page(
    prefix: &str,
    qualifications: &[Qualification],
    staff_qualifs: &[(StaffQualif, String, String, Option<i16>)],
) -> Markup {
    let p = prefix;
    let today = chrono::Utc::now().date_naive();

    let content = html! {
        div #notification-container {}

        section .section {
            div .container.is-fluid {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href={(p) "/"} { "Accueil" } }
                        li { a href={(p) "/admin"} { "Administration" } }
                        li .is-active { a href="#" aria-current="page" { "Qualifications" } }
                    }
                }

                h1 .title.is-3 {
                    span .icon.mr-2 { i .fa-solid.fa-certificate {} }
                    "Gestion des qualifications"
                }

                div .columns {
                    // Left column: Qualification types
                    div .column.is-one-third {
                        div .box {
                            h2 .title.is-5 {
                                span .icon { i .fa-solid.fa-list {} }
                                "\u{00a0}Types de qualification"
                            }

                            // Existing qualifications
                            @for qual in qualifications {
                                div .is-flex.is-align-items-center.is-justify-content-space-between.mb-3 {
                                    div {
                                        strong { (qual.name) }
                                        @if let Some(d) = qual.duration {
                                            span .tag.is-light.ml-2 { (d) " an(s)" }
                                        } @else {
                                            span .tag.is-light.ml-2 { "Permanent" }
                                        }
                                    }
                                    button .button.is-small.is-danger.is-outlined.qual-delete-btn
                                        data-id=(qual.id)
                                        data-name=(qual.name) {
                                        span .icon { i .fa-solid.fa-trash {} }
                                    }
                                }
                            }

                            @if qualifications.is_empty() {
                                p .has-text-grey-light.mb-4 { "Aucune qualification définie" }
                            }

                            // Add form
                            hr;
                            h3 .title.is-6 { "Ajouter un type" }
                            div .field {
                                label .label { "Nom" }
                                div .control {
                                    input .input #qual-name type="text" placeholder="ex: PSE1";
                                }
                            }
                            div .field {
                                label .label { "Durée de validité (années)" }
                                div .control {
                                    input .input #qual-duration type="number" min="1" placeholder="Vide = permanent";
                                }
                            }
                            div .field {
                                button .button.is-success #add-qual-btn {
                                    span .icon { i .fa-solid.fa-plus {} }
                                    span { "Ajouter" }
                                }
                            }
                        }
                    }

                    // Right column: Staff qualifications
                    div .column {
                        div .box {
                            h2 .title.is-5 {
                                span .icon { i .fa-solid.fa-user-graduate {} }
                                "\u{00a0}Qualifications du staff"
                            }

                            // Add form
                            div .columns.mb-4 {
                                div .column {
                                    div .field {
                                        label .label { "Bénévole" }
                                        div .control.has-icons-left {
                                            input .input #sq-staff-search type="text"
                                                placeholder="Tapez au moins 4 caractères..."
                                                autocomplete="off";
                                            span .icon.is-left { i .fa-solid.fa-magnifying-glass {} }
                                        }
                                        nav .panel.d-none #sq-staff-results {}
                                        input type="hidden" #sq-staff-id value="";
                                        p .help #sq-staff-selected .d-none {
                                            span .icon.is-small { i .fa-solid.fa-check {} }
                                            span #sq-staff-selected-name {}
                                        }
                                    }
                                }
                                div .column {
                                    div .field {
                                        label .label { "Qualification" }
                                        div .control {
                                            div .select.is-fullwidth {
                                                select #sq-qual {
                                                    option value="" { "Choisir..." }
                                                    @for qual in qualifications {
                                                        option value=(qual.id) { (qual.name) }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                div .column {
                                    div .field {
                                        label .label { "Date d'obtention" }
                                        div .control {
                                            input .input #sq-date type="date" value=(today);
                                        }
                                    }
                                }
                                div .column.is-narrow {
                                    div .field {
                                        label .label { "\u{00a0}" }
                                        div .control {
                                            button .button.is-success #add-sq-btn {
                                                span .icon { i .fa-solid.fa-plus {} }
                                                span { "Ajouter" }
                                            }
                                        }
                                    }
                                }
                            }

                            // Table of assignments
                            div .table-container {
                                table .table.is-striped.is-hoverable.is-fullwidth {
                                    thead {
                                        tr {
                                            th { "Bénévole" }
                                            th { "Qualification" }
                                            th { "Obtenu le" }
                                            th { "Statut" }
                                            th {}
                                        }
                                    }
                                    tbody {
                                        @for (sq, staff_name, qual_name, duration) in staff_qualifs {
                                            @let expired = duration.is_some_and(|d| {
                                                let months = u32::from(d.unsigned_abs()) * 12;
                                                let expiry = sq.obtained_date + chrono::Months::new(months);
                                                expiry < today
                                            });
                                            @let status_tag = if expired {
                                                ("is-danger", "Expirée")
                                            } else if duration.is_some() {
                                                ("is-success", "Valide")
                                            } else {
                                                ("is-success", "Permanent")
                                            };
                                            tr {
                                                td {
                                                    a href={(p) "/person/" (sq.staff)} { (staff_name) }
                                                }
                                                td { (qual_name) }
                                                td { (sq.obtained_date.format("%d/%m/%Y")) }
                                                td {
                                                    span class={"tag " (status_tag.0)} { (status_tag.1) }
                                                    @if let Some(d) = duration {
                                                        @let expiry_date = sq.obtained_date + chrono::Months::new(u32::from(d.unsigned_abs()) * 12);
                                                        small .ml-1.has-text-grey { " → " (expiry_date.format("%d/%m/%Y")) }
                                                    }
                                                }
                                                td {
                                                    button .button.is-small.is-danger.is-outlined.sq-delete-btn
                                                        data-id=(sq.id) {
                                                        span .icon { i .fa-solid.fa-trash {} }
                                                    }
                                                }
                                            }
                                        }
                                        @if staff_qualifs.is_empty() {
                                            tr {
                                                td colspan="5" .has-text-centered.has-text-grey-light.py-5 {
                                                    "Aucune qualification attribuée"
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

    let script = html! {
        script {
            (maud::PreEscaped(format!(r#"
const PREFIX = "{}";

document.getElementById('add-qual-btn').addEventListener('click', async () => {{
    const name = document.getElementById('qual-name').value.trim();
    if (!name) {{ alert('Nom requis'); return; }}
    const dur = document.getElementById('qual-duration').value;
    const duration = dur ? parseInt(dur) : null;
    const res = await fetch(PREFIX + '/api/qualifications', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{ name, duration }})
    }});
    if (res.ok) {{ location.reload(); }}
    else {{ const e = await res.json(); alert(e.error || 'Erreur'); }}
}});

document.querySelectorAll('.qual-delete-btn').forEach(btn => {{
    btn.addEventListener('click', async () => {{
        const name = btn.dataset.name;
        if (!confirm('Supprimer la qualification "' + name + '" et toutes ses attributions ?')) return;
        const res = await fetch(PREFIX + '/api/qualifications/' + btn.dataset.id, {{ method: 'DELETE' }});
        if (res.ok) {{ location.reload(); }}
        else {{ const e = await res.json(); alert(e.error || 'Erreur'); }}
    }});
}});

document.getElementById('add-sq-btn').addEventListener('click', async () => {{
    const staff = document.getElementById('sq-staff-id').value;
    const qual = document.getElementById('sq-qual').value;
    const date = document.getElementById('sq-date').value;
    if (!staff || !qual || !date) {{ alert('Tous les champs sont requis'); return; }}
    const res = await fetch(PREFIX + '/api/staff-qualif', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{ staff_id: staff, qualification_id: parseInt(qual), obtained_date: date }})
    }});
    if (res.ok) {{ location.reload(); }}
    else {{ const e = await res.json(); alert(e.error || 'Erreur'); }}
}});

document.querySelectorAll('.sq-delete-btn').forEach(btn => {{
    btn.addEventListener('click', async () => {{
        if (!confirm('Supprimer cette qualification ?')) return;
        const res = await fetch(PREFIX + '/api/staff-qualif/' + btn.dataset.id, {{ method: 'DELETE' }});
        if (res.ok) {{ location.reload(); }}
        else {{ const e = await res.json(); alert(e.error || 'Erreur'); }}
    }});
}});
"#, p)))
        }
    };

    page(
        "Qualifications - AGHIL",
        prefix,
        &NavKind::Standard,
        "admin",
        html! {},
        content,
        script,
    )
}
