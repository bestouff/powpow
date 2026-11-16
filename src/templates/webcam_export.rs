use chrono::{Local, NaiveTime};
use maud::{Markup, html};

use super::{NavKind, page};
use crate::webcam::{Gallery, export::MAX_SPEED, previews};

pub fn webcam_export_page(
    prefix: &str,
    gallery: &Gallery,
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
    speed: u32,
    warning: Option<&str>,
) -> Markup {
    let content = html! {
        section .section {
            div .container {
                h1 .title { "Exporter une vidéo de la webcam" }
                p .subtitle { "Un accéléré de la journée, à télécharger au format MP4 (H.264)." }
                a .button.is-light.mb-5 href={(prefix) "/webcam?date=" (gallery.date) @if let Some(camera) = &gallery.camera { "&camera=" (camera) }} { "Retour à la webcam" }
                @if let Some(warning) = warning {
                    div .notification.is-warning role="status" { (warning) }
                }
                @if gallery.images.is_empty() {
                    div .notification.is-info { "Aucune image disponible pour cette journée et cette caméra." }
                }
                form #webcam-export-form method="get" action={(prefix) "/webcam/export/video"} {
                    @if let Some(camera) = &gallery.camera {
                        input type="hidden" name="camera" value=(camera);
                        p .mb-4 { "Caméra : " strong { (camera) } " · " (gallery.images.len()) " images dans la journée" }
                    }
                    div .field {
                        label .label for="export-date" { "Journée" }
                        input .input #export-date type="date" name="date" value=(gallery.date) max=(Local::now().date_naive()) required;
                        button .button.is-light.mt-2 #export-refresh type="submit" formaction={(prefix) "/webcam/export"} formnovalidate { "Actualiser les horaires du soleil" }
                    }
                    div .columns {
                        div .column {
                            label .label for="export-start" { "Début" }
                            input .input #export-start type="time" name="start" step="1" value=[start.map(|time| time.format("%H:%M:%S").to_string())] required;
                            p .help { "Par défaut : lever du soleil." }
                        }
                        div .column {
                            label .label for="export-end" { "Fin" }
                            input .input #export-end type="time" name="end" step="1" value=[end.map(|time| time.format("%H:%M:%S").to_string())] required;
                            p .help { "Par défaut : coucher du soleil, ou l’heure actuelle si elle est plus tôt." }
                        }
                        div .column {
                            label .label for="export-speed" { "Vitesse (×)" }
                            input .input #export-speed type="number" name="speed" min="1" max=(MAX_SPEED) step="1" value=(speed) required;
                            p .help { "À 2000×, une heure devient 1,8 seconde." }
                        }
                    }
                    p .help.mb-4 {
                        "Heures locales du serveur, comme dans les archives. Lever et coucher du soleil : "
                        a href="https://open-meteo.com/" { "Open-Meteo" } "."
                    }
                    @if let Some(camera) = &gallery.camera && !gallery.images.is_empty() {
                        section #export-selection .mb-5
                          data-list={(prefix) "/webcam/" (previews::list_name(gallery.date, camera))} hidden {
                            h2 .title.is-5.mb-2 { "Choisir la plage à exporter" }
                            p .help #export-range-help { "Déplacez les poignées (bleue : début, orange : fin) ou saisissez les heures ci-dessus. Relâchez pour charger l’image complète. Les flèches du clavier permettent un réglage à la seconde." }
                            div .webcam-export-range {
                                div .webcam-export-track aria-hidden="true" { span {} }
                                input #export-range-start type="range" min="0" max="86399" step="1" value="0"
                                  aria-label="Heure de début" aria-describedby="export-range-help" disabled;
                                input #export-range-end type="range" min="0" max="86399" step="1" value="86399"
                                  aria-label="Heure de fin" aria-describedby="export-range-help" disabled;
                            }
                            div .webcam-export-scale aria-hidden="true" {
                                span { "00:00" } span { "06:00" } span { "12:00" } span { "18:00" } span { "23:59" }
                            }
                            p .has-text-centered.my-3 {
                                output #export-range-label for="export-range-start export-range-end" {}
                            }
                            div .columns {
                                @for (side, title) in [("start", "Première image de la plage"), ("end", "Dernière image de la plage")] {
                                    div .column {
                                        figure .webcam-view.webcam-export-preview id={"export-preview-" (side)} {
                                            img .webcam-photo alt=(title) hidden;
                                            p .webcam-export-placeholder { "Choisissez une plage horaire." }
                                            figcaption .webcam-caption {
                                                div .webcam-caption-heading { strong { (title) } time {} }
                                            }
                                        }
                                        p .help data-preview-status=(side) role="status" {}
                                    }
                                }
                            }
                        }
                        p .help.mb-4 #export-selection-status role="status" {}
                        noscript { p .help.mb-4 { "Activez JavaScript pour utiliser les poignées et les aperçus, ou saisissez les heures ci-dessus." } }
                    }
                    p .mb-4 { "La vidéo conserve le temps entre les prises de vue. La première image couvre le début de la plage ; chaque image reste affichée jusqu’à la suivante. Résolution maximale : 1280 × 720." }
                    button #export-download .button.is-link type="submit" disabled[gallery.images.is_empty()] {
                        span .icon { i .fa-solid.fa-download aria-hidden="true" {} }
                        span { "Créer et télécharger la vidéo" }
                    }
                    p .help.mt-3 { "La préparation peut prendre quelques minutes. Gardez cette page ouverte jusqu’au téléchargement." }
                }
            }
        }
    };
    page(
        "Exporter la webcam",
        prefix,
        &NavKind::StaffOnly,
        "webcam",
        html! {},
        content,
        html! {},
    )
}
