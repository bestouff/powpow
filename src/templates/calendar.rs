use super::{NavKind, capitalize_words, page};
use crate::models::{Atelier, Need, Staff};
use chrono::Datelike;
use maud::{Markup, PreEscaped, html};
use std::collections::HashMap;

#[derive(serde::Serialize)]
struct AtelierJs<'a> {
    id: uuid::Uuid,
    name: &'a str,
    slug: &'a str,
    icon: &'a str,
    default_nightly: bool,
}

#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]
pub fn calendar(
    atelier: &Atelier,
    needs: &[Need],
    staff_list: &[Staff],
    presence: &HashMap<(uuid::Uuid, uuid::Uuid), (bool, bool)>,
    all_ateliers: &[Atelier],
    prefix: &str,
    viewer_id: Option<uuid::Uuid>,
    _is_admin: bool,
    opening_days: &[crate::models::OpeningDay],
) -> Markup {
    let p = prefix;

    // Precompute which needs are complete (both halves individually >= quantity)
    let complete_needs: std::collections::HashSet<uuid::Uuid> = needs
        .iter()
        .filter(|need| {
            let filled_first: i16 = staff_list
                .iter()
                .filter(|s| presence.get(&(need.id, s.id)).is_some_and(|(f, _)| *f))
                .count() as i16;
            let filled_second: i16 = staff_list
                .iter()
                .filter(|s| presence.get(&(need.id, s.id)).is_some_and(|(_, s)| *s))
                .count() as i16;
            filled_first >= need.quantity && filled_second >= need.quantity
        })
        .map(|n| n.id)
        .collect();

    // Build a lookup map for opening days
    let opening_map: std::collections::HashMap<chrono::NaiveDate, &crate::models::OpeningDay> =
        opening_days.iter().map(|od| (od.day, od)).collect();

    let content = html! {
        div #notification-container {}

        section .section.pt-4.pb-4 {
            div .container.is-fluid {
                h1 .title.is-4.mb-3 {
                    span .icon { i .fa-solid.fa-calendar-days {} }
                    " Planning \u{2014} " (atelier.name)
                }

                div .atelier-nav {
                    @for a in all_ateliers {
                        a .navbar-item.is-active[a.id == atelier.id] href={(p) "/calendar/" (a.slug)} {
                            span .icon { i class={"fa-solid fa-" (a.icon)} {} }
                            "\u{00a0}" (a.name)
                        }
                    }
                }

                div .cal-scroll {
                    table .cal-table.table.is-bordered.is-narrow.is-hoverable {
                        thead {
                            // Header row with day columns
                            tr {
                                th .cal-name-col { "Nom" }
                                @for need in needs {
                                    @let day_abbrev = need.day.format("%a").to_string();
                                    @let day_name = match day_abbrev.as_str() {
                                        "Mon" => "lun.",
                                        "Tue" => "mar.",
                                        "Wed" => "mer.",
                                        "Thu" => "jeu.",
                                        "Fri" => "ven.",
                                        "Sat" => "sam.",
                                        "Sun" => "dim.",
                                        _ => &day_abbrev,
                                    };
                                    @let day_date = need.day.format("%d/%m").to_string();
                                    @let is_sunday = need.day.weekday() == chrono::Weekday::Sun;
                                    @let filled_first: i16 = staff_list.iter().filter(|s| presence.get(&(need.id, s.id)).is_some_and(|(f, _)| *f)).count() as i16;
                                    @let filled_second: i16 = staff_list.iter().filter(|s| presence.get(&(need.id, s.id)).is_some_and(|(_, se)| *se)).count() as i16;
                                    @let both_complete = filled_first >= need.quantity && filled_second >= need.quantity;
                                    @let (first_label_h, second_label_h) = if need.nightly { ("soir", "nuit") } else { ("matin", "après-midi") };
                                    th .cal-day-col.has-text-centered.cal-sunday[is_sunday].cal-complete[both_complete].cal-danger[!both_complete] {
                                        div .cal-day-name { (day_name) }
                                        div .cal-day-date { (day_date) }
                                        div .cal-day-count {
                                            span .has-text-success[filled_first >= need.quantity].has-text-danger[filled_first < need.quantity] {
                                                (first_label_h) " " (filled_first) "/" (need.quantity)
                                            }
                                            " "
                                            span .has-text-success[filled_second >= need.quantity].has-text-danger[filled_second < need.quantity] {
                                                (second_label_h) " " (filled_second) "/" (need.quantity)
                                            }
                                        }
                                    }
                                }
                            }
                            // Opening day status row
                            tr .cal-opening-row {
                                td .cal-name-col { strong { "Ouverture" } }
                                @for need in needs {
                                    @let is_sunday = need.day.weekday() == chrono::Weekday::Sun;
                                    td .has-text-centered.cal-sunday[is_sunday] {
                                        @if let Some(od) = opening_map.get(&need.day) {
                                            @match od.status {
                                                crate::models::OpeningDayStatus::Reserved => {
                                                    span .tag.is-info { "Prévu" }
                                                },
                                                crate::models::OpeningDayStatus::Validated => {
                                                    span .tag.is-success { "Confirmé" }
                                                },
                                                crate::models::OpeningDayStatus::Canceled => {
                                                    span .tag.is-danger { "Annulé" }
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        tbody {
                            @for staff in staff_list {
                                @let can_toggle = viewer_id.is_some_and(|vid| staff.id == vid);
                                @let name = format!("{} {}", capitalize_words(&staff.first_name), capitalize_words(&staff.last_name));
                                tr .cal-me[can_toggle] {
                                    td .cal-name-col {
                                        a href={(p) "/person/" (staff.id)} { (name) }
                                    }
                                    @for need in needs {
                                        @let (first_half, second_half) = presence.get(&(need.id, staff.id)).copied().unwrap_or((false, false));
                                        @let (first_label, second_label) = if need.nightly { ("soir", "nuit") } else { ("matin", "après-midi") };
                                        @let is_active = first_half || second_half;
                                        @let is_sunday = need.day.weekday() == chrono::Weekday::Sun;
                                        @let is_complete = complete_needs.contains(&need.id);
                                        td .cal-cell.has-text-centered.cal-active[is_active].cal-sunday[is_sunday].cal-complete[is_complete].cal-danger[!is_complete] {
                                            label .cal-check title=(if need.nightly { "Soirée" } else { "Matin" }) {
                                                input .presence-cb type="checkbox"
                                                    data-need=(need.id)
                                                    data-staff=(staff.id)
                                                    data-half="first"
                                                    checked[first_half]
                                                    disabled[!can_toggle];
                                                span { (first_label) }
                                            }
                                            label .cal-check title=(if need.nightly { "Nuit" } else { "Après-midi" }) {
                                                input .presence-cb type="checkbox"
                                                    data-need=(need.id)
                                                    data-staff=(staff.id)
                                                    data-half="second"
                                                    checked[second_half]
                                                    disabled[!can_toggle];
                                                span { (second_label) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Empty messages
                @if needs.is_empty() {
                    div .notification.is-warning.is-light.mt-4 {
                        span .icon { i .fa-solid.fa-triangle-exclamation {} }
                        " Aucun besoin déclaré pour cet atelier."
                    }
                } @else if staff_list.is_empty() {
                    div .notification.is-info.is-light.mt-4 {
                        span .icon { i .fa-solid.fa-circle-info {} }
                        " Aucun bénévole assigné à cet atelier."
                    }
                }
            }
        }
    };

    let title = format!("Planning {} - PowPow", atelier.name);
    page(
        &title,
        p,
        &NavKind::Standard,
        "calendar",
        html! {},
        content,
        html! {},
    )
}

/// Email-friendly variant of `render_upcoming_week`: plain HTML without Bulma/FontAwesome,
/// using Unicode symbols instead.
pub fn render_upcoming_week_email(upcoming: &[(chrono::NaiveDate, String, i16, i64)]) -> String {
    let mut html = String::new();
    if upcoming.is_empty() {
        html.push_str("<p>Aucun besoin déclaré pour les 7 prochains jours.</p>");
        return html;
    }

    let mut current_day: Option<chrono::NaiveDate> = None;
    let mut day_deficits: Vec<(String, i64)> = Vec::new();

    let flush_day = |day: chrono::NaiveDate, deficits: &[(String, i64)], out: &mut String| {
        let day_abbrev = day.format("%a").to_string();
        let day_name = match day_abbrev.as_str() {
            "Mon" => "Lundi",
            "Tue" => "Mardi",
            "Wed" => "Mercredi",
            "Thu" => "Jeudi",
            "Fri" => "Vendredi",
            "Sat" => "Samedi",
            "Sun" => "Dimanche",
            _ => &day_abbrev,
        };
        let month_names = [
            "",
            "janvier",
            "février",
            "mars",
            "avril",
            "mai",
            "juin",
            "juillet",
            "août",
            "septembre",
            "octobre",
            "novembre",
            "décembre",
        ];
        let month_name = month_names[day.month() as usize];
        let date_str = format!("{} {} {}", day_name, day.day(), month_name);

        let missing_parts: Vec<String> = deficits
            .iter()
            .filter(|(_, missing)| *missing > 0)
            .map(|(name, missing)| format!("{} {}", missing, name.to_lowercase()))
            .collect();

        if missing_parts.is_empty() {
            out.push_str(&format!(
                "<p>\u{2713} <strong>{}</strong> — complet</p>\n",
                date_str
            ));
        } else {
            out.push_str(&format!(
                "<p>\u{26A0} <strong>{}</strong> — il manque {}</p>\n",
                date_str,
                missing_parts.join(", "),
            ));
        }
    };

    for (day, atelier_name, quantity, filled) in upcoming {
        let missing = i64::from(*quantity) - filled;
        if current_day != Some(*day) {
            if let Some(prev_day) = current_day {
                flush_day(prev_day, &day_deficits, &mut html);
            }
            current_day = Some(*day);
            day_deficits.clear();
        }
        day_deficits.push((atelier_name.clone(), missing.max(0)));
    }
    if let Some(prev_day) = current_day {
        flush_day(prev_day, &day_deficits, &mut html);
    }

    html
}

pub fn calendar_editor(
    all_ateliers: &[Atelier],
    editable_ids: &[uuid::Uuid],
    future_needs: &[(Need, i64, i64)],
    prefix: &str,
    logged_in: bool,
    is_admin: bool,
    opening_days: &[crate::models::OpeningDay],
) -> Markup {
    use std::collections::{BTreeMap, BTreeSet};

    let p = prefix;

    // Collect unique sorted days
    let days: Vec<chrono::NaiveDate> = {
        let mut s = BTreeSet::new();
        for (n, _, _) in future_needs {
            s.insert(n.day);
        }
        s.into_iter().collect()
    };

    // For each day, determine (has_day_need, has_night_need)
    let mut day_types: BTreeMap<chrono::NaiveDate, (bool, bool)> = BTreeMap::new();
    for (n, _, _) in future_needs {
        let entry = day_types.entry(n.day).or_insert((false, false));
        if n.nightly {
            entry.1 = true;
        } else {
            entry.0 = true;
        }
    }

    // Build needs_map: (atelier_id, day) -> (&Need, h1_count, h2_count)
    let mut needs_map: HashMap<(uuid::Uuid, chrono::NaiveDate), (&Need, i64, i64)> = HashMap::new();
    for (n, h1, h2) in future_needs {
        needs_map.insert((n.atelier, n.day), (n, *h1, *h2));
    }

    // French day-of-week abbreviations
    let day_abbrev = |d: chrono::NaiveDate| -> &'static str {
        match d.weekday() {
            chrono::Weekday::Mon => "lun.",
            chrono::Weekday::Tue => "mar.",
            chrono::Weekday::Wed => "mer.",
            chrono::Weekday::Thu => "jeu.",
            chrono::Weekday::Fri => "ven.",
            chrono::Weekday::Sat => "sam.",
            chrono::Weekday::Sun => "dim.",
        }
    };

    // Sub-column count for a day
    let subcols = |d: &chrono::NaiveDate| -> usize {
        let (has_day, has_night) = day_types.get(d).copied().unwrap_or((false, false));
        if has_day && has_night { 4 } else { 2 }
    };

    // Opening day map
    let opening_map: HashMap<chrono::NaiveDate, &crate::models::OpeningDay> =
        opening_days.iter().map(|od| (od.day, od)).collect();

    // Build editable atelier IDs as JSON array for JS
    let editable_json: String =
        serde_json::to_string(editable_ids).unwrap_or_else(|_| "[]".to_string());

    // Build atelier cards data as JSON for JS (used in the modal)
    let atelier_cards: Vec<AtelierJs<'_>> = all_ateliers
        .iter()
        .map(|a| AtelierJs {
            id: a.id,
            name: &a.name,
            slug: &a.slug,
            icon: &a.icon,
            default_nightly: a.default_nightly,
        })
        .collect();
    let ateliers_json: String =
        serde_json::to_string(&atelier_cards).unwrap_or_else(|_| "[]".to_string());

    let extra_head = html! {
        link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bulma-calendar-js@7.1.2/dist/css/bulma-calendar.min.css" integrity="sha384-PWg6kRaCiFAMYaANyvWqUS4fYZ2uKHjaQj1BDiCnzwBvLZTVoh6TvAFYRA+QpRhT" crossorigin="anonymous";
    };

    let content = html! {
        div #notification-container {}
        script #ateliers-data type="application/json" { (PreEscaped(&ateliers_json)) }
        script #editable-data type="application/json" { (PreEscaped(&editable_json)) }

        section .section.pt-4.pb-4 {
            div .container.is-fluid {
                h1 .title.is-4.mb-3 {
                    span .icon { i .fa-solid.fa-calendar-days {} }
                    " Planning des besoins"
                }

                // Calendar links section
                @if logged_in {
                    div .calendar-links {
                        span .has-text-grey.mr-1.cal-label { "Plannings :" }
                        @for a in all_ateliers {
                            a .tag.is-medium.is-link.is-light href={(p) "/calendar/" (a.slug)} {
                                span .icon { i class={"fa-solid fa-" (a.icon)} {} }
                                "\u{00a0}" (a.name)
                            }
                        }
                    }
                }

                // Add buttons section
                @if !editable_ids.is_empty() {
                    div .mb-4.buttons {
                        @if is_admin {
                            button .button.is-info #open-add-opening-day-modal {
                                span .icon { i .fa-solid.fa-sun {} }
                                span { "Ajouter un jour d'ouverture" }
                            }
                        }
                        button .button.is-primary #open-add-modal {
                            span .icon { i .fa-solid.fa-pen-to-square {} }
                            span { "Modifier des besoins en bénévoles" }
                        }
                    }
                }

                // Main table
                div .cal-scroll {
                    table .cal-table.table.is-bordered.is-narrow.is-hoverable {
                        thead {
                            // Header row 1: Atelier + date columns
                            tr {
                                th .cal-name-col rowspan="2" { "Atelier" }
                                @for d in &days {
                                    th .day-start colspan=(subcols(d)) {
                                        (day_abbrev(*d)) " " (format!("{:02}", d.day())) "/" (format!("{:02}", d.month()))
                                    }
                                }
                            }
                            // Header row 2: sub-column labels
                            tr {
                                @for d in &days {
                                    @let (has_day, has_night) = day_types.get(d).copied().unwrap_or((false, false));
                                    @if has_day && has_night {
                                        th .day-start { "matin" }
                                        th { "a-m" }
                                        th { "soir" }
                                        th { "nuit" }
                                    } @else if has_night {
                                        th .day-start { "soir" }
                                        th { "nuit" }
                                    } @else {
                                        th .day-start { "matin" }
                                        th { "a-m" }
                                    }
                                }
                            }
                            // Opening day row
                            tr .cal-opening-row {
                                td .cal-name-col { strong { "Ouverture" } }
                                @for d in &days {
                                    @let n_sub = subcols(d);
                                    td .day-start.has-text-centered colspan=(n_sub) {
                                        @if let Some(od) = opening_map.get(d) {
                                            @let (tag_class, tag_label) = match od.status {
                                                crate::models::OpeningDayStatus::Reserved => ("is-info", "Prévu"),
                                                crate::models::OpeningDayStatus::Validated => ("is-success", "Confirmé"),
                                                crate::models::OpeningDayStatus::Canceled => ("is-danger", "Annulé"),
                                            };
                                            @let day_str = d.format("%Y-%m-%d").to_string();
                                            @if is_admin && od.status == crate::models::OpeningDayStatus::Reserved {
                                                span class={"tag " (tag_class) " opening-tag is-clickable"} data-day=(day_str) { (tag_label) }
                                            } @else {
                                                span class={"tag " (tag_class)} { (tag_label) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        tbody {
                            @if days.is_empty() {
                                tr {
                                    td .cal-name-col colspan="100%" {
                                        em { "Aucun besoin à venir. Utilisez le bouton ci-dessus pour en créer." }
                                    }
                                }
                            }
                            @for atelier in all_ateliers {
                                tr {
                                    td .cal-name-col { (atelier.name) }
                                    @for d in &days {
                                        @let (has_day, has_night) = day_types.get(d).copied().unwrap_or((false, false));
                                        @let mixed = has_day && has_night;
                                        @let n_subcols = if mixed { 4_usize } else { 2_usize };
                                        @let day_str = d.format("%Y-%m-%d").to_string();
                                        @let entry = needs_map.get(&(atelier.id, *d));
                                        @match entry {
                                            None => {
                                                @for idx in 0..n_subcols {
                                                    @let cls = if idx == 0 { "day-cell day-start" } else { "day-cell" };
                                                    td class=(cls) data-day=(&day_str) {}
                                                }
                                            },
                                            Some((need, h1, h2)) => {
                                                @let qty = i64::from(need.quantity);
                                                @let pad_before = if mixed && need.nightly { 2_usize } else { 0_usize };
                                                @let pad_after = if mixed && !need.nightly { 2_usize } else { 0_usize };
                                                // Padding cells before (for nightly needs in mixed days)
                                                @for idx in 0..pad_before {
                                                    @let cls = if idx == 0 { "day-cell day-start" } else { "day-cell" };
                                                    td class=(cls) data-day=(&day_str) {}
                                                }
                                                // First half cell
                                                @let style_h1 = if *h1 >= qty { "cell-ok" } else { "cell-deficit" };
                                                @let cls_h1 = if pad_before == 0 {
                                                    format!("day-cell has-text-centered {style_h1} day-start")
                                                } else {
                                                    format!("day-cell has-text-centered {style_h1}")
                                                };
                                                td class=(&cls_h1) data-day=(&day_str) { (h1) "/" (qty) }
                                                // Second half cell
                                                @let style_h2 = if *h2 >= qty { "cell-ok" } else { "cell-deficit" };
                                                @let cls_h2 = format!("day-cell has-text-centered {style_h2}");
                                                td class=(&cls_h2) data-day=(&day_str) { (h2) "/" (qty) }
                                                // Padding cells after (for day needs in mixed days)
                                                @for _idx in 0..pad_after {
                                                    td .day-cell data-day=(&day_str) {}
                                                }
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Modal: day editor (opened by clicking a cell)
        div .modal #day-modal {
            div .modal-background {}
            div .modal-card.modal-card-wide {
                header .modal-card-head {
                    p .modal-card-title #day-modal-title { "\u{2014}" }
                    button .delete aria-label="close" #close-day-modal {}
                }
                section .modal-card-body {
                    div .atelier-cards #day-atelier-cards {}
                }
            }
        }

        // Modal: add needs via calendar picker
        div .modal #add-modal {
            div .modal-background {}
            div .modal-card.modal-card-wide {
                header .modal-card-head {
                    p .modal-card-title { "Modifier des besoins en bénévoles" }
                    button .delete aria-label="close" #close-add-modal {}
                }
                section .modal-card-body {
                    div .editor-columns {
                        div .editor-left {
                            input type="date" #calendar-widget;
                        }
                        div .editor-right {
                            div #add-edit-panel .d-none {
                                h2 .subtitle.is-5.mb-3 #add-panel-title { "\u{2014}" }
                                div .atelier-cards #add-atelier-cards {}
                            }
                            div .notification.is-info.is-light #add-no-selection {
                                span .icon { i .fa-solid.fa-hand-pointer {} }
                                " Sélectionnez une date sur le calendrier."
                            }
                        }
                    }
                }
            }
        }

        // Modal: add opening day via calendar picker
        div .modal #opening-day-modal {
            div .modal-background {}
            div .modal-card.modal-card-medium {
                header .modal-card-head {
                    p .modal-card-title { "Ajouter un jour d'ouverture" }
                    button .delete aria-label="close" #close-opening-day-modal {}
                }
                section .modal-card-body {
                    div .has-text-centered {
                        input type="date" #opening-day-picker;
                    }
                    div .mt-4.has-text-centered.d-none #opening-day-confirm {
                        p .mb-3 #opening-day-confirm-text {}
                        button .button.is-info #opening-day-submit {
                            span .icon { i .fa-solid.fa-check {} }
                            span { "Créer le jour d'ouverture" }
                        }
                    }
                }
            }
        }

        // Modal: Go / NoGo for a reserved opening day
        div .modal #gonogo-modal {
            div .modal-background {}
            div .modal-card.modal-card-small {
                header .modal-card-head {
                    p .modal-card-title #gonogo-title { "\u{2014}" }
                    button .delete aria-label="close" #close-gonogo-modal {}
                }
                section .modal-card-body.has-text-centered {
                    p .mb-4 { "Que souhaitez-vous faire pour cette journée ?" }
                    div .buttons.is-centered {
                        button .button.is-success.is-medium #gonogo-go {
                            span .icon { i .fa-solid.fa-circle-check {} }
                            span { "Go" }
                        }
                        button .button.is-danger.is-medium #gonogo-nogo {
                            span .icon { i .fa-solid.fa-circle-xmark {} }
                            span { "NO Go" }
                        }
                        button .button.is-medium #gonogo-cancel {
                            span { "Ne rien faire" }
                        }
                    }
                }
            }
        }
    };

    let extra_scripts = html! {
        script src="https://cdn.jsdelivr.net/npm/bulma-calendar-js@7.1.2/dist/js/bulma-calendar.min.js" integrity="sha384-onqOHSNjpIlm1BKqzaATbU2MGaNgk2Mam/76Tibn5+DBk35hQcm2NKYQP2hD/7EF" crossorigin="anonymous" {}
    };

    page(
        "Gestion des besoins - PowPow",
        prefix,
        &NavKind::Standard,
        "calendar",
        extra_head,
        content,
        extra_scripts,
    )
}
