use super::{NavKind, page};
use crate::models::ContentBlock;
use maud::{Markup, PreEscaped, html};

pub fn email_change_page(prefix: &str, title: &str, message: &str, token: Option<&str>) -> Markup {
    let content = html! {
        section .section {
            div .container {
                div .columns.is-centered {
                    div .column.is-5 {
                        div .box {
                            h1 .title.is-4 { (title) }
                            p .mb-4 { (message) }
                            @if let Some(token) = token {
                                form method="POST" action={(prefix) "/email-change"} {
                                    input type="hidden" name="token" value=(token);
                                    button .button.is-primary type="submit" { "Confirmer mon adresse email" }
                                }
                                p .help.mt-3 { "Votre adresse actuelle reste active jusqu'à cette confirmation." }
                            } @else {
                                a .button.is-primary href={(prefix) "/login"} { "Se connecter" }
                            }
                        }
                    }
                }
            }
        }
    };
    page(
        title,
        prefix,
        &NavKind::LoginOnly,
        "",
        html! {},
        content,
        html! {},
    )
}

pub fn login_confirmation(staff: &crate::models::Staff, token: uuid::Uuid, prefix: &str) -> Markup {
    let content = html! {
        section .section {
            div .container {
                div .columns.is-centered {
                    div .column.is-5 {
                        div .box {
                            h1 .title.is-4 { "Confirmer la connexion" }
                            p .mb-4 { "Vous allez vous connecter en tant que " strong { (staff.first_name) " " (staff.last_name) } "." }
                            form method="POST" action={(prefix) "/login/confirm"} {
                                input type="hidden" name="staff_id" value=(staff.id);
                                input type="hidden" name="token" value=(token);
                                button .button.is-primary.is-fullwidth type="submit" { "Se connecter" }
                            }
                            p .help.mt-3 { "Cette confirmation évite qu'un aperçu de lien, par exemple dans WhatsApp, utilise votre lien de connexion à votre place." }
                        }
                    }
                }
            }
        }
    };
    page(
        "Confirmer la connexion - PowPow",
        prefix,
        &NavKind::LoginOnly,
        "",
        html! {},
        content,
        html! {},
    )
}

pub fn invalid_login_link(prefix: &str) -> Markup {
    let content = html! {
        section .section {
            div .container {
                div .columns.is-centered {
                    div .column.is-5 {
                        div .box {
                            h1 .title.is-4 { "Lien de connexion invalide" }
                            p .mb-4 { "Ce lien a expiré ou a déjà été utilisé. Demandez un nouveau lien de connexion." }
                            a .button.is-primary href={(prefix) "/login"} { "Demander un nouveau lien" }
                        }
                    }
                }
            }
        }
    };
    page(
        "Lien de connexion invalide - PowPow",
        prefix,
        &NavKind::LoginOnly,
        "",
        html! {},
        content,
        html! {},
    )
}

pub fn login_page(
    prefix: &str,
    about_block: Option<&ContentBlock>,
    association_slug: &str,
    entity_name: &str,
) -> Markup {
    let membership_url = about_block
        .and_then(|block| block.link_url.as_deref())
        .and_then(|url| reqwest::Url::parse(url).ok())
        .filter(|url| {
            url.scheme() == "https"
                && url
                    .host_str()
                    .is_some_and(|host| host == "helloasso.com" || host.ends_with(".helloasso.com"))
        })
        .unwrap_or_else(|| super::helloasso_url(association_slug));
    let membership_label = about_block
        .filter(|block| block.link_url.as_deref() == Some(membership_url.as_str()))
        .and_then(|block| block.link_label.as_deref())
        .filter(|label| !label.is_empty())
        .unwrap_or("Adhérer sur HelloAsso");
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
                                div #login-search {
                                    div .field {
                                        label .label for="search-input" { "Rechercher votre nom" }
                                        div .control.has-icons-left {
                                            input .input type="text" #search-input
                                                placeholder="Tapez au moins 4 caractères..."
                                                autocomplete="off" aria-describedby="login-search-help" aria-controls="results-panel";
                                            span .icon.is-left { i .fa-solid.fa-magnifying-glass {} }
                                        }
                                        p .help #login-search-help { "Entrez votre prénom ou nom de famille" }
                                    }
                                    nav .panel.d-none #results-panel aria-label="Résultats de la recherche" aria-live="polite" {}
                                }
                                div #confirm-box .d-none.notification.is-info.is-light.mt-4 {
                                    p #confirm-text {}
                                    button .button.is-primary.mt-3 #send-btn {
                                        span .icon { i .fa-solid.fa-envelope {} }
                                        span { "Envoyer le lien de connexion" }
                                    }
                                }
                                div #success-box .d-none.notification.is-success.is-light.mt-4
                                  data-entity-name=(entity_name) role="status" tabindex="-1" {
                                    p {
                                        span .icon { i .fa-solid.fa-check {} }
                                        " Un email de connexion"
                                        @if !entity_name.trim().is_empty() {
                                            " de « " (entity_name) " »"
                                        }
                                        " a été envoyé. Ouvrez votre boîte de réception et cliquez sur le lien dans cet email."
                                    }
                                }
                                div #error-box .d-none.notification.is-danger.is-light.mt-4 {
                                    p #error-text {}
                                }
                            }
                        }
                        div .section-about-association.mt-5 {
                            @if let Some(block) = about_block {
                                @if !block.body.is_empty() {
                                    h3 .title.is-4.has-text-centered { (block.title) }
                                    div .content {
                                        (PreEscaped(block.render_body()))
                                    }
                                }
                            }
                            div .has-text-centered {
                                a #membership-link .btn-station.btn-station-call-to-action href=(membership_url.as_str()) target="_blank" rel="noopener noreferrer" {
                                    (membership_label)
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
        "Connexion - PowPow",
        prefix,
        &NavKind::LoginOnly,
        "",
        html! {},
        content,
        extra_scripts,
    )
}
