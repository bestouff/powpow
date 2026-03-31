use super::{
    ImportContext, NavKind, capitalize_words, escape_html, format_phone_international, page,
    render_import_form,
};
use crate::models::{Membership, MembershipWithStatus, StaffWithSeason, User};
use maud::{Markup, html};

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
                          href={(p) "/online"} {
                            span .icon.is-large.has-text-info {
                                i .fa-solid.fa-ticket.fa-2x {}
                            }
                            p .stat-number.has-text-info.mt-2 { (total_count) }
                            p .has-text-grey { "Total adhésions" }
                        }
                    }
                    div .column.is-4 {
                        a .box.stat-card.has-text-centered href={(p) "/online?filter=all"} {
                            span .icon.is-large.has-text-success {
                                i .fa-solid.fa-circle-check.fa-2x {}
                            }
                            p .stat-number.has-text-success.mt-2 { (imported_count) }
                            p .has-text-grey { "Importées" }
                        }
                    }
                    div .column.is-4 {
                        a class={"box stat-card has-text-centered" (not_imported_card_active)}
                          href={(p) "/online?filter=not_imported"} {
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
                    form #filterForm method="GET" action={(p) "/online"} {
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
                                              href={(p) "/online?search=" (search_value)} {
                                                span .icon { i .fa-solid.fa-list {} }
                                                span { "Toutes" }
                                            }
                                            a class={"button " (filter_not_imported_class) " is-medium"}
                                              href={(p) "/online?search=" (search_value) "&filter=not_imported"} {
                                                span .icon { i .fa-solid.fa-circle-exclamation {} }
                                                span { "À importer" }
                                            }
                                        }
                                    }
                                    @if has_filters {
                                        a .button.is-light.is-small.ml-2 href={(p) "/online"} {
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
                                        Some("Registration") => ("Forfait", "is-link"),
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
                                            @if matches!(membership.item_type.as_deref(), Some("Registration" | "Donation")) {
                                                span .tag.is-light { "Ignoré" }
                                            } @else if membership_with_status.is_double_subscription {
                                                @if let Some(sid) = membership_with_status.staff_id {
                                                    a .tag.is-danger href={(p) "/person/" (sid)} { "Double adhésion" }
                                                } @else {
                                                    span .tag.is-danger { "Double adhésion" }
                                                }
                                            } @else if let Some(sid) = membership_with_status.staff_id {
                                                a .tag.is-success href={(p) "/person/" (sid)} { "Importé" }
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
        "admin",
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

                            a .button.is-primary.is-medium.mt-4 href={(p) "/online"} {
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
        "Adhésion déjà importée - PowPow",
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

    let membership_email = membership.email.as_deref().unwrap_or("").to_lowercase();
    let payer = payer_email.unwrap_or("").to_lowercase();
    let default_email = if membership_email.is_empty() {
        payer.clone()
    } else {
        membership_email.clone()
    };
    let phone = format_phone_international(membership.phone.as_deref().unwrap_or(""));
    let comment = membership.comment.as_deref().unwrap_or("").to_string();

    let item_name = membership.item_name.as_deref().unwrap_or("N/A");
    let amount = membership.amount.map_or_else(
        || "N/A".to_string(),
        |a| format!("{:.2}\u{20ac}", a as f32 / 100.0),
    );
    let order_date = membership
        .order_date
        .map_or_else(|| "N/A".to_string(), |d| d.format("%d/%m/%Y").to_string());
    let is_donation = membership.item_type.as_deref() == Some("Donation");

    let beneficiary_name = format!("{beneficiary_first} {beneficiary_last}")
        .trim()
        .to_string();
    let membership_email_display = if membership_email.is_empty() {
        "N/A".to_string()
    } else {
        membership_email.clone()
    };

    let detail_rows = vec![
        (
            "Bénéficiaire",
            format!("<strong>{}</strong>", escape_html(&beneficiary_name)),
        ),
        ("Email bénéficiaire", escape_html(&membership_email_display)),
        ("Email payeur", escape_html(&payer)),
        ("Téléphone", escape_html(&phone)),
        ("Article", escape_html(item_name)),
        ("Montant", escape_html(&amount)),
        ("Date", escape_html(&order_date)),
        (
            "Saison",
            format!("<span class=\"tag is-info is-medium\">{season}</span>"),
        ),
    ];

    let ctx = ImportContext {
        first_name: beneficiary_first,
        last_name: beneficiary_last,
        primary_email: membership_email,
        payer_email: payer,
        default_email,
        phone,
        default_comment: comment,
        is_donation,
        allow_create: !name_already_exists,
        name_choice_value: "membership",
        name_choice_label: "De l'adhésion:",
        page_title: "Importer Staff - PowPow",
        page_heading: "Importer un Staff",
        detail_title: "Détails de l'Adhésion",
        back_suffix: "/online",
        nav_active: "",
        detail_rows,
    };

    render_import_form(&ctx, &candidates, prefix)
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
                        a .button.is-light href={(p) "/online"} {
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
                    a .button.is-primary href={(p) "/online?filter=not_imported"} {
                        span .icon { i .fa-solid.fa-arrow-left {} }
                        span { "Retour aux adhésions à importer" }
                    }
                    a .button.is-light href={(p) "/online"} {
                        span .icon { i .fa-solid.fa-list {} }
                        span { "Voir toutes les adhésions" }
                    }
                }
            }
        }
    };

    let page_title = format!("{title} - PowPow");
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
