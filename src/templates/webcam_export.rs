use chrono::{Local, NaiveTime};
use maud::{Markup, html};

use super::{NavKind, page};
use crate::webcam::{Gallery, export::MAX_SPEED};

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
                a .button.is-light.mb-5 href={(prefix) "/webcam" @if let Some(camera) = &gallery.camera { "?camera=" (camera) }} { "Retour à la webcam" }
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
                    p .mb-4 { "La vidéo conserve le temps entre les prises de vue. La première image couvre le début de la plage ; chaque image reste affichée jusqu’à la suivante. Résolution maximale : 1280 × 720." }
                    button .button.is-link type="submit" disabled[gallery.images.is_empty()] {
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
