use chrono::NaiveDate;
use maud::{Markup, html};

use super::{NavKind, page};
use crate::models::{AtelierDayPlanning, OpeningDayStatus};

pub fn today_page(
    prefix: &str,
    today: NaiveDate,
    planning: &[AtelierDayPlanning],
    opening_status: Option<OpeningDayStatus>,
) -> Markup {
    let content = html! {
        section .section #today-page {
            div .container {
                div .level {
                    div .level-left {
                        div {
                            h1 .title { "Infos du jour" }
                            p .subtitle {
                                time datetime=(today) { (today.format("%d/%m/%Y")) }
                            }
                        }
                    }
                    div .level-right {
                        a .button.is-link.is-light href={(prefix) "/today"} {
                            span .icon { i .fa-solid.fa-arrows-rotate {} }
                            span { "Actualiser" }
                        }
                    }
                }
                @if let Some(status) = opening_status {
                    @let (class, label) = match status {
                        OpeningDayStatus::Reserved => ("is-warning", "Ouverture prévue, à confirmer."),
                        OpeningDayStatus::Validated => ("is-success", "Ouverture confirmée."),
                        OpeningDayStatus::Canceled => ("is-danger", "Journée d'ouverture annulée."),
                    };
                    div class={"notification is-light " (class)} { (label) }
                }
                h2 .title.is-4 { "Planning du jour" }
                @if planning.is_empty() {
                    div .notification.is-light { "Aucun atelier prévu aujourd'hui." }
                } @else {
                    div .columns.is-multiline {
                        @for atelier in planning {
                            div .column.is-half-desktop {
                                article .box {
                                    h3 .title.is-5 {
                                        a href={(prefix) "/calendar/" (atelier.atelier_slug)} {
                                            span .icon.mr-1 { i class={"fa-solid fa-" (atelier.atelier_icon)} {} }
                                            (atelier.atelier_name)
                                        }
                                    }
                                    div .columns {
                                        (shift(atelier, true, prefix))
                                        (shift(atelier, false, prefix))
                                    }
                                }
                            }
                        }
                    }
                }
                a .button.is-link.is-outlined href={(prefix) "/calendar"} {
                    span .icon { i .fa-solid.fa-calendar-days {} }
                    span { "Voir le planning complet" }
                }
            }
        }
    };
    page(
        "Infos du jour - PowPow",
        prefix,
        &NavKind::Standard,
        "today",
        html! {},
        content,
        html! {},
    )
}

fn shift(atelier: &AtelierDayPlanning, first_half: bool, prefix: &str) -> Markup {
    let label = match (atelier.nightly, first_half) {
        (false, true) => "Matin",
        (false, false) => "Après-midi",
        (true, true) => "Soirée",
        (true, false) => "Nuit",
    };
    let staff: Vec<_> = atelier
        .staff
        .iter()
        .filter(|staff| {
            if first_half {
                staff.first_half
            } else {
                staff.second_half
            }
        })
        .collect();
    html! {
        section .column {
            h4 .title.is-6 { (label) }
            @if staff.is_empty() {
                p .has-text-grey { "Aucun bénévole inscrit." }
            } @else {
                ul {
                    @for member in staff {
                        li .mb-2 {
                            a href={(prefix) "/person/" (member.id)} { (member.first_name) " " (member.last_name) }
                        }
                    }
                }
            }
        }
    }
}
