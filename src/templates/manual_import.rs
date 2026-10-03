use maud::{Markup, html};

use super::{NavKind, page};
use crate::{auto_import::Source, manual_import::Context};

pub fn manual_import_page(context: &Context, prefix: &str) -> Markup {
    let (kind, id, back) = match context.source {
        Source::HelloAsso(id) => ("online", id.to_string(), "/online"),
        Source::Cash(id) => ("cash", id.to_string(), "/cash"),
    };
    if let Some(staff) = context.imported_to {
        return page(
            "Adhésion déjà importée - PowPow",
            prefix,
            &NavKind::Standard,
            "admin",
            html! {},
            html! {
                section .section {
                    div .container {
                        h1 .title { "Adhésion déjà importée" }
                        p .mb-4 { "Cette adhésion est déjà rattachée à une fiche staff. Annulez son importation avant de changer son attribution." }
                        div .buttons {
                            a .button.is-link href={(prefix) "/person/" (staff)} { "Voir la fiche staff" }
                            a .button href={(prefix) (back)} { "Retour" }
                        }
                    }
                }
            },
            html! {},
        );
    }
    let config = serde_json::json!({
        "source": kind, "id": id, "season": context.season,
        "defaults": context.fields,
        "search_url": format!("{prefix}/api/import/staff"),
        "submit_url": format!("{prefix}/api/import/{kind}/{id}"),
        "back_url": format!("{prefix}{back}"),
        "profile_prefix": format!("{prefix}/person/"),
    })
    .to_string();
    let markup = html! {
        div #manual-import-data data-config=(config) {}
        section .section {
            div .container.is-fluid {
                div .level {
                    h1 .title.is-3 { "Importer une adhésion" }
                    a .button.is-light href={(prefix) (back)} { "Retour" }
                }
                div .notification.is-danger role="alert" {
                    strong { "Attention : une mauvaise importation peut corrompre la base de données." }
                    p { "Ne faites cette opération que si vous comprenez à quelle personne appartient l'adhésion et quelles données vous modifiez. En cas de doute, laissez un autre administrateur la traiter." }
                }
                @for warning in &context.warnings {
                    div .notification.is-warning.is-light { (warning) }
                }
                div .box {
                    h2 .title.is-5 { "Détails de l'adhésion à importer" }
                    div .table-container {
                        table .table.is-fullwidth {
                            tbody {
                                @for (label, value) in &context.details {
                                    tr { th { (label) } td { @if value.is_empty() { "—" } @else { (value) } } }
                                }
                                tr { th { "Saison" } td { (context.season) } }
                            }
                        }
                    }
                }
                div .box {
                    button #manual-import-new .button.is-warning type="button" { "C'est un nouvel adhérent" }
                    div .field.mt-5 {
                        label .label for="manual-import-search" { "Rechercher un staff existant" }
                        div .field.has-addons {
                            div .control.is-expanded {
                                input #manual-import-search .input type="search" placeholder="Prénom, nom, email ou téléphone" autocomplete="off";
                            }
                            div .control {
                                button #manual-import-clear .button.is-light type="button" aria-label="Effacer la recherche" title="Effacer et réinitialiser la recherche" disabled { "×" }
                            }
                        }
                        p .help { "Sans recherche, les correspondances de l'adhésion sont affichées. La recherche inclut les noms des anciennes adhésions." }
                    }
                    p #manual-import-search-error .notification.is-danger.is-hidden role="alert" {}
                    div #manual-import-results aria-live="polite" {}
                }
            }
        }
        div #manual-import-modal .modal role="dialog" aria-modal="true" aria-labelledby="manual-import-modal-title" {
            div .modal-background data-close-import="true" {}
            div .modal-card {
                header .modal-card-head {
                    p #manual-import-modal-title .modal-card-title {}
                    button .delete type="button" aria-label="Fermer" data-close-import="true" {}
                }
                section .modal-card-body {
                    p #manual-import-selection .notification.is-info.is-light {}
                    p { "Adhésion de " strong { (context.fields.first_name) " " (context.fields.last_name) } " — saison " (context.season) }
                    p .help.mb-4 { "Pour un staff existant, ses données actuelles sont conservées par défaut. Modifiez uniquement les champs que vous souhaitez corriger." }
                    p .help.mb-4 { "Annuler ensuite l'adhésion ne rétablit pas automatiquement ces champs. Les valeurs avant et après modification sont conservées dans le journal d'audit." }
                    div #manual-import-identity-warning .notification.is-warning.is-hidden {}
                    div #manual-import-error .notification.is-danger.is-hidden role="alert" {}
                    div #manual-import-history-section .box.is-hidden {
                        h3 .title.is-6 { "Adhésions précédentes" }
                        p .help.mb-3 { "Cliquez sur un nom, un email ou un téléphone pour le reprendre dans le formulaire. Les données historiques peuvent aussi contenir des erreurs : vérifiez-les avant de confirmer." }
                        div #manual-import-history {}
                    }
                    form #manual-import-form {
                        div .field {
                            label .label for="manual-import-first" { "Prénom" }
                            input #manual-import-first .input name="first_name" required maxlength="200";
                        }
                        div .field {
                            label .label for="manual-import-last" { "Nom" }
                            input #manual-import-last .input name="last_name" required maxlength="200";
                        }
                        div .field {
                            label .label for="manual-import-email" { "Email" }
                            input #manual-import-email .input name="email" type="email" maxlength="320";
                        }
                        div .field {
                            label .label for="manual-import-phone" { "Téléphone" }
                            input #manual-import-phone .input name="phone" type="tel" maxlength="100";
                        }
                        div .field {
                            label .label for="manual-import-comment" { "Commentaire" }
                            textarea #manual-import-comment .textarea name="comment" rows="3" maxlength="10000" {}
                        }
                        div #manual-import-duplicate .notification.is-warning.is-hidden {
                            p { "Cette personne a déjà une adhésion pour cette saison." }
                            label .checkbox.mt-2 {
                                input #manual-import-allow-duplicate type="checkbox";
                                " Je confirme que cette adhésion supplémentaire doit être conservée."
                            }
                        }
                    }
                }
                footer .modal-card-foot {
                    div .buttons {
                        button #manual-import-confirm .button.is-warning type="submit" form="manual-import-form" { "Confirmer l'importation" }
                        button .button type="button" data-close-import="true" { "Annuler" }
                    }
                }
            }
        }
    };
    page(
        "Import manuel - PowPow",
        prefix,
        &NavKind::Standard,
        "admin",
        html! {},
        markup,
        html! {},
    )
}
