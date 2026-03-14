use super::{NavKind, page};
use crate::models::ContentBlock;
use maud::{Markup, PreEscaped, html};

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
                                div .section-golden.mt-5 {
                                    h3 .title.is-4.has-text-centered { (block.title) }
                                    div .content {
                                        (PreEscaped(block.render_body()))
                                    }
                                    @if let Some(ref url) = block.link_url {
                                        @let label = block.link_label.as_deref().unwrap_or(url);
                                        div .has-text-centered {
                                            a .btn-station.btn-station-primary href=(url) target="_blank" {
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
