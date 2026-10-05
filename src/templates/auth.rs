use super::{NavKind, page};
use crate::models::ContentBlock;
use maud::{Markup, PreEscaped, html};

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

pub fn login_page(prefix: &str, about_block: Option<&ContentBlock>) -> Markup {
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
                        @if let Some(block) = about_block {
                            @if !block.body.is_empty() {
                                div .section-about-association.mt-5 {
                                    h3 .title.is-4.has-text-centered { (block.title) }
                                    div .content {
                                        (PreEscaped(block.render_body()))
                                    }
                                    @if let Some(ref url) = block.link_url {
                                        @let label = block.link_label.as_deref().unwrap_or(url);
                                        div .has-text-centered {
                                            a .btn-station.btn-station-call-to-action href=(url) target="_blank" {
                                                (label)
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
        "Connexion - PowPow",
        prefix,
        &NavKind::LoginOnly,
        "",
        html! {},
        content,
        extra_scripts,
    )
}
