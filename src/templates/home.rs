use super::{NavKind, calendar::render_upcoming_week, page};
use crate::models::{Atelier, Equipment, EquipmentStatus, EquipmentType, Staff};
use maud::{Markup, html};

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn index(
    prefix: &str,
    staff: Option<&Staff>,
    current_season: i16,
    has_paid: bool,
    chief_ateliers: &[Atelier],
    upcoming: &[(chrono::NaiveDate, String, i16, i64)],
    equipments: &[Equipment],
    station_open: bool,
    photo_ids: &[uuid::Uuid],
) -> Markup {
    let p = prefix;
    let season_display = format!("{}-{}", current_season - 1, current_season);

    // Split equipments into slopes and ski tows
    let slopes: Vec<&Equipment> = equipments
        .iter()
        .filter(|e| e.equipment_type == EquipmentType::SkiSlope)
        .collect();
    let tows: Vec<&Equipment> = equipments
        .iter()
        .filter(|e| e.equipment_type == EquipmentType::SkiTow)
        .collect();

    let open_slopes = slopes
        .iter()
        .filter(|e| e.status == EquipmentStatus::Open)
        .count();
    let partial_slopes = slopes
        .iter()
        .filter(|e| e.status == EquipmentStatus::Partial)
        .count();
    let total_slopes = slopes.len();
    let open_tows = tows
        .iter()
        .filter(|e| e.status == EquipmentStatus::Open)
        .count();
    let partial_tows = tows
        .iter()
        .filter(|e| e.status == EquipmentStatus::Partial)
        .count();
    let total_tows = tows.len();

    // Build JSON array of photo IDs for the hero slideshow
    let photo_ids_json = serde_json::to_string(photo_ids).unwrap_or_else(|_| "[]".to_string());

    let extra_head = html! {};

    let content = html! {
        // ── Hero section ─────────────────────────────────────────────
        section #hero .hero-station {
            div .hero-slides data-prefix=(p) data-photos=(photo_ids_json) {}
            div .hero-overlay {
                div .hero-overlay-content {
                    img .hero-logo
                        src="https://station-ski-saint-hilaire.fr/wp-content/uploads/2016/09/logo.png"
                        alt="Logo Station";
                    h1 .hero-title { "Agir pour la station de ski de St Hilaire !" }
                    p .hero-subtitle {
                        "L'association AGH'IL \u{2013} Agir pour la station de ski de St Hilaire \u{2013} regroupe "
                        "les bénévoles qui se mobilisent chaque hiver pour faire vivre notre belle "
                        "petite station de ski."
                    }
                    div .hero-buttons {
                        a .btn-station.btn-station-primary
                          href="https://www.helloasso.com/associations/agir-pour-la-station-de-ski-de-st-hil"
                          target="_blank" {
                            "Réserver votre forfait"
                        }
                        a .btn-station.btn-station-subtle href="#infosplus" {
                            "Infos station"
                        }
                    }
                }
            }
        }

        // ── Infos Station ────────────────────────────────────────────
        section #infosplus .section.section-brown {
            div .container {
                div .columns.is-multiline {
                    // Column 1: Station status
                    div .column.is-4 {
                        h2 .section-heading.has-text-white { "LA STATION EST :" }
                        @if station_open {
                            div .station-status-badge.is-open {
                                span .icon.is-large { i .fa-solid.fa-circle-check.fa-2x {} }
                                span { "OUVERTE" }
                            }
                        } @else {
                            div .station-status-badge.is-closed {
                                span .icon.is-large { i .fa-solid.fa-circle-xmark.fa-2x {} }
                                span { "FERMÉE" }
                            }
                        }
                        p .has-text-white.mt-3.is-size-7 {
                            "Pistes : " (open_slopes) "/" (total_slopes) " ouvertes"
                            @if partial_slopes > 0 {
                                " (" (partial_slopes) " partielles)"
                            }
                            br;
                            "Téléskis : " (open_tows) "/" (total_tows) " ouverts"
                            @if partial_tows > 0 {
                                " (" (partial_tows) " partiels)"
                            }
                        }
                    }

                    // Column 2: Pistes with progress bars
                    div .column.is-4 {
                        h2 .section-heading.has-text-white { "Pistes" }
                        @for slope in &slopes {
                            div .equip-row {
                                @if let Some(diff) = slope.difficulty {
                                    span .difficulty-dot style=(format!("background:{}", diff.css_color())) {}
                                }
                                span .equip-name { (slope.name) }
                                div .equip-bar {
                                    div class=(format!("equip-bar-fill {}", slope.status.css_class()))
                                        data-progress="100" style="width:0%" {}
                                }
                                span .equip-status { (slope.status.label_piste()) }
                            }
                        }
                        // Legend
                        div .equip-legend.mt-4 {
                            span .equip-legend-item {
                                span .equip-legend-dot.is-open {}
                                "Ouvert"
                            }
                            span .equip-legend-item {
                                span .equip-legend-dot.is-partial {}
                                "Partiellement ouvert"
                            }
                            span .equip-legend-item {
                                span .equip-legend-dot.is-closed {}
                                "Fermé"
                            }
                        }
                    }

                    // Column 3: Téléskis + trail map
                    div .column.is-4 {
                        h2 .section-heading.has-text-white { "Téléskis" }
                        @for tow in &tows {
                            div .equip-row {
                                span .equip-name { (tow.name) }
                                div .equip-bar {
                                    div class=(format!("equip-bar-fill {}", tow.status.css_class()))
                                        data-progress="100" style="width:0%" {}
                                }
                                span .equip-status { (tow.status.label_tow()) }
                            }
                        }
                        a .img-modal-trigger href="#"
                          data-src="https://www.station-ski-saint-hilaire.fr/wp-content/uploads/2026/02/Plan-des-pistes-2026_-1030x557.jpg" {
                            img .mt-4 src="https://www.station-ski-saint-hilaire.fr/wp-content/uploads/2026/02/Plan-des-pistes-2026_-1030x557.jpg"
                                alt="Plan des pistes"
                                style="max-width:100%;border-radius:6px;cursor:zoom-in;";
                        }
                        div .mt-3 {
                            a .btn-station.btn-station-primary
                              href="https://www.helloasso.com/associations/agir-pour-la-station-de-ski-de-st-hil"
                              target="_blank" {
                                "Vente en ligne"
                            }
                        }
                    }
                }
            }
        }

        // ── About / Description ──────────────────────────────────────
        section .section.section-navy {
            div .container {
                div .columns {
                    div .column.is-7 {
                        h2 .section-heading.has-text-white {
                            "Le Plateau des Petites Roches et sa station de ski alpin"
                        }
                        div .content.has-text-white {
                            p {
                                "La station de ski de Saint-Hilaire-du-Touvet est située sur le "
                                "Plateau des Petites Roches, dans le massif de la Chartreuse (Isère). "
                                "Nichée entre 1000 et 1300 mètres d'altitude, elle offre un panorama "
                                "exceptionnel sur la vallée du Grésivaudan et la chaîne de Belledonne."
                            }
                            p {
                                "Depuis sa création, la station est animée par des bénévoles passionnés "
                                "qui assurent son fonctionnement : damage des pistes, exploitation des "
                                "remontées mécaniques, accueil du public, vente de forfaits, secours "
                                "sur pistes... Un esprit unique de solidarité et de convivialité !"
                            }
                        }
                    }
                    div .column.is-5 {
                        div .section-golden {
                            h3 .title.is-4.has-text-centered { "La station ouvre en 2026 !" }
                            h4 .subtitle.is-5.has-text-centered { "QUI SOMMES-NOUS ?" }
                            div .content {
                                p {
                                    "L'association AGH'IL (Agir pour la station de ski de St Hilaire) "
                                    "regroupe les bénévoles qui permettent à la station de fonctionner. "
                                    "Rejoignez-nous !"
                                }
                            }
                            div .has-text-centered {
                                a .btn-station.btn-station-primary
                                  href="https://www.helloasso.com/associations/agir-pour-la-station-de-ski-de-st-hil"
                                  target="_blank" {
                                    "Adhérer sur HelloAsso"
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Facebook ─────────────────────────────────────────────────
        section .section.section-navy {
            div .container {
                h2 .section-heading.has-text-centered.has-text-white { "Actualités" }
                div .has-text-centered {
                    iframe src="https://www.facebook.com/plugins/page.php?href=https%3A%2F%2Fwww.facebook.com%2F2185539661710843&tabs=timeline&width=500&height=700&small_header=false&adapt_container_width=true&hide_cover=false&show_facepile=true"
                        width="500" height="700"
                        style="border:none;overflow:hidden;max-width:100%;"
                        scrolling="no" frameborder="0"
                        allow="autoplay; clipboard-write; encrypted-media; picture-in-picture; web-share"
                        allowfullscreen="true" {}
                }
            }
        }

        // ── Webcam ───────────────────────────────────────────────────
        section .section.section-olive {
            div .container {
                h2 .section-heading.has-text-centered.has-text-white {
                    "Webcam de la station de ski de Saint-Hilaire"
                }
                div .has-text-centered {
                    p .has-text-white { "La webcam sera disponible prochainement." }
                }
            }
        }

        // ── Événements ───────────────────────────────────────────────
        section .section.section-brown {
            div .container {
                h2 .section-heading.has-text-centered.has-text-white { "Événements 2026" }
                div .columns.is-centered {
                    div .column.is-8 {
                        div .box {
                            h3 .title.is-4 {
                                span .icon.mr-2 { i .fa-solid.fa-calendar-star {} }
                                "FÊTE de la STATION = le 07 FÉVRIER 2026"
                            }
                            div .content {
                                p {
                                    "Comme chaque année, la fête de la station est un moment "
                                    "convivial et festif ouvert à tous. Au programme : ski, "
                                    "animations, buvette et bonne humeur sur les pistes !"
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Salle hors-sac + Newsletter ──────────────────────────────
        section .section.section-green {
            div .container {
                div .columns {
                    div .column.is-6 {
                        h2 .section-heading.has-text-white { "Une salle, deux utilisations..." }
                        div .content.has-text-white {
                            p {
                                "La station met à disposition une salle hors-sac pour les "
                                "skieurs et les familles. Idéale pour pique-niquer au chaud "
                                "entre deux descentes, elle permet aussi d'organiser des "
                                "événements associatifs."
                            }
                        }
                    }
                    div .column.is-6 {
                        h2 .section-heading.has-text-white { "Newsletter et adhésion" }
                        div .content.has-text-white {
                            p {
                                "Pour rester informé de l'actualité de la station et de "
                                "l'association, adhérez via HelloAsso. Vous recevrez toutes "
                                "les informations sur les ouvertures et événements."
                            }
                            a .btn-station.btn-station-primary
                              href="https://www.helloasso.com/associations/agir-pour-la-station-de-ski-de-st-hil"
                              target="_blank" {
                                "Adhérer sur HelloAsso"
                            }
                        }
                    }
                }
            }
        }

        // ── Fullscreen image modal ────────────────────────────────────
        div #img-modal .img-modal {
            img .img-modal-content src="" alt="";
        }

        // ── Logged-in user sections ──────────────────────────────────
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
                    a .box.box-link href={(p) "/person/" (staff.id)} {
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

        // Upcoming week (visible to everyone)
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
        "Station de ski de Saint-Hilaire du Touvet",
        prefix,
        &NavKind::LoginOnly,
        "",
        extra_head,
        content,
        html! {},
    )
}
