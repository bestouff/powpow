use chrono::Local;
use maud::{Markup, html};

use super::{NavKind, page};
use crate::{
    models::MeteoReading,
    webcam::{Gallery, previews},
};

pub fn webcam_page(
    prefix: &str,
    title: &str,
    gallery: &Gallery,
    follow_latest: bool,
    weather: Option<&MeteoReading>,
    dimensions: Option<(u32, u32)>,
    logo: Option<uuid::Uuid>,
) -> Markup {
    let selected_name = gallery
        .selected
        .as_ref()
        .map_or("", |frame| frame.filename.as_str());
    let content = html! {
        section .section #webcam-page data-follow-latest=(follow_latest) data-image=(selected_name)
          data-camera=(gallery.camera.as_deref().unwrap_or_default())
          data-weather-revision=(weather.map(MeteoReading::revision).unwrap_or_default()) {
            div .container {
                div .level {
                    div .level-left {
                        div {
                            p .heading { "Webcam" }
                            h1 .title { (title) }
                        }
                    }
                }
                div .columns.is-variable.is-5 {
                    div .column.is-three-quarters {
                        @if let Some(frame) = &gallery.selected {
                            @let received_at = frame.received_at.with_timezone(&Local);
                            figure .webcam-view {
                                img .webcam-photo src={(prefix) "/webcam/images/" (frame.filename)}
                                  width=[dimensions.map(|(width, _)| width)]
                                  height=[dimensions.map(|(_, height)| height)]
                                  alt={"Vue de la webcam : " (title)};
                                @if let Some(id) = logo {
                                    img .webcam-logo src={(prefix) "/content-images/" (id)} alt="Logo de la station";
                                }
                                button .button.is-dark.webcam-fullscreen.is-hidden type="button"
                                  title="Plein écran" aria-label="Plein écran" {
                                    span .icon { i .fa-solid.fa-expand aria-hidden="true" {} }
                                    span .webcam-fullscreen-label { "Plein écran" }
                                }
                                figcaption .webcam-caption {
                                    div .webcam-caption-heading {
                                        strong { (title) }
                                        time datetime=(received_at.to_rfc3339()) {
                                            (received_at.format("%d/%m/%Y à %H:%M:%S"))
                                        }
                                    }
                                    @if let Some(reading) = weather {
                                        (weather_overlay(reading))
                                    }
                                }
                            }
                            div .webcam-timeline.mt-4 data-list={(prefix) "/webcam/" (previews::list_name(gallery.date, &frame.username))} {
                                label .label for="webcam-slider" { "Parcourir la journée" }
                                input #webcam-slider type="range" min="0" max=(gallery.images.len().saturating_sub(1))
                                  value=(gallery.images.iter().position(|image| image.filename == frame.filename).unwrap_or_default())
                                  step="1" aria-valuetext=(frame.local_label()) aria-describedby="webcam-slider-help" disabled;
                                output #webcam-slider-time for="webcam-slider" { (frame.local_label()) }
                                p .help #webcam-slider-help { "Faites glisser pour prévisualiser, puis relâchez pour charger l'image complète." }
                                p .help #webcam-slider-status role="status" {}
                                noscript { p .help { "Activez JavaScript pour parcourir les images avec le curseur." } }
                            }
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
                            h2 .title.is-5 { "Choisir une image" }
                            nav .webcam-shortcuts.mb-5 aria-label="Images récentes" {
                                a .button.is-link.is-light href={(prefix) "/webcam" @if let Some(camera) = &gallery.camera { "?camera=" (camera) }} { "Actuelle" }
                                @for (hours, frame) in [1, 3, 5].into_iter().zip(&gallery.recent) {
                                    @if let Some(frame) = frame {
                                        a .button href={(prefix) "/webcam?image=" (frame.filename)} title=(frame.local_label()) { "Il y a " (hours) " h" }
                                    } @else {
                                        button .button disabled title="Aucune image à moins de 30 minutes avant ou après cette heure." { "Il y a " (hours) " h" }
                                    }
                                }
                            }
                            form method="get" action={(prefix) "/webcam"} {
                                @if let Some(camera) = &gallery.camera {
                                    input type="hidden" name="camera" value=(camera);
                                }
                                div .field {
                                    label .label for="webcam-date" { "Date" }
                                    div .control {
                                        input .input #webcam-date type="date" name="date" value=(gallery.date) required;
                                    }
                                }
                                button .button.is-link.is-fullwidth type="submit" { "Afficher" }
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
    let observed_at = reading.observed_at.with_timezone(&Local);
    let measurements = [
        ("Température", reading.temperature_c, "°C", None),
        ("Pluie (1 h)", reading.rainfall_last_hour_mm, "mm", None),
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
                time datetime=(observed_at.to_rfc3339()) {
                    (observed_at.format("%d/%m/%Y à %H:%M"))
                }
            }
        }
    }
}
