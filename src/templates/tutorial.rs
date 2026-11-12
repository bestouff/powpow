use maud::{Markup, html};

pub(super) fn guide() -> Markup {
    html! {
        div #volunteer-tutorial hidden {
            svg .tutorial-drawing aria-hidden="true" focusable="false" {
                path .tutorial-circle {}
                path .tutorial-circle-echo {}
                path .tutorial-arrow {}
                path .tutorial-arrowhead {}
            }
            aside .tutorial-card role="region" aria-labelledby="tutorial-title" {
                button .tutorial-close type="button" aria-label="Fermer le guide" { "×" }
                p .tutorial-progress {}
                div aria-live="polite" aria-atomic="true" {
                    h2 #tutorial-title {}
                    p #tutorial-description {}
                }
                div .tutorial-actions {
                    div .tutorial-navigation hidden {
                        button #tutorial-previous .button.is-light type="button" { "Précédent" }
                        button #tutorial-next .button.is-danger type="button" { "Suivant" }
                    }
                    button #tutorial-missing .button.is-danger.is-outlined type="button" hidden {
                        "Je ne trouve pas mon nom"
                    }
                    button #tutorial-back .button.is-light type="button" hidden {
                        "Revenir à la recherche"
                    }
                    button #tutorial-dismiss type="button" { "Fermer le guide" }
                }
            }
        }
    }
}
