use chrono::{Local, NaiveDate};
use maud::{Markup, html};

use super::{NavKind, page};
use crate::{
    forecast::Forecast,
    models::{AtelierDayPlanning, Equipment, EquipmentStatus, EquipmentType, OpeningDayStatus},
};

pub fn today_page(
    prefix: &str,
    today: NaiveDate,
    planning: &[AtelierDayPlanning],
    opening_status: Option<OpeningDayStatus>,
    equipments: &[Equipment],
    forecast: Option<&Forecast>,
    tomorrow_pdf: Option<NaiveDate>,
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
                }
                @if let Some(status) = opening_status {
                    @let (class, label) = match status {
                        OpeningDayStatus::Reserved => ("is-warning", "Ouverture prévue, à confirmer."),
                        OpeningDayStatus::Validated => ("is-success", "Ouverture confirmée."),
                        OpeningDayStatus::Canceled => ("is-danger", "Journée d'ouverture annulée."),
                    };
                    div class={"notification is-light " (class)} { (label) }
                }
                div .columns.is-multiline.today-summary.mb-5 {
                    div .column.is-full-tablet.is-one-third-desktop {
                        (weather(forecast))
                    }
                    @for kind in [EquipmentType::SkiSlope, EquipmentType::SkiTow] {
                        div .column.is-half-tablet.is-one-third-desktop {
                            (equipment_status(equipments, kind))
                        }
                    }
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
                div .buttons.today-planning-downloads {
                    (pdf_button(prefix, (!planning.is_empty()).then_some(today), "Planning d’aujourd’hui (PDF)", "Pas d’activité aujourd’hui"))
                    (pdf_button(prefix, tomorrow_pdf, "Planning de demain (PDF)", "Pas d’activité demain"))
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

fn pdf_button(prefix: &str, day: Option<NaiveDate>, label: &str, empty_label: &str) -> Markup {
    html! {
        @if let Some(day) = day {
            a .button.is-link.is-outlined href={(prefix) "/calendar/pdf/" (day)} download {
                span .icon { i .fa-solid.fa-file-pdf aria-hidden="true" {} }
                span { (label) }
            }
        } @else {
            button .button.is-light type="button" disabled { (empty_label) }
        }
    }
}

fn weather(forecast: Option<&Forecast>) -> Markup {
    html! {
        section .box #today-forecast {
            h2 .title.is-5 { "Prévisions météo" }
            p .subtitle.is-6 { "Saint-Hilaire · 1 200 m" }
            @if let Some(forecast) = forecast {
                @let (icon, label) = weather_condition(forecast.weather_code);
                p .is-size-5.mb-4 {
                    span .icon.mr-2 { i class={"fa-solid fa-" (icon)} aria-hidden="true" {} }
                    (label)
                }
                dl .today-forecast-values {
                    @for (label, value, unit) in [
                        ("Minimum", forecast.temperature_min_c, "°C"),
                        ("Maximum", forecast.temperature_max_c, "°C"),
                        ("Précipitations", forecast.precipitation_mm, "mm"),
                        ("Neige fraîche", forecast.snowfall_cm, "cm"),
                        ("Vent max.", forecast.wind_max_kmh, "km/h"),
                        ("Rafales max.", forecast.gust_max_kmh, "km/h"),
                    ] {
                        @if let Some(value) = value {
                            div {
                                dt .is-size-7.has-text-grey { (label) }
                                dd .has-text-weight-semibold { (format!("{value:.1}").replace('.', ",")) " " (unit) }
                            }
                        }
                    }
                }
                p .help.mt-4 {
                    a href="https://open-meteo.com/" { "Open-Meteo" }
                    " · actualisé à "
                    time datetime=(forecast.fetched_at.to_rfc3339()) {
                        (forecast.fetched_at.with_timezone(&Local).format("%H:%M"))
                    }
                }
            } @else {
                p .has-text-grey { "Prévisions temporairement indisponibles." }
            }
        }
    }
}

fn weather_condition(code: Option<u8>) -> (&'static str, &'static str) {
    match code {
        Some(0) => ("sun", "Ciel dégagé"),
        Some(1) => ("cloud-sun", "Peu nuageux"),
        Some(2) => ("cloud-sun", "Partiellement nuageux"),
        Some(3) => ("cloud", "Couvert"),
        Some(45 | 48) => ("smog", "Brouillard"),
        Some(51..=55) => ("cloud-rain", "Bruine"),
        Some(56 | 57 | 66 | 67) => ("cloud-rain", "Précipitations verglaçantes"),
        Some(61..=65) => ("cloud-rain", "Pluie"),
        Some(71..=77 | 85 | 86) => ("snowflake", "Neige"),
        Some(80..=82) => ("cloud-showers-heavy", "Averses"),
        Some(95..=99) => ("cloud-bolt", "Orages"),
        _ => ("cloud-sun", "Prévisions du jour"),
    }
}

fn equipment_status(equipments: &[Equipment], kind: EquipmentType) -> Markup {
    let equipments: Vec<_> = equipments
        .iter()
        .filter(|equipment| equipment.equipment_type == kind)
        .collect();
    let open = equipments
        .iter()
        .filter(|equipment| equipment.status == EquipmentStatus::Open)
        .count();
    let partial = equipments
        .iter()
        .filter(|equipment| equipment.status == EquipmentStatus::Partial)
        .count();
    let (title, opened, partially_opened) = match kind {
        EquipmentType::SkiSlope => ("Pistes", "ouvertes", "partielles"),
        EquipmentType::SkiTow => ("Téléskis", "ouverts", "partiels"),
    };
    html! {
        section .box {
            h2 .title.is-5 { (title) }
            @if equipments.is_empty() {
                p .has-text-grey { "Aucun équipement renseigné." }
            } @else {
                p .subtitle.is-6 {
                    (open) " / " (equipments.len()) " " (opened)
                    @if partial > 0 { " · " (partial) " " (partially_opened) }
                }
                ul .today-equipment-list {
                    @for equipment in equipments {
                        @let class = match equipment.status {
                            EquipmentStatus::Open => "is-success",
                            EquipmentStatus::Partial => "is-warning",
                            EquipmentStatus::Closed => "is-danger",
                        };
                        li .today-equipment-row {
                            span .today-equipment-name {
                                @if let Some(difficulty) = equipment.difficulty {
                                    span .difficulty-dot style={"background:" (difficulty.css_color())} {}
                                }
                                (equipment.name)
                            }
                            span class={"tag " (class)} {
                                @match kind {
                                    EquipmentType::SkiSlope => (equipment.status.label_piste()),
                                    EquipmentType::SkiTow => (equipment.status.label_tow()),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
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
