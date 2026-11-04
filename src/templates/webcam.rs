use maud::{Markup, html};

use super::{NavKind, get_navbar_block, page};
use crate::{models::MeteoReading, webcam::Gallery};

pub fn webcam_page(
    prefix: &str,
    title: &str,
    gallery: &Gallery,
    follow_latest: bool,
    weather: Option<&MeteoReading>,
) -> Markup {
    let logo = get_navbar_block().and_then(|block| block.image_id);
    let selected_name = gallery
        .selected
        .as_ref()
        .map_or("", |frame| frame.filename.as_str());
    let content = html! {
        section .section #webcam-page data-follow-latest=(follow_latest) data-image=(selected_name)
          data-weather-revision=(weather.map(MeteoReading::revision).unwrap_or_default()) {
            div .container {
                div .level {
                    div .level-left {
                        div {
                            p .heading { "Webcam" }
                            h1 .title { (title) }
                        }
                    }
                    div .level-right {
                        a .button.is-link.is-light href={(prefix) "/webcam"} {
                            span .icon { i .fa-solid.fa-clock-rotate-left {} }
                            span { "Dernière image" }
                        }
                    }
                }
                div .columns.is-variable.is-5 {
                    div .column.is-three-quarters {
                        @if let Some(frame) = &gallery.selected {
                            figure .webcam-view {
                                img .webcam-photo src={(prefix) "/webcam/images/" (frame.filename)} alt={"Vue de la webcam : " (title)};
                                @if let Some(id) = logo {
                                    img .webcam-logo src={(prefix) "/content-images/" (id)} alt="Logo de la station";
                                }
                                figcaption .webcam-caption {
                                    div .webcam-caption-heading {
                                        strong { (title) }
                                        time datetime=(frame.received_at.to_rfc3339()) {
                                            (frame.received_at.format("%d/%m/%Y à %H:%M:%S UTC"))
                                        }
                                    }
                                    @if let Some(reading) = weather {
                                        (weather_overlay(reading))
                                    }
                                }
                            }
                            nav .webcam-navigation.mt-4 aria-label="Parcourir les images" {
                                @if let Some(previous) = &gallery.previous {
                                    a .button href={(prefix) "/webcam?image=" (previous.filename)} {
                                        span .icon { i .fa-solid.fa-chevron-left {} }
                                        span { "Précédente" }
                                    }
                                } @else {
                                    button .button disabled { "Précédente" }
                                }
                                @if let Some(next) = &gallery.next {
                                    a .button href={(prefix) "/webcam?image=" (next.filename)} {
                                        span { "Suivante" }
                                        span .icon { i .fa-solid.fa-chevron-right {} }
                                    }
                                } @else {
                                    button .button disabled { "Suivante" }
                                }
                            }
                            p .help.mt-3 { "Heure de réception sur le serveur (UTC)." }
                        } @else {
                            div .notification.is-light {
                                "Aucune image disponible pour cette date."
                            }
                        }
                        @if follow_latest {
                            p .help { "Cette page se met à jour automatiquement à l'arrivée d'une nouvelle image ou d'un nouveau relevé météo." }
                        }
                    }
                    aside .column {
                        div .box.webcam-history {
                            h2 .title.is-5 { "Images précédentes" }
                            form method="get" action={(prefix) "/webcam"} {
                                div .field {
                                    label .label for="webcam-date" { "Date (UTC)" }
                                    div .control {
                                        input .input #webcam-date type="date" name="date" value=(gallery.date) required;
                                    }
                                }
                                button .button.is-link.is-fullwidth type="submit" { "Afficher" }
                            }
                            @if !gallery.images.is_empty() {
                                div .webcam-times.mt-4 aria-label="Choisir une image" {
                                    @for frame in &gallery.images {
                                        a .webcam-time .is-selected[frame.filename == selected_name]
                                          href={(prefix) "/webcam?image=" (frame.filename)}
                                          aria-current=[(frame.filename == selected_name).then_some("true")]
                                          title=(frame.username) {
                                            (frame.received_at.format("%H:%M:%S"))
                                        }
                                    }
                                }
                                @if gallery.pages > 1 {
                                    nav .webcam-pagination.mt-4 aria-label="Pages d'archives" {
                                        @if gallery.page > 0 {
                                            a .button.is-small href={(prefix) "/webcam?date=" (gallery.date) "&page=" (gallery.page - 1)} { "Plus récentes" }
                                        }
                                        span .is-size-7 { (gallery.page + 1) " / " (gallery.pages) }
                                        @if gallery.page + 1 < gallery.pages {
                                            a .button.is-small href={(prefix) "/webcam?date=" (gallery.date) "&page=" (gallery.page + 1)} { "Plus anciennes" }
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
        title,
        prefix,
        &NavKind::LoginOnly,
        "webcam",
        html! {},
        content,
        html! {},
    )
}

fn weather_overlay(reading: &MeteoReading) -> Markup {
    let measurements = [
        ("Température", reading.temperature_c, "°C", None),
        ("Pluie (6 min)", reading.rainfall_mm, "mm", None),
        (
            "Vent",
            reading.wind_speed_m_s.map(|speed| speed * 3.6),
            "km/h",
            reading.wind_direction_deg,
        ),
        (
            "Rafales",
            reading.gust_speed_m_s.map(|speed| speed * 3.6),
            "km/h",
            reading.gust_direction_deg,
        ),
        (
            "Humidité",
            reading.humidity_percent.map(f64::from),
            "%",
            None,
        ),
        (
            "Neige",
            reading.snow_depth_m.map(|depth| depth * 100.0),
            "cm",
            None,
        ),
        (
            "Pression",
            reading.pressure_pa.map(|pressure| pressure / 100.0),
            "hPa",
            None,
        ),
        (
            "Pression mer",
            reading
                .sea_level_pressure_pa
                .map(|pressure| pressure / 100.0),
            "hPa",
            None,
        ),
        ("Point de rosée", reading.dew_point_c, "°C", None),
    ];
    html! {
        div .webcam-weather aria-label="Observations météo" {
            div .webcam-weather-values {
                @for (label, value, unit, direction) in measurements {
                    @if let Some(value) = value {
                        span .webcam-weather-measurement {
                            (label) " " strong { (format!("{value:.1}").replace('.', ",")) " " (unit) }
                            @if let Some(direction) = direction { " · " (direction) "°" }
                        }
                    }
                }
            }
            div .webcam-weather-source {
                "Météo-France · relevé du "
                time datetime=(reading.observed_at.to_rfc3339()) {
                    (reading.observed_at.format("%d/%m/%Y à %H:%M UTC"))
                }
            }
        }
    }
}
