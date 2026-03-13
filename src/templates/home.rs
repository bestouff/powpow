use super::{NavKind, page_with_footer};
use crate::models::{ContentMap, Equipment, EquipmentStatus, EquipmentType, NewsRow};
use chrono::Datelike;
use maud::{Markup, PreEscaped, html};

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn index(
    prefix: &str,
    equipments: &[Equipment],
    station_open: bool,
    photo_ids: &[uuid::Uuid],
    staff_photo_ids: &[uuid::Uuid],
    contents: &ContentMap,
    dicton: Option<&str>,
    news_items: &[NewsRow],
) -> Markup {
    let p = prefix;

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
                    @let block = contents.get("hero-subtitle");
                    @if let Some(img_id) = block.image_id {
                        img .hero-logo
                            src=(format!("{p}/content-images/{img_id}"))
                            alt="Logo Station";
                    }
                    h1 .hero-title { (block.title) }
                    @let body_html = block.render_body();
                    @if !body_html.is_empty() {
                        div .hero-subtitle { (PreEscaped(body_html)) }
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
                        // Editable infos-station block (below status)
                        @let block = contents.get("infos-station");
                        @let body_html = block.render_body();
                        @if !body_html.is_empty() {
                            div .content.has-text-white.mt-4 {
                                (PreEscaped(body_html))
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
                        // Dicton du jour
                        @if let Some(dicton_text) = dicton {
                            div .dicton-du-jour.mt-4 {
                                h3 .dicton-title {
                                    span .icon.mr-1 { i .fa-solid.fa-feather-pointed {} }
                                    "Dicton du jour"
                                }
                                div .dicton-text { (PreEscaped(dicton_text)) }
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
                        @let trail = contents.get("trail-map");
                        @if let Some(img_id) = trail.image_id {
                            @let img_url = format!("{p}/content-images/{img_id}");
                            a .img-modal-trigger href="#"
                              data-src=(img_url) {
                                img .mt-4 src=(img_url)
                                    alt=(trail.title)
                                    style="max-width:100%;border-radius:6px;cursor:zoom-in;";
                            }
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
                        @let block = contents.get("about-station");
                        h2 .section-heading.has-text-white { (block.title) }
                        div .content.has-text-white {
                            (PreEscaped(block.render_body()))
                        }
                    }
                    div .column.is-5 {
                        div .section-golden {
                            @let block = contents.get("about-association");
                            h3 .title.is-4.has-text-centered { (block.title) }
                            div .content {
                                (PreEscaped(block.render_body()))
                            }
                            @if let Some(img_id) = block.image_id {
                                img src=(format!("{p}/content-images/{img_id}"))
                                    alt=(block.title)
                                    style="max-width:100%;border-radius:6px;";
                            }
                            @if let Some(ref url) = block.link_url {
                                @let label = block.link_label.as_deref().unwrap_or(url);
                                div .has-text-centered {
                                    a .btn-station.btn-station-primary href=(url) target="_blank" {
                                        (label)
                                    }
                                }
                            }
                            @if !staff_photo_ids.is_empty() {
                                @let staff_ids_json = serde_json::to_string(staff_photo_ids).unwrap_or_else(|_| "[]".to_string());
                                div .staff-carousel data-prefix=(p) data-photos=(staff_ids_json) {
                                    div .staff-carousel-track {}
                                    button .staff-carousel-btn.staff-prev type="button" { "❮" }
                                    button .staff-carousel-btn.staff-next type="button" { "❯" }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Actualités (RSS) ───────────────────────────────────────────
        @if !news_items.is_empty() {
            section .section.section-navy {
                div .container {
                    h2 .section-heading.has-text-centered.has-text-white { "Actualités" }
                    div .news-grid {
                        @for item in news_items {
                            a .news-card href=(item.link) target="_blank" rel="noopener" {
                                @if item.has_image {
                                    img .news-card-img
                                        src=(format!("{p}/news-images/{}", item.id))
                                        alt="" loading="lazy";
                                }
                                @if let Some(dt) = item.pub_date {
                                    span .news-date {
                                        (format_date_fr_short(dt))
                                    }
                                }
                                p .news-text { (item.text) }
                            }
                        }
                    }
                }
            }
        }

        // ── Événements ───────────────────────────────────────────────
        section .section.section-brown {
            div .container {
                @let block = contents.get("events");
                h2 .section-heading.has-text-centered.has-text-white { (block.title) }
                div .columns.is-centered {
                    div .column.is-8 {
                        div .box {
                            div .content {
                                (PreEscaped(block.render_body()))
                            }
                            @if let Some(img_id) = block.image_id {
                                img src=(format!("{p}/content-images/{img_id}"))
                                    alt=(block.title)
                                    style="max-width:100%;border-radius:6px;";
                            }
                            @if let Some(ref url) = block.link_url {
                                @let label = block.link_label.as_deref().unwrap_or(url);
                                a .btn-station.btn-station-primary href=(url) target="_blank" {
                                    (label)
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
                        @let block = contents.get("salle-hors-sac");
                        h2 .section-heading.has-text-white { (block.title) }
                        div .content.has-text-white {
                            (PreEscaped(block.render_body()))
                        }
                        @if let Some(img_id) = block.image_id {
                            img src=(format!("{p}/content-images/{img_id}"))
                                alt=(block.title)
                                style="max-width:100%;border-radius:6px;";
                        }
                    }
                    div .column.is-6 {
                        @let block = contents.get("newsletter");
                        h2 .section-heading.has-text-white { (block.title) }
                        div .content.has-text-white {
                            (PreEscaped(block.render_body()))
                            @if let Some(ref url) = block.link_url {
                                @let label = block.link_label.as_deref().unwrap_or(url);
                                a .btn-station.btn-station-primary href=(url) target="_blank" {
                                    (label)
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Accès / Driving indications (olive) ─────────────────────
        section .section.section-olive {
            div .container {
                @let block = contents.get("driving-indications");
                h2 .section-heading.has-text-centered.has-text-white { (block.title) }
                div .columns.is-centered {
                    div .column.is-8 {
                        @if let Some(img_id) = block.image_id {
                            div .has-text-centered.mb-4 {
                                img src=(format!("{p}/content-images/{img_id}"))
                                    alt=(block.title)
                                    style="max-width:100%;border-radius:6px;";
                            }
                        }
                        div .content.has-text-white {
                            (PreEscaped(block.render_body()))
                        }
                        @if let Some(ref url) = block.link_url {
                            @let label = block.link_label.as_deref().unwrap_or(url);
                            div .has-text-centered.mt-4 {
                                a .btn-station.btn-station-primary href=(url) target="_blank" {
                                    (label)
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Tarifs (pricing) ─────────────────────────────────────────
        section .section.section-teal {
            div .container {
                @let block = contents.get("pricing");
                h2 .section-heading.has-text-centered.has-text-white { (block.title) }
                div .columns.is-centered {
                    div .column.is-8 {
                        div .content.has-text-white {
                            (PreEscaped(block.render_body()))
                        }
                        @if let Some(img_id) = block.image_id {
                            div .has-text-centered.mb-4 {
                                img src=(format!("{p}/content-images/{img_id}"))
                                    alt=(block.title)
                                    style="max-width:100%;border-radius:6px;";
                            }
                        }
                        @if let Some(ref url) = block.link_url {
                            @let label = block.link_label.as_deref().unwrap_or(url);
                            div .has-text-centered.mt-4 {
                                a .btn-station.btn-station-primary href=(url) target="_blank" {
                                    (label)
                                }
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
    };

    page_with_footer(
        "Station de ski de Saint-Hilaire du Touvet",
        prefix,
        &NavKind::LoginOnly,
        "",
        extra_head,
        content,
        html! {},
        Some(contents),
    )
}

/// Format a `DateTime<Utc>` as a short French date (e.g. "15 janvier 2026").
fn format_date_fr_short(dt: chrono::DateTime<chrono::Utc>) -> String {
    let d = dt.date_naive();
    let month = match d.month() {
        1 => "janvier",
        2 => "février",
        3 => "mars",
        4 => "avril",
        5 => "mai",
        6 => "juin",
        7 => "juillet",
        8 => "août",
        9 => "septembre",
        10 => "octobre",
        11 => "novembre",
        12 => "décembre",
        _ => "???",
    };
    format!("{} {} {}", d.day(), month, d.year())
}
