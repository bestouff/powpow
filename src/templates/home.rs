use super::{NavKind, calendar::render_upcoming_week, page};
use crate::models::{Atelier, Staff};
use maud::{Markup, html};

pub fn index(
    prefix: &str,
    staff: Option<&Staff>,
    current_season: i16,
    has_paid: bool,
    chief_ateliers: &[Atelier],
    upcoming: &[(chrono::NaiveDate, String, i16, i64)],
) -> Markup {
    let p = prefix;
    let season_display = format!("{}-{}", current_season - 1, current_season);

    let extra_head = html! {};

    let content = html! {
        // Hero
        section .hero.is-info {
            div .hero-body {
                div .container.has-text-centered {
                    h1 .title.is-2.mb-4 {
                        span .icon.is-large { i .fa-solid.fa-person-skiing.fa-2x {} }
                        br;
                        "Gestionnaire de plannings bénévoles"
                    }
                    h2 .subtitle.is-5 {
                        "PowPow (Pistes, Organisation, Week-end, Planning, Optimisation, Wouah!) pour AG'HIL"
                    }
                }
            }
        }

        @if let Some(staff) = staff {
            // Membership status
            section .section.py-4 {
                div .container.is-fluid {
                    @if has_paid {
                        div .notification.is-success.is-light {
                            span .icon { i .fa-solid.fa-circle-check {} }
                            " Ta cotisation est à jour pour la saison " (season_display) "."
                        }
                    } @else {
                        div .notification.is-warning.is-light {
                            span .icon { i .fa-solid.fa-triangle-exclamation {} }
                            " Ta cotisation n'est pas à jour pour la saison " (season_display)
                            " \u{2014} "
                            a href="https://www.helloasso.com/associations/agir-pour-la-station-de-ski-de-st-hil"
                              target="_blank" { "inscris-toi sur HelloAsso" }
                            "."
                        }
                    }
                }
            }

            // My profile
            section .section.py-4 {
                div .container.is-fluid {
                    a .box href={(p) "/person/" (staff.id)} {
                        span .icon.mr-2 { i .fa-solid.fa-user-gear {} }
                        strong { "Gérer mes ateliers et mes préférences" }
                    }
                }
            }

            // Chief ateliers
            @if !chief_ateliers.is_empty() {
                section .section.py-4 {
                    div .container.is-fluid {
                        div .box {
                            h3 .title.is-5.mb-3 {
                                span .icon.mr-2 { i .fa-solid.fa-user-shield {} }
                                "Mes ateliers"
                            }
                            div .buttons {
                                @for a in chief_ateliers {
                                    a .button.is-link.is-light.mr-2.mb-2
                                      href={(p) "/calendar/" (a.slug)} {
                                        span .icon { i class={"fa-solid fa-" (a.icon)} {} }
                                        "\u{00a0}"
                                        span { (a.name) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Upcoming week
        section .section.py-4 {
            div .container.is-fluid {
                a .box.box-link href={(p) "/calendar"} {
                    h3 .title.is-5.mb-3 {
                        span .icon.mr-2 { i .fa-solid.fa-calendar-week {} }
                        "Semaine à venir"
                    }
                    (render_upcoming_week(upcoming))
                }
            }
        }

        @if let Some(staff) = staff {
            // Admin / chief link
            @if staff.is_admin || !chief_ateliers.is_empty() {
                section .section.py-4 {
                    div .container.is-fluid {
                        a .box.box-link href={(p) "/admin"} {
                            span .icon.mr-2 { i .fa-solid.fa-screwdriver-wrench {} }
                            strong { "Administration" }
                        }
                    }
                }
            }
        }
    };

    page(
        "PowPow for AGH'IL",
        prefix,
        &NavKind::LoginOnly,
        "",
        extra_head,
        content,
        html! {},
    )
}
