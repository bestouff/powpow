use super::{
    ImportContext, NavKind, capitalize_words, escape_html, format_phone_international, page,
    render_import_form,
};
use crate::models::{Cash, StaffWithSeason};
use chrono::Datelike;
use maud::{Markup, html};

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
        "admin",
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
        "admin",
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
    let default_email = cash_email.clone();
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

    let email_display = if cash_email.is_empty() {
        "N/A".to_string()
    } else {
        cash_email.clone()
    };
    let beneficiary_name = format!("{beneficiary_first} {beneficiary_last}");

    let detail_rows = vec![
        (
            "Nom",
            format!("<strong>{}</strong>", escape_html(&beneficiary_name)),
        ),
        ("Email", escape_html(&email_display)),
        ("Téléphone", escape_html(&phone)),
        ("Moyen", escape_html(method_label)),
        ("Type", escape_html(type_label)),
        ("Montant", escape_html(&amount)),
        ("Date", escape_html(&date)),
        (
            "Saison",
            format!("<span class=\"tag is-info is-medium\">{season}</span>"),
        ),
    ];

    let ctx = ImportContext {
        first_name: beneficiary_first,
        last_name: beneficiary_last,
        primary_email: cash_email,
        payer_email: String::new(),
        default_email,
        phone,
        default_comment: String::new(),
        is_donation: false,
        allow_create: true,
        name_choice_value: "cash",
        name_choice_label: "Du paiement:",
        page_title: "Importer paiement - AGHIL",
        page_heading: "Importer un paiement",
        detail_title: "Détails du paiement",
        back_suffix: "/cash",
        nav_active: "admin",
        detail_rows,
    };

    render_import_form(&ctx, &candidates, prefix)
}
