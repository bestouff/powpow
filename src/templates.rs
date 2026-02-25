use crate::models::{
    Atelier, Cash, Membership, MembershipWithStatus, Need, PhotoMeta, Role, Staff, StaffMatchType,
    StaffWithSeason, User,
};
use chrono::Datelike;
use phonenumber::Mode;
use std::collections::HashMap;
use std::sync::RwLock;

/// Global photo-of-the-day URL + photographer name, updated when photos change.
static PHOTO_BG_URL: RwLock<Option<String>> = RwLock::new(None);
static PHOTO_BG_AUTHOR: RwLock<Option<String>> = RwLock::new(None);

pub fn set_photo_bg(url: String, photographer: String) {
    if let Ok(mut w) = PHOTO_BG_URL.write() {
        *w = Some(url);
    }
    if let Ok(mut w) = PHOTO_BG_AUTHOR.write() {
        *w = Some(photographer);
    }
}

/// Simple HTML escaping for minimal security
pub fn escape_html_public(s: &str) -> String {
    escape_html(s)
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub struct TodoItem {
    pub icon: &'static str,
    pub color: &'static str,
    pub html: String,
}

/// Format a phone number to international format
/// Assumes French numbers if no country code is present
pub fn format_phone_international(phone: &str) -> String {
    if phone.is_empty() {
        return String::new();
    }

    // Try to parse with France as default country
    match phonenumber::parse(Some(phonenumber::country::Id::FR), phone) {
        Ok(number) => number.format().mode(Mode::International).to_string(),
        Err(_) => phone.to_string(), // Return original if parsing fails
    }
}

/// Capitalize each word in a string (first letter uppercase, rest lowercase)
/// Handles both spaces and hyphens as word separators
fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            // Handle hyphenated words like "Jean-Pierre"
            word.split('-')
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            first.to_uppercase().collect::<String>()
                                + &chars.as_str().to_lowercase()
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join("-")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

enum NavKind {
    Full,      // Adhésions, Cash, Staff, API, Login
    Standard,  // Adhésions, Cash, Staff, Login
    LoginOnly, // Only Login button
    StaffOnly, // Only Staff-related items
}

fn navbar(prefix: &str, kind: &NavKind, active: &str) -> String {
    let users_active = if active == "users" { " is-active" } else { "" };
    let cash_active = if active == "cash" { " is-active" } else { "" };
    let staff_active = if active == "staff" { " is-active" } else { "" };
    let calendar_active = if active == "calendar" {
        " is-active"
    } else {
        ""
    };

    // Admin links are always in the DOM; on pages that don't guarantee admin
    // access they start hidden and the /api/me script reveals them.
    let admin_hide = match kind {
        NavKind::LoginOnly => r#" style="display:none""#,
        _ => "",
    };

    let links = format!(
        r#"<a class="navbar-item{calendar_active}" href="{p}/calendar">
                        <span class="icon mr-1"><i class="fa-solid fa-calendar-days"></i></span>
                        Planning
                    </a>
                    <a class="navbar-item navbar-admin{users_active}" href="{p}/users"{admin_hide}>
                        <span class="icon mr-1"><i class="fa-solid fa-ticket"></i></span>
                        Adhésions
                        <span class="nav-badge" data-badge="users" style="display:none"></span>
                    </a>
                    <a class="navbar-item navbar-admin{cash_active}" href="{p}/cash"{admin_hide}>
                        <span class="icon mr-1"><i class="fa-solid fa-money-bill-wave"></i></span>
                        Espèces / Chèques
                        <span class="nav-badge" data-badge="cash" style="display:none"></span>
                    </a>
                    <a class="navbar-item navbar-admin{staff_active}" href="{p}/staff"{admin_hide}>
                        <span class="icon mr-1"><i class="fa-solid fa-user-group"></i></span>
                        Staff
                    </a>
                    <a class="navbar-item" id="login-btn" href="{p}/login"><i class="fa-solid fa-right-to-bracket"></i>&nbsp;Se connecter</a>"#,
        p = prefix,
        admin_hide = admin_hide,
        calendar_active = calendar_active,
        users_active = users_active,
        cash_active = cash_active,
        staff_active = staff_active,
    );

    format!(
        r#"<nav class="navbar is-dark" role="navigation" aria-label="main navigation">
        <div class="container is-fluid">
            <div class="navbar-brand">
                <a class="navbar-item" href="{p}/">
                    <span class="icon mr-2"><i class="fa-solid fa-person-skiing"></i></span>
                    <strong>PowPow pour AGH'IL</strong>
                </a>
                <a role="button" class="navbar-burger" aria-label="menu" aria-expanded="false" data-target="main-navbar">
                    <span aria-hidden="true"></span>
                    <span aria-hidden="true"></span>
                    <span aria-hidden="true"></span>
                </a>
            </div>
            <div id="main-navbar" class="navbar-menu">
                <div class="navbar-end">
                    {links}
                </div>
            </div>
        </div>
    </nav>
    <script>
    document.addEventListener('DOMContentLoaded', function() {{
        var burger = document.querySelector('.navbar-burger');
        var menu = document.getElementById(burger.dataset.target);
        burger.addEventListener('click', function() {{
            burger.classList.toggle('is-active');
            menu.classList.toggle('is-active');
        }});
    }});
    </script>"#,
        p = prefix,
        links = links,
    )
}

fn page(
    title: &str,
    prefix: &str,
    nav_kind: &NavKind,
    active: &str,
    extra_head: &str,
    content: &str,
    extra_scripts: &str,
) -> String {
    let nav = navbar(prefix, nav_kind, active);
    let p = prefix;
    let badge_css = r"<style>.navbar-item{position:relative;}.nav-badge{background:var(--bulma-danger);color:var(--bulma-scheme-main);border-radius:999px;min-width:18px;height:18px;font-size:0.65rem;font-weight:bold;display:inline-flex;align-items:center;justify-content:center;padding:0 4px;position:absolute;top:6px;right:-2px;line-height:1;box-shadow:0 0 0 2px var(--bulma-navbar-background-color,#363636);}.button .nav-badge{position:static;margin-left:6px;box-shadow:none;}</style>";
    let badge_script = format!(
        "<script>fetch('{p}/api/badge-counts').then(r=>r.json()).then(d=>{{document.querySelectorAll('.nav-badge').forEach(b=>{{const c=d[b.dataset.badge];if(c>0){{b.textContent=c;b.style.display='';}}}});}}).catch(()=>{{}});</script>",
        p = p,
    );
    let me_script = format!(
        "<script>fetch('{p}/api/me').then(r=>{{if(r.ok)return r.json();throw 0;}}).then(d=>{{const b=document.getElementById('login-btn');if(b){{b.innerHTML='<i class=\"fa-solid fa-user\"></i>&nbsp;'+d.first_name+' '+d.last_name;b.href='{p}/person/'+d.id;const lo=document.createElement('a');lo.className='navbar-item';lo.href='{p}/logout';lo.innerHTML='<i class=\"fa-solid fa-right-from-bracket\"></i>';b.parentNode.insertBefore(lo,b.nextSibling);}}if(d.is_admin){{document.querySelectorAll('.navbar-admin').forEach(el=>el.style.display='');}}}}).catch(()=>{{}});</script>",
        p = p,
    );

    let photo_credit = PHOTO_BG_AUTHOR
        .read()
        .ok()
        .and_then(|r| r.clone())
        .map(|name| {
            format!(
                r#"<p class="is-size-7 has-text-grey">photo &copy; {}</p>"#,
                escape_html(&name)
            )
        })
        .unwrap_or_default();

    let photo_bg_css = PHOTO_BG_URL.read().ok().and_then(|r| r.clone()).map(|url| format!(
        r"<style>
        body {{
            background-image: linear-gradient(rgba(255, 255, 255, 0.15), rgba(255, 255, 255, 0.15)), url('{}{}');
            background-size: cover;
            background-position: center;
            background-attachment: fixed;
            min-height: 100vh;
        }}
        .section, .box, .footer {{
            background-color: rgba(255, 255, 255, 0.65);
        }}
        </style>",
        p, url
    )).unwrap_or_default();

    format!(
        r#"<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="google-site-verification" content="S04nKUrv5gsWl0VqBBdd9Q6zS7rxLWHJLc2aFftaD4E" />
    <title>{title}</title>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bulma@1.0.4/css/bulma.min.css">
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@fortawesome/fontawesome-free@7.2.0/css/fontawesome.min.css">
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@fortawesome/fontawesome-free@7.2.0/css/solid.min.css">
    {photo_bg_css}
    {extra_head}
</head>
<body>
    {nav}

    {content}

    {extra_scripts}
{badge_css}
{badge_script}
{me_script}
    <footer class="footer py-4"><div class="content has-text-centered"><p class="is-size-7 has-text-grey">PowPow v{version} pour AG'HIL, &copy;2026 Xavier Bestel &lt;xav@bes.tel&gt; — <a href="{p}/privacy">Confidentialité</a> · <a href="{p}/tos">CGU</a></p>{photo_credit}</div></footer>
</body>
</html>"#,
        title = title,
        extra_head = extra_head,
        photo_bg_css = photo_bg_css,
        nav = nav,
        content = content,
        extra_scripts = extra_scripts,
        badge_css = badge_css,
        badge_script = badge_script,
        me_script = me_script,
        version = env!("CARGO_PKG_VERSION"),
    )
}

pub fn index(
    prefix: &str,
    staff: Option<&Staff>,
    current_season: i16,
    has_paid: bool,
    chief_ateliers: &[Atelier],
    upcoming: &[(chrono::NaiveDate, String, i16, i64)],
) -> String {
    let extra_head = r"<style>
            .week-day { padding: 0.5rem 0; border-bottom: 1px solid var(--bulma-border-weak); display: flex; align-items: center; gap: 0.4rem; }
        </style>";

    let mut sections = String::new();

    // --- Hero (Anonymous) ---
    sections.push_str(
        r#"    <section class="hero is-info">
        <div class="hero-body">
            <div class="container has-text-centered">
                <h1 class="title is-2 mb-4">
                    <span class="icon is-large"><i class="fa-solid fa-person-skiing fa-2x"></i></span>
                    <br>
                    Gestionnaire de plannings bénévoles
                </h1>
                <h2 class="subtitle is-5">
                    PowPow (Pistes, Organisation, Week-end, Planning, Optimisation, Wouah!) pour AG'HIL
                </h2>
            </div>
        </div>
    </section>"#,
    );

    if let Some(staff) = staff {
        // --- Membership status (Staff) ---
        let season_display = format!("{}-{}", current_season - 1, current_season);
        if has_paid {
            sections.push_str(&format!(
                r#"
    <section class="section py-4">
        <div class="container is-fluid">
            <div class="notification is-success is-light">
                <span class="icon"><i class="fa-solid fa-circle-check"></i></span>
                Ta cotisation est à jour pour la saison {season}.
            </div>
        </div>
    </section>"#,
                season = season_display,
            ));
        } else {
            sections.push_str(&format!(
                r#"
    <section class="section py-4">
        <div class="container is-fluid">
            <div class="notification is-warning is-light">
                <span class="icon"><i class="fa-solid fa-triangle-exclamation"></i></span>
                Ta cotisation n'est pas à jour pour la saison {season} &mdash; <a href="https://www.helloasso.com/associations/agir-pour-la-station-de-ski-de-st-hil" target="_blank">inscris-toi sur HelloAsso</a>.
            </div>
        </div>
    </section>"#,
                season = season_display,
            ));
        }

        // --- My profile (Staff) ---
        sections.push_str(&format!(
            r#"
    <section class="section py-4">
        <div class="container is-fluid">
            <a class="box" href="{p}/person/{id}">
                <span class="icon mr-2"><i class="fa-solid fa-user-gear"></i></span>
                <strong>Gérer mes ateliers et mes préférences</strong>
            </a>
        </div>
    </section>"#,
            p = prefix,
            id = staff.id,
        ));

        // --- Chief ateliers (Chief) ---
        if !chief_ateliers.is_empty() {
            let mut links = String::new();
            for a in chief_ateliers {
                links.push_str(&format!(
                    r#"<a class="button is-link is-light mr-2 mb-2" href="{p}/calendar/{slug}">
                        <span class="icon"><i class="fa-solid fa-{icon}"></i></span>&nbsp;
                        <span>{name}</span>
                    </a>"#,
                    p = prefix,
                    slug = a.slug,
                    icon = a.icon,
                    name = a.name,
                ));
            }
            sections.push_str(&format!(
                r#"
    <section class="section py-4">
        <div class="container is-fluid">
            <div class="box">
                <h3 class="title is-5 mb-3">
                    <span class="icon mr-2"><i class="fa-solid fa-user-shield"></i></span>
                    Mes ateliers
                </h3>
                <div class="buttons">{links}</div>
            </div>
        </div>
    </section>"#,
                links = links,
            ));
        }
    }

    // --- Upcoming week (Anonymous) ---
    let week_html = render_upcoming_week(upcoming);
    sections.push_str(&format!(
        r#"
    <section class="section py-4">
        <div class="container is-fluid">
            <a class="box" href="{p}/calendar" style="display:block;text-decoration:none;color:inherit;">
                <h3 class="title is-5 mb-3">
                    <span class="icon mr-2"><i class="fa-solid fa-calendar-week"></i></span>
                    Semaine à venir
                </h3>
                {week_html}
            </a>
        </div>
    </section>"#,
        p = prefix,
        week_html = week_html,
    ));

    if let Some(staff) = staff {
        // --- Admin: memberships & staff ---
        if staff.is_admin {
            sections.push_str(&format!(
                r#"
    <section class="section py-4">
        <div class="container is-fluid">
            <div class="box">
                <h3 class="title is-5 mb-3">
                    <span class="icon mr-2"><i class="fa-solid fa-ticket"></i></span>
                    Gestion des adhésions
                </h3>
                <div class="buttons">
                    <a class="button is-primary" href="{p}/users">
                        <span class="icon"><i class="fa-solid fa-ticket"></i></span>
                        <span>Adhésions HelloAsso</span>
                        <span class="nav-badge" data-badge="users" style="display:none"></span>
                    </a>
                    <a class="button is-primary is-light" href="{p}/cash">
                        <span class="icon"><i class="fa-solid fa-money-bill-wave"></i></span>
                        <span>Espèces / Chèques</span>
                        <span class="nav-badge" data-badge="cash" style="display:none"></span>
                    </a>
                </div>
            </div>
        </div>
    </section>
    <section class="section py-4">
        <div class="container is-fluid">
            <div class="box">
                <h3 class="title is-5 mb-3">
                    <span class="icon mr-2"><i class="fa-solid fa-user-group"></i></span>
                    Gestion du staff
                </h3>
                <div class="buttons">
                <a class="button is-link" href="{p}/staff">
                    <span class="icon"><i class="fa-solid fa-user-group"></i></span>
                    <span>Voir le staff</span>
                </a>
                <a class="button is-link is-light" href="{p}/export/mailchimp">
                    <span class="icon"><i class="fa-solid fa-file-csv"></i></span>
                    <span>Export Mailchimp</span>
                </a>
                <a class="button is-light" href="{p}/audit">
                    <span class="icon"><i class="fa-solid fa-clipboard-list"></i></span>
                    <span>Journal d'audit</span>
                </a>
                </div>
            </div>
        </div>
    </section>"#,
                p = prefix,
            ));
        }

        // --- God: backup/restore ---
        if staff.is_god {
            sections.push_str(&format!(
                r#"
    <section class="section py-4">
        <div class="container is-fluid">
            <div class="box">
                <h3 class="title is-5 mb-3">
                    <span class="icon mr-2"><i class="fa-solid fa-database"></i></span>
                    Sauvegarde / Restauration
                </h3>
                <div class="buttons">
                    <a class="button is-warning" href="{p}/backup">
                        <span class="icon"><i class="fa-solid fa-download"></i></span>
                        <span>Télécharger la sauvegarde</span>
                    </a>
                    <a class="button is-danger" href="{p}/restore">
                        <span class="icon"><i class="fa-solid fa-upload"></i></span>
                        <span>Restaurer</span>
                    </a>
                </div>
            </div>
        </div>
    </section>"#,
                p = prefix,
            ));
        }
    }

    page(
        "PowPow for AGH'IL",
        prefix,
        &NavKind::LoginOnly,
        "",
        extra_head,
        &sections,
        "",
    )
}

#[allow(clippy::too_many_arguments)]
pub fn membership_list_with_filters(
    memberships_with_status: Vec<(User, MembershipWithStatus)>,
    search: Option<String>,
    only_not_imported: bool,
    total_count: usize,
    imported_count: usize,
    not_imported_count: usize,
    current_season: i16,
    prefix: &str,
) -> String {
    let mut rows_html = String::new();
    let displayed_count = memberships_with_status.len();

    for (_user, membership_with_status) in memberships_with_status {
        let membership = &membership_with_status.membership;
        let beneficiary_name = format!(
            "{} {}",
            membership.beneficiary_first_name.as_deref().unwrap_or(""),
            membership.beneficiary_last_name.as_deref().unwrap_or("")
        )
        .trim()
        .to_string();

        // Detect type: Adhésion or Donation
        let (type_label, type_class) = match membership.item_type.as_deref() {
            Some("Donation") => ("Don", "is-info"),
            Some("Membership") => ("Adhésion", "is-primary"),
            _ => ("?", "is-light"),
        };

        let amount = membership.amount.map_or_else(
            || "N/A".to_string(),
            |a| format!("{:.2}€", a as f32 / 100.0),
        );
        let order_date = membership
            .order_date
            .map_or_else(|| "N/A".to_string(), |d| d.format("%d/%m/%Y").to_string());
        let membership_phone = membership.phone.as_deref().unwrap_or("");
        let membership_email = membership.email.as_deref().unwrap_or("");
        let season = membership_with_status.season;

        // Highlight current season differently
        let season_tag_class = if season == current_season {
            "is-primary"
        } else {
            "is-light"
        };

        // Generate status badge
        let status_html = if membership_with_status.is_double_subscription {
            r#"<span class="tag is-danger">Double adhésion</span>"#.to_string()
        } else if membership_with_status.has_staff {
            r#"<span class="tag is-success">Importé</span>"#.to_string()
        } else {
            format!(
                r#"<a href="{}/import/{}" class="tag is-warning">À importer</a>"#,
                prefix, membership.helloasso_item_id
            )
        };

        rows_html.push_str(&format!(
            r#"
                <tr>
                    <td><strong>{}</strong></td>
                    <td>{}</td>
                    <td>{}</td>
                    <td><span class="tag {}">{}</span></td>
                    <td class="has-text-right"><strong class="has-text-success">{}</strong></td>
                    <td>{}</td>
                    <td><span class="tag {}">{}</span></td>
                    <td>{}</td>
                </tr>"#,
            beneficiary_name,
            membership_email,
            membership_phone,
            type_class,
            type_label,
            amount,
            order_date,
            season_tag_class,
            season,
            status_html
        ));
    }

    let search_value = search.as_deref().unwrap_or("");
    let has_filters = search.is_some() || only_not_imported;
    let clear_filters_html = if has_filters {
        format!(
            r#"<a href="{}/users" class="button is-light is-small ml-2">
            <span class="icon"><i class="fa-solid fa-xmark"></i></span>
            <span>Effacer filtres</span>
        </a>"#,
            prefix
        )
    } else {
        String::new()
    };

    let total_card_active = if !only_not_imported && search.is_none() {
        "is-active"
    } else {
        ""
    };
    let not_imported_card_active = if only_not_imported { "is-active" } else { "" };
    let filter_all_class = if only_not_imported {
        "is-light"
    } else {
        "is-primary"
    };
    let filter_not_imported_class = if only_not_imported {
        "is-primary"
    } else {
        "is-light"
    };

    let extra_head = r"<style>
        .table-container {
            overflow-x: auto;
        }
        .stat-card {
            transition: transform 0.2s, box-shadow 0.2s;
            cursor: pointer;
        }
        .stat-card:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(0,0,0,0.15);
        }
        .stat-card.is-active {
            border: 3px solid var(--bulma-link);
        }
        .stat-number {
            font-size: 2.5rem;
            font-weight: bold;
            line-height: 1;
        }
    </style>";

    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="level mb-4">
                <div class="level-left">
                    <h1 class="title is-3">Adhésions HelloAsso</h1>
                </div>
                <div class="level-right">
                    <a href="{p}/sync" class="button is-primary">
                        <span class="icon"><i class="fa-solid fa-arrows-rotate"></i></span>
                        <span>Synchronisation manuelle</span>
                    </a>
                </div>
            </div>

            <!-- Stats Cards -->
            <div class="columns mb-4">
                <div class="column is-4">
                    <a href="{p}/users" class="box stat-card has-text-centered {total_card_active}">
                        <span class="icon is-large has-text-info">
                            <i class="fa-solid fa-ticket fa-2x"></i>
                        </span>
                        <p class="stat-number has-text-info mt-2">{total_count}</p>
                        <p class="has-text-grey">Total adhésions</p>
                    </a>
                </div>
                <div class="column is-4">
                    <a href="{p}/users?filter=all" class="box stat-card has-text-centered">
                        <span class="icon is-large has-text-success">
                            <i class="fa-solid fa-circle-check fa-2x"></i>
                        </span>
                        <p class="stat-number has-text-success mt-2">{imported_count}</p>
                        <p class="has-text-grey">Importées</p>
                    </a>
                </div>
                <div class="column is-4">
                    <a href="{p}/users?filter=not_imported" class="box stat-card has-text-centered {not_imported_card_active}">
                        <span class="icon is-large has-text-warning">
                            <i class="fa-solid fa-circle-exclamation fa-2x"></i>
                        </span>
                        <p class="stat-number has-text-warning mt-2">{not_imported_count}</p>
                        <p class="has-text-grey">À importer</p>
                    </a>
                </div>
            </div>

            <!-- Search and Filter Box -->
            <div class="box mb-4">
                <form method="GET" action="{p}/users" id="filterForm">
                    <div class="columns is-vcentered">
                        <div class="column is-6">
                            <div class="field has-addons">
                                <div class="control is-expanded has-icons-left">
                                    <input class="input" type="text" name="search" id="searchInput"
                                           placeholder="Rechercher par email, nom ou prénom..." value="{search_value}">
                                    <span class="icon is-left">
                                        <i class="fa-solid fa-magnifying-glass"></i>
                                    </span>
                                </div>
                                <div class="control">
                                    <button type="submit" class="button is-info">
                                        <span class="icon"><i class="fa-solid fa-magnifying-glass"></i></span>
                                    </button>
                                </div>
                            </div>
                        </div>
                        <div class="column is-6">
                            <div class="field is-grouped is-grouped-right">
                                <div class="control">
                                    <div class="buttons has-addons">
                                        <a href="{p}/users?search={search_value}" class="button {filter_all_class} is-medium">
                                            <span class="icon"><i class="fa-solid fa-list"></i></span>
                                            <span>Toutes</span>
                                        </a>
                                        <a href="{p}/users?search={search_value}&filter=not_imported" class="button {filter_not_imported_class} is-medium">
                                            <span class="icon"><i class="fa-solid fa-circle-exclamation"></i></span>
                                            <span>À importer</span>
                                        </a>
                                    </div>
                                </div>
                                {clear_filters_html}
                            </div>
                        </div>
                    </div>
                </form>
            </div>

            <!-- Results Box -->
            <div class="box">
                <div class="level mb-3">
                    <div class="level-left">
                        <p><strong>{displayed_count}</strong> adhésion(s) affichée(s)</p>
                    </div>
                </div>
                <div class="table-container">
                    <table class="table is-fullwidth is-striped is-hoverable">
                        <thead>
                            <tr>
                                <th>Bénéficiaire</th>
                                <th>Email</th>
                                <th>Téléphone</th>
                                <th>Type</th>
                                <th class="has-text-right">Montant</th>
                                <th>Date</th>
                                <th>Saison</th>
                                <th>Statut</th>
                            </tr>
                        </thead>
                        <tbody>
                            {rows_html}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    </section>"#,
        p = prefix,
        total_card_active = total_card_active,
        total_count = total_count,
        imported_count = imported_count,
        not_imported_card_active = not_imported_card_active,
        not_imported_count = not_imported_count,
        search_value = search_value,
        filter_all_class = filter_all_class,
        filter_not_imported_class = filter_not_imported_class,
        clear_filters_html = clear_filters_html,
        displayed_count = displayed_count,
        rows_html = rows_html
    );

    let scripts = r"<script>
        // Auto-submit on enter in search field
        document.getElementById('searchInput').addEventListener('keypress', function(e) {
            if (e.key === 'Enter') {
                document.getElementById('filterForm').submit();
            }
        });
    </script>";

    page(
        "Liste des Adhésions - HelloAsso",
        prefix,
        &NavKind::Full,
        "users",
        extra_head,
        &content,
        scripts,
    )
}

pub fn already_imported_page(membership: Membership, season: i16, prefix: &str) -> String {
    let beneficiary_first = membership.beneficiary_first_name.as_deref().unwrap_or("");
    let beneficiary_last = membership.beneficiary_last_name.as_deref().unwrap_or("");
    let beneficiary_name = format!("{} {}", beneficiary_first, beneficiary_last)
        .trim()
        .to_string();

    let email = membership.email.as_deref().unwrap_or("N/A");
    let item_name = membership.item_name.as_deref().unwrap_or("N/A");
    let amount = membership.amount.map_or_else(
        || "N/A".to_string(),
        |a| format!("{:.2}€", a as f32 / 100.0),
    );
    let order_date = membership
        .order_date
        .map_or_else(|| "N/A".to_string(), |d| d.format("%d/%m/%Y").to_string());

    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="columns is-centered">
                <div class="column is-8">
                    <div class="box has-text-centered">
                        <span class="icon is-large has-text-success mb-4">
                            <i class="fa-solid fa-circle-check fa-4x"></i>
                        </span>
                        <h1 class="title is-3 has-text-success">Adhésion déjà importée</h1>
                        <p class="subtitle is-5 mb-5">Cette adhésion a déjà été importée dans le système pour la saison {season}.</p>

                        <div class="box" style="background-color: var(--bulma-scheme-main-bis);">
                            <h2 class="title is-5 mb-4">Détails de l'adhésion</h2>
                            <table class="table is-fullwidth">
                                <tbody>
                                    <tr>
                                        <th>Bénéficiaire</th>
                                        <td><strong>{beneficiary_name}</strong></td>
                                    </tr>
                                    <tr>
                                        <th>Email</th>
                                        <td>{email}</td>
                                    </tr>
                                    <tr>
                                        <th>Article</th>
                                        <td>{item_name}</td>
                                    </tr>
                                    <tr>
                                        <th>Montant</th>
                                        <td>{amount}</td>
                                    </tr>
                                    <tr>
                                        <th>Date</th>
                                        <td>{order_date}</td>
                                    </tr>
                                    <tr>
                                        <th>Saison</th>
                                        <td><span class="tag is-success is-medium">{season}</span></td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>

                        <a href="{p}/users" class="button is-primary is-medium mt-4">
                            <span class="icon"><i class="fa-solid fa-arrow-left"></i></span>
                            <span>Retour aux adhésions</span>
                        </a>
                    </div>
                </div>
            </div>
        </div>
    </section>"#,
        p = prefix,
        season = season,
        beneficiary_name = beneficiary_name,
        email = email,
        item_name = item_name,
        amount = amount,
        order_date = order_date
    );

    page(
        "Adhésion déjà importée - AGHIL",
        prefix,
        &NavKind::Standard,
        "",
        "",
        &content,
        "",
    )
}

pub fn import_staff_form(
    membership: Membership,
    season: i16,
    candidates: Vec<StaffWithSeason>,
    payer_email: Option<&str>,
    name_already_exists: bool,
    prefix: &str,
) -> String {
    let beneficiary_first =
        capitalize_words(membership.beneficiary_first_name.as_deref().unwrap_or(""));
    let beneficiary_last =
        capitalize_words(membership.beneficiary_last_name.as_deref().unwrap_or(""));
    let beneficiary_name = format!("{} {}", beneficiary_first, beneficiary_last)
        .trim()
        .to_string();

    let membership_email = membership.email.as_deref().unwrap_or("").to_lowercase();
    let payer_email = payer_email.unwrap_or("").to_lowercase();
    // Use membership email if available, otherwise fall back to payer email
    let default_email = if membership_email.is_empty() {
        &payer_email
    } else {
        &membership_email
    };
    let phone = format_phone_international(membership.phone.as_deref().unwrap_or(""));
    let comment = membership.comment.as_deref().unwrap_or("");
    let item_name = membership.item_name.as_deref().unwrap_or("N/A");
    let amount = membership.amount.map_or_else(
        || "N/A".to_string(),
        |a| format!("{:.2}€", a as f32 / 100.0),
    );
    let order_date = membership
        .order_date
        .map_or_else(|| "N/A".to_string(), |d| d.format("%d/%m/%Y").to_string());

    // Check if this is a donation
    let is_donation = membership.item_type.as_deref() == Some("Donation");

    // Check if we have any exact matches (these should be recommended over "create new")
    let has_exact_match = candidates.iter().any(|c| {
        matches!(
            c.match_type,
            StaffMatchType::ExactBoth | StaffMatchType::ExactEmail | StaffMatchType::ExactName
        )
    });

    // Check if we have a double subscription match
    let has_double_subscription = candidates
        .iter()
        .any(|c| c.match_type == StaffMatchType::DoubleSubscription);

    // For donations with double subscription, recommend the double subscription option
    // Otherwise, recommend create if no exact matches (and name doesn't already exist)
    let recommend_double_subscription = is_donation && has_double_subscription;
    let recommend_create =
        !has_exact_match && !recommend_double_subscription && !name_already_exists;
    let allow_create = !name_already_exists;

    // Build candidates HTML
    let mut candidates_html = String::new();
    // is_first controls highlighting: true means next exact match gets "best option" highlight
    let mut is_first = !recommend_create && !recommend_double_subscription;
    let mut is_first_double_subscription = recommend_double_subscription;
    let mut option_index = 0usize; // For alternating background colors

    // Options: update existing staff if there are candidates
    for candidate in &candidates {
        let staff = &candidate.staff;
        let match_label = match candidate.match_type {
            StaffMatchType::ExactBoth => "Email et nom identiques",
            StaffMatchType::ExactName => "Nom identique",
            StaffMatchType::ExactEmail => "Email identique",
            StaffMatchType::PayerEmailMatch => "Email payeur identique",
            StaffMatchType::SimilarEmail => "Email similaire",
            StaffMatchType::SimilarName => "Nom similaire",
            StaffMatchType::DoubleSubscription => {
                if is_donation {
                    "Adhésion + don détecté"
                } else {
                    "Double adhésion probable"
                }
            }
        };

        let season_info = candidate.latest_season.map_or_else(
            || "Aucune saison".to_string(),
            |s| format!("Dernière saison: {}", s),
        );

        // Highlight and recommendation based on match type and position
        let is_exact_match = matches!(
            candidate.match_type,
            StaffMatchType::ExactBoth | StaffMatchType::ExactEmail | StaffMatchType::ExactName
        );

        let (highlight, recommended, border_color) = if candidate.match_type
            == StaffMatchType::DoubleSubscription
            && is_first_double_subscription
        {
            // Double subscription recommended (for donations): primary highlight
            (
                "is-primary",
                r#"<span class="tag is-success ml-2">Probable meilleure option</span>"#,
                "var(--bulma-primary)",
            )
        } else if candidate.match_type == StaffMatchType::DoubleSubscription {
            // Double subscription not recommended: red highlight
            (
                "is-danger",
                r#"<span class="tag is-danger ml-2">Double adhésion</span>"#,
                "var(--bulma-danger)",
            )
        } else if is_first && is_exact_match {
            // First exact match: primary highlight, recommended
            (
                "is-primary",
                r#"<span class="tag is-success ml-2">Probable meilleure option</span>"#,
                "var(--bulma-primary)",
            )
        } else if is_exact_match {
            // Other exact matches: info highlight
            (
                "is-info",
                r#"<span class="tag is-warning ml-2">Option envisageable</span>"#,
                "var(--bulma-info)",
            )
        } else {
            // Fuzzy matches (SimilarEmail, SimilarName): light, no recommendation
            ("is-light", "", "var(--bulma-border)")
        };

        // Check if names are the same (case-insensitive)
        let names_match = beneficiary_first.to_lowercase() == staff.first_name.to_lowercase()
            && beneficiary_last.to_lowercase() == staff.last_name.to_lowercase();

        // Build name choice HTML
        let name_choice_html = if names_match {
            format!(
                r#"<input type="hidden" name="first_name" value="{}">
                   <input type="hidden" name="last_name" value="{}">"#,
                beneficiary_first, beneficiary_last
            )
        } else {
            format!(
                r#"<div class="field">
                    <label class="label">Garder le prénom et nom</label>
                    <div class="control">
                        <label class="radio">
                            <input type="radio" name="name_choice" value="membership" checked onchange="updateNameFields(this.form, '{}', '{}')">
                            De l'adhésion: <strong>{} {}</strong>
                        </label>
                        <br>
                        <label class="radio">
                            <input type="radio" name="name_choice" value="staff" onchange="updateNameFields(this.form, '{}', '{}')">
                            Du staff: <strong>{} {}</strong>
                        </label>
                    </div>
                </div>
                <input type="hidden" name="first_name" value="{}">
                <input type="hidden" name="last_name" value="{}">"#,
                beneficiary_first,
                beneficiary_last,
                beneficiary_first,
                beneficiary_last,
                staff.first_name,
                staff.last_name,
                staff.first_name,
                staff.last_name,
                beneficiary_first,
                beneficiary_last
            )
        };

        // Collect unique emails
        let staff_email_lower = staff.email.to_lowercase();
        let membership_email_lower = membership_email.to_lowercase();
        let payer_email_lower = payer_email.to_lowercase();

        let mut unique_emails: Vec<(&str, &str, &str)> = Vec::new(); // (value, label, display)

        if !membership_email.is_empty() {
            unique_emails.push(("membership", "Du bénéficiaire", &membership_email));
        }
        if !payer_email.is_empty() && payer_email_lower != membership_email_lower {
            unique_emails.push(("payer", "Du payeur", &payer_email));
        }
        if staff_email_lower != membership_email_lower && staff_email_lower != payer_email_lower {
            unique_emails.push(("staff", "Du staff", &staff.email));
        }

        // Build email choice HTML
        let email_choice_html = if unique_emails.len() <= 1 {
            // Only one email option, no need for choice
            let email_value = if !membership_email.is_empty() {
                &membership_email
            } else if !payer_email.is_empty() {
                &payer_email
            } else {
                &staff.email
            };
            format!(
                r#"<input type="hidden" name="email" value="{}">"#,
                email_value
            )
        } else {
            // Multiple email options
            let mut options_html = String::new();
            let mut first = true;
            for (value, label, display) in &unique_emails {
                let checked = if first { "checked" } else { "" };
                options_html.push_str(&format!(
                    r#"<label class="radio">
                        <input type="radio" name="email_choice" value="{}" {} onchange="updateEmailField(this.form, '{}')">
                        {}: <strong>{}</strong>
                    </label>
                    <br>"#,
                    value, checked, display, label, display
                ));
                first = false;
            }
            let default_email_val = unique_emails
                .first()
                .map_or(default_email.as_str(), |(_, _, d)| *d);
            format!(
                r#"<div class="field">
                    <label class="label">Garder l'email</label>
                    <div class="control">
                        {}
                    </div>
                </div>
                <input type="hidden" name="email" value="{}">"#,
                options_html, default_email_val
            )
        };

        // Alternate background colors for better visual separation
        let bg_color = if option_index.is_multiple_of(2) {
            "var(--bulma-scheme-main)"
        } else {
            "var(--bulma-scheme-main-bis)"
        };
        option_index += 1;

        candidates_html.push_str(&format!(
            r#"
            <div class="box mb-4" style="border: 2px solid {}; background-color: {};">
                <form method="POST">
                    <input type="hidden" name="action" value="update">
                    <input type="hidden" name="staff_id" value="{}">

                    <div class="level mb-3">
                        <div class="level-left">
                            <span class="tag {}">{}</span>
                            {}
                        </div>
                        <div class="level-right">
                            <span class="tag is-info is-light">{}</span>
                        </div>
                    </div>

                    <p class="mb-3"><strong>Staff existant:</strong> {} {} &lt;{}&gt;</p>

                    {}

                    {}

                    <input type="hidden" name="phone" value="{}">

                    <div class="field">
                        <label class="label">Commentaire</label>
                        <div class="control">
                            <textarea class="textarea" name="comment" rows="2">{}</textarea>
                        </div>
                    </div>

                    <div class="field">
                        <div class="control">
                            <button type="submit" class="button {} is-fullwidth">
                                <span class="icon"><i class="fa-solid fa-arrows-rotate"></i></span>
                                <span>Mettre à jour ce staff</span>
                            </button>
                        </div>
                    </div>
                </form>
            </div>
            "#,
            border_color,
            bg_color,
            staff.id,
            highlight,
            match_label,
            recommended,
            season_info,
            staff.first_name,
            staff.last_name,
            staff.email,
            name_choice_html,
            email_choice_html,
            phone,
            comment,
            highlight,
        ));

        is_first = false;
        if candidate.match_type == StaffMatchType::DoubleSubscription {
            is_first_double_subscription = false;
        }
    }

    // Create new staff option
    let create_highlight = if recommend_create {
        "is-primary"
    } else {
        "is-light"
    };
    let create_recommended = if recommend_create {
        r#"<span class="tag is-success ml-2">Probable meilleure option</span>"#
    } else {
        ""
    };
    let create_border = if recommend_create {
        "var(--bulma-primary)"
    } else {
        "var(--bulma-border)"
    };
    let create_bg_color = if option_index.is_multiple_of(2) {
        "var(--bulma-scheme-main)"
    } else {
        "var(--bulma-scheme-main-bis)"
    };

    // Build email choice HTML for "create new staff" form
    let create_email_html = if !membership_email.is_empty()
        && !payer_email.is_empty()
        && membership_email != payer_email
    {
        // Both emails available and different - show radio buttons
        format!(
            r#"
                <div class="field">
                    <label class="label">Email</label>
                    <div class="control">
                        <label class="radio">
                            <input type="radio" name="email_choice" value="membership" checked onchange="document.getElementById('create_email').value='{}'">
                            Du bénéficiaire: <strong>{}</strong>
                        </label>
                        <br>
                        <label class="radio">
                            <input type="radio" name="email_choice" value="payer" onchange="document.getElementById('create_email').value='{}'">
                            Du payeur: <strong>{}</strong>
                        </label>
                    </div>
                    <input type="hidden" id="create_email" name="email" value="{}">
                </div>
            "#,
            membership_email, membership_email, payer_email, payer_email, membership_email
        )
    } else if membership_email.is_empty() && !payer_email.is_empty() {
        // Only payer email available - show it with option to edit
        format!(
            r#"
                <div class="field">
                    <label class="label">Email (du payeur)</label>
                    <div class="control">
                        <input class="input" type="email" name="email" value="{}">
                    </div>
                </div>
            "#,
            payer_email
        )
    } else {
        // Only membership email or same emails - simple input
        format!(
            r#"
                <div class="field">
                    <label class="label">Email</label>
                    <div class="control">
                        <input class="input" type="email" name="email" value="{}">
                    </div>
                </div>
            "#,
            default_email
        )
    };

    let create_html = format!(
        r#"
        <div class="box mb-4" style="border: 2px solid {}; background-color: {};">
            <form method="POST">
                <input type="hidden" name="action" value="create">

                <div class="level mb-3">
                    <div class="level-left">
                        <span class="tag {}">Nouveau staff</span>
                        {}
                    </div>
                </div>

                <div class="columns">
                    <div class="column">
                        <div class="field">
                            <label class="label">Prénom</label>
                            <div class="control">
                                <input class="input" type="text" name="first_name" value="{}">
                            </div>
                        </div>
                    </div>
                    <div class="column">
                        <div class="field">
                            <label class="label">Nom</label>
                            <div class="control">
                                <input class="input" type="text" name="last_name" value="{}">
                            </div>
                        </div>
                    </div>
                </div>

                {}

                <div class="field">
                    <label class="label">Téléphone</label>
                    <div class="control">
                        <input class="input" type="tel" name="phone" value="{}">
                    </div>
                </div>

                <div class="field">
                    <label class="label">Commentaire</label>
                    <div class="control">
                        <textarea class="textarea" name="comment" rows="2">{}</textarea>
                    </div>
                </div>

                <div class="field">
                    <div class="control">
                        <button type="submit" class="button {} is-fullwidth">
                            <span class="icon"><i class="fa-solid fa-plus"></i></span>
                            <span>Créer un nouveau staff</span>
                        </button>
                    </div>
                </div>
            </form>
        </div>
        "#,
        create_border,
        create_bg_color,
        create_highlight,
        create_recommended,
        beneficiary_first,
        beneficiary_last,
        create_email_html,
        phone,
        comment,
        create_highlight,
    );

    // Combine options in the right order based on recommendation
    let options_html = if !allow_create {
        // Name already exists, only show candidates
        candidates_html.clone()
    } else if recommend_create {
        // "Create new" is recommended, show it first
        format!("{}{}", create_html, candidates_html)
    } else {
        // Exact match found, show candidates first
        format!("{}{}", candidates_html, create_html)
    };

    // Count total options available
    let total_options = candidates.len() + usize::from(allow_create);

    // Warning notification if there are multiple options
    let multiple_options_warning = if total_options > 1 {
        r#"<div class="notification is-danger mb-4">
            <span class="icon"><i class="fa-solid fa-triangle-exclamation"></i></span>
            <strong>Attention</strong>, il y a plusieurs possibilités, examinez-les bien avant de choisir la bonne.
        </div>"#
    } else {
        ""
    };

    let extra_head = r#"<script>
        function updateNameFields(form, firstName, lastName) {
            form.querySelector('input[name="first_name"]').value = firstName;
            form.querySelector('input[name="last_name"]').value = lastName;
        }
        function updateEmailField(form, email) {
            form.querySelector('input[name="email"]').value = email;
        }
    </script>"#;

    let membership_email_display = if membership_email.is_empty() {
        "N/A"
    } else {
        &membership_email
    };

    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="level mb-5">
                <div class="level-left">
                    <h1 class="title is-3">Importer un Staff</h1>
                </div>
                <div class="level-right">
                    <a href="{p}/users" class="button is-light">
                        <span class="icon"><i class="fa-solid fa-arrow-left"></i></span>
                        <span>Retour</span>
                    </a>
                </div>
            </div>

            <div class="columns">
                <div class="column is-5">
                    <div class="box">
                        <h2 class="title is-4 mb-4">Détails de l'Adhésion</h2>
                        <div class="content">
                            <table class="table is-fullwidth">
                                <tbody>
                                    <tr>
                                        <th>Bénéficiaire</th>
                                        <td><strong>{beneficiary_name}</strong></td>
                                    </tr>
                                    <tr>
                                        <th>Email bénéficiaire</th>
                                        <td>{membership_email_display}</td>
                                    </tr>
                                    <tr>
                                        <th>Email payeur</th>
                                        <td>{payer_email}</td>
                                    </tr>
                                    <tr>
                                        <th>Téléphone</th>
                                        <td>{phone}</td>
                                    </tr>
                                    <tr>
                                        <th>Article</th>
                                        <td>{item_name}</td>
                                    </tr>
                                    <tr>
                                        <th>Montant</th>
                                        <td>{amount}</td>
                                    </tr>
                                    <tr>
                                        <th>Date</th>
                                        <td>{order_date}</td>
                                    </tr>
                                    <tr>
                                        <th>Saison</th>
                                        <td><span class="tag is-info is-medium">{season}</span></td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>

                <div class="column is-7">
                    <h2 class="title is-4 mb-4">Options d'import</h2>
                    {multiple_options_warning}
                    {options_html}
                </div>
            </div>
        </div>
    </section>"#,
        p = prefix,
        beneficiary_name = beneficiary_name,
        membership_email_display = membership_email_display,
        payer_email = &payer_email,
        phone = phone,
        item_name = item_name,
        amount = amount,
        order_date = order_date,
        season = season,
        multiple_options_warning = multiple_options_warning,
        options_html = options_html
    );

    page(
        "Importer Staff - AGHIL",
        prefix,
        &NavKind::Standard,
        "",
        extra_head,
        &content,
        "",
    )
}

pub fn user_detail(user: User, prefix: &str) -> String {
    let full_name = format!(
        "{} {}",
        user.first_name.as_deref().unwrap_or(""),
        user.last_name.as_deref().unwrap_or("")
    )
    .trim()
    .to_string();

    let email = &user.email;
    let phone = user.phone.as_deref().unwrap_or("N/A");
    let address = {
        let addr = format!(
            "{} {}",
            user.address.as_deref().unwrap_or(""),
            user.city.as_deref().unwrap_or("")
        );
        addr.trim().to_string()
    };
    let zip_code = user.zip_code.as_deref().unwrap_or("");
    let country = user.country.as_deref().unwrap_or("");
    let birth_date = user
        .birth_date
        .map_or_else(|| "N/A".to_string(), |d| d.format("%d/%m/%Y").to_string());
    let created_at = user.created_at.format("%d/%m/%Y à %H:%M").to_string();
    let updated_at = user.updated_at.format("%d/%m/%Y à %H:%M").to_string();
    let last_sync = user.last_sync_at.map_or_else(
        || "Jamais".to_string(),
        |d| d.format("%d/%m/%Y à %H:%M").to_string(),
    );

    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="level mb-5">
                <div class="level-left">
                    <h1 class="title is-3">Détails de l'Utilisateur</h1>
                </div>
                <div class="level-right">
                    <a href="{p}/users" class="button is-light">
                        <span class="icon"><i class="fa-solid fa-arrow-left"></i></span>
                        <span>Retour à la liste</span>
                    </a>
                </div>
            </div>

            <div class="box">
                <div class="columns">
                    <div class="column is-4">
                        <div class="has-text-centered">
                            <div class="avatar-circle is-size-1 mb-4" style="width: 120px; height: 120px; margin: 0 auto;">
                                <span class="icon is-large"><i class="fa-solid fa-user fa-3x"></i></span>
                            </div>
                            <h2 class="title is-4">{full_name}</h2>
                            <p class="subtitle is-6 has-text-grey">{email}</p>
                        </div>
                    </div>
                    <div class="column is-8">
                        <div class="content">
                            <h3 class="title is-5 mb-3">Informations Personnelles</h3>
                            <div class="columns is-multiline">
                                <div class="column is-6">
                                    <div class="field">
                                        <label class="label">Téléphone</label>
                                        <div class="control">{phone}</div>
                                    </div>
                                </div>
                                <div class="column is-6">
                                    <div class="field">
                                        <label class="label">Date de naissance</label>
                                        <div class="control">{birth_date}</div>
                                    </div>
                                </div>
                                <div class="column is-12">
                                    <div class="field">
                                        <label class="label">Adresse</label>
                                        <div class="control">{address}<br>{zip_code} {country}</div>
                                    </div>
                                </div>
                            </div>

                            <h3 class="title is-5 mb-3 mt-5">Informations Système</h3>
                            <div class="columns is-multiline">
                                <div class="column is-6">
                                    <div class="field">
                                        <label class="label">Email</label>
                                        <div class="control">{email}</div>
                                    </div>
                                </div>
                                <div class="column is-6">
                                    <div class="field">
                                        <label class="label">Créé le</label>
                                        <div class="control">{created_at}</div>
                                    </div>
                                </div>
                                <div class="column is-6">
                                    <div class="field">
                                        <label class="label">Dernière mise à jour</label>
                                        <div class="control">{updated_at}</div>
                                    </div>
                                </div>
                                <div class="column is-6">
                                    <div class="field">
                                        <label class="label">Dernière synchronisation</label>
                                        <div class="control">{last_sync}</div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </section>"#,
        p = prefix,
        full_name = full_name,
        email = email,
        phone = phone,
        birth_date = birth_date,
        address = address,
        zip_code = zip_code,
        country = country,
        created_at = created_at,
        updated_at = updated_at,
        last_sync = last_sync
    );

    let title = format!("Détails de l'Utilisateur - {}", full_name);
    page(&title, prefix, &NavKind::Full, "", "", &content, "")
}

pub fn restore_page(prefix: &str) -> String {
    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="columns is-centered">
                <div class="column is-8">
                    <div class="box">
                        <h1 class="title is-3 has-text-centered">
                            <span class="icon has-text-warning"><i class="fa-solid fa-upload"></i></span>
                            Restaurer la base de données
                        </h1>

                        <div class="notification is-warning is-light">
                            <p><strong>Attention:</strong> Cette opération va remplacer toutes les données actuelles par celles du fichier de sauvegarde.</p>
                        </div>

                        <form method="POST" enctype="multipart/form-data" action="{p}/restore">
                            <div class="field">
                                <label class="label">Fichier de sauvegarde (.sql)</label>
                                <div class="control">
                                    <div class="file has-name is-fullwidth is-boxed">
                                        <label class="file-label">
                                            <input class="file-input" type="file" name="backup_file" accept=".sql" required onchange="updateFileName(this)">
                                            <span class="file-cta">
                                                <span class="file-icon">
                                                    <i class="fa-solid fa-upload"></i>
                                                </span>
                                                <span class="file-label">Choisir un fichier...</span>
                                            </span>
                                            <span class="file-name" id="file-name">Aucun fichier sélectionné</span>
                                        </label>
                                    </div>
                                </div>
                            </div>

                            <div class="field is-grouped is-grouped-centered mt-5">
                                <div class="control">
                                    <button type="submit" class="button is-danger is-medium">
                                        <span class="icon"><i class="fa-solid fa-database"></i></span>
                                        <span>Restaurer la base de données</span>
                                    </button>
                                </div>
                                <div class="control">
                                    <a href="{p}/" class="button is-light is-medium">
                                        <span class="icon"><i class="fa-solid fa-xmark"></i></span>
                                        <span>Annuler</span>
                                    </a>
                                </div>
                            </div>
                        </form>
                    </div>

                    <div class="box">
                        <h2 class="title is-5">
                            <span class="icon has-text-info"><i class="fa-solid fa-download"></i></span>
                            Créer une sauvegarde
                        </h2>
                        <p class="mb-4">Téléchargez une copie de la base de données actuelle avant de restaurer.</p>
                        <a href="{p}/backup" class="button is-info">
                            <span class="icon"><i class="fa-solid fa-download"></i></span>
                            <span>Télécharger la sauvegarde</span>
                        </a>
                    </div>
                </div>
            </div>
        </div>
    </section>"#,
        p = prefix
    );

    let scripts = r"    <script>
        function updateFileName(input) {
            const fileName = input.files[0] ? input.files[0].name : 'Aucun fichier sélectionné';
            document.getElementById('file-name').textContent = fileName;
        }
    </script>";

    page(
        "Restaurer la base de données - AGHIL",
        prefix,
        &NavKind::Standard,
        "",
        "",
        &content,
        scripts,
    )
}

pub fn restore_result(prefix: &str, success: bool, message: &str) -> String {
    let (icon_class, title, notification_class) = if success {
        ("has-text-success", "Restauration réussie", "is-success")
    } else {
        ("has-text-danger", "Erreur de restauration", "is-danger")
    };

    let icon = if success {
        "check-circle"
    } else {
        "exclamation-triangle"
    };

    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="columns is-centered">
                <div class="column is-8">
                    <div class="box has-text-centered">
                        <span class="icon is-large {icon_class} mb-4">
                            <i class="fa-solid fa-{icon} fa-4x"></i>
                        </span>
                        <h1 class="title is-3">{title}</h1>
                        <div class="notification {notification_class} is-light">
                            <p>{message}</p>
                        </div>
                        <div class="buttons is-centered mt-5">
                            <a href="{p}/" class="button is-primary is-medium">
                                <span class="icon"><i class="fa-solid fa-house"></i></span>
                                <span>Retour à l'accueil</span>
                            </a>
                            <a href="{p}/users" class="button is-info is-medium">
                                <span class="icon"><i class="fa-solid fa-users"></i></span>
                                <span>Voir les adhésions</span>
                            </a>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </section>"#,
        p = prefix,
        title = title,
        icon_class = icon_class,
        icon = icon,
        notification_class = notification_class,
        message = message
    );

    let page_title = format!("{} - AGHIL", title);
    page(
        &page_title,
        prefix,
        &NavKind::Standard,
        "",
        "",
        &content,
        "",
    )
}

pub fn import_result(success: bool, message: &str, prefix: &str) -> String {
    let (title, icon, notification_class) = if success {
        ("Import réussi", "check-circle", "is-success")
    } else {
        ("Erreur d'import", "exclamation-triangle", "is-danger")
    };

    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="notification {notification_class}">
                <p class="title is-4">
                    <span class="icon"><i class="fa-solid fa-{icon}"></i></span>
                    {title}
                </p>
                <p>{message}</p>
            </div>
            <div class="buttons mt-4">
                <a href="{p}/users?filter=not_imported" class="button is-primary">
                    <span class="icon"><i class="fa-solid fa-arrow-left"></i></span>
                    <span>Retour aux adhésions à importer</span>
                </a>
                <a href="{p}/users" class="button is-light">
                    <span class="icon"><i class="fa-solid fa-list"></i></span>
                    <span>Voir toutes les adhésions</span>
                </a>
            </div>
        </div>
    </section>"#,
        p = prefix,
        title = title,
        icon = icon,
        notification_class = notification_class,
        message = message
    );

    let page_title = format!("{} - AGHIL", title);
    page(
        &page_title,
        prefix,
        &NavKind::Standard,
        "",
        "",
        &content,
        "",
    )
}

pub fn staff_list(
    staff_with_seasons: Vec<(Staff, Option<i16>)>,
    ateliers: &[Atelier],
    roles: &[Role],
    current_season: i16,
    prefix: &str,
    show_contact: bool,
) -> String {
    let mut rows_html = String::new();
    let staff_count = staff_with_seasons.len();

    let contact_headers = if show_contact {
        "<th>Email</th><th>Téléphone</th>"
    } else {
        ""
    };

    // Build atelier header columns
    let mut atelier_headers = String::new();
    for atelier in ateliers {
        atelier_headers.push_str(&format!(
            r#"<th class="has-text-centered atelier-col"><span class="vertical-text">{name}</span></th>"#,
            name = atelier.name
        ));
    }

    for (staff, latest_season) in staff_with_seasons {
        let full_name = format!("{} {}", staff.first_name, staff.last_name);
        let phone = if show_contact {
            staff.phone.as_deref().unwrap_or("")
        } else {
            ""
        };
        let email_display = if show_contact {
            staff.email.as_str()
        } else {
            ""
        };
        let comment_display = staff.comment.clone();

        let (season_tag_class, season_display) = match latest_season {
            Some(s) if s == current_season => ("is-success", s.to_string()),
            Some(s) => ("is-danger", s.to_string()),
            None => ("is-light", "—".to_string()),
        };

        // Grey out row if season is not current
        let row_class = match latest_season {
            Some(s) if s == current_season => "",
            _ => "inactive-staff",
        };

        // Build atelier cells for this staff member
        let mut atelier_cells = String::new();
        for atelier in ateliers {
            let role = roles
                .iter()
                .find(|r| r.staff == staff.id && r.atelier == atelier.id);
            let (cell_class, cell_content) = match role {
                Some(r) if r.chief => (
                    "has-background-warning",
                    r#"<span class="icon has-text-black"><i class="fa-solid fa-crown"></i></span>"#,
                ),
                Some(r) if r.validated => (
                    "has-background-info",
                    r#"<span class="icon has-text-white"><i class="fa-solid fa-check"></i></span>"#,
                ),
                Some(_) => (
                    "has-background-grey",
                    r#"<span class="icon has-text-grey-dark"><i class="fa-solid fa-clock"></i></span>"#,
                ),
                None => ("", ""),
            };
            atelier_cells.push_str(&format!(
                r#"<td class="has-text-centered atelier-col {}">{}</td>"#,
                cell_class, cell_content
            ));
        }

        // Admin column
        let admin_cell = if staff.is_god {
            r#"<td class="has-text-centered has-background-warning"><span class="icon has-text-black"><i class="fa-solid fa-crown"></i></span></td>"#.to_string()
        } else if staff.is_admin {
            r#"<td class="has-text-centered has-background-info"><span class="icon has-text-white"><i class="fa-solid fa-check"></i></span></td>"#.to_string()
        } else {
            r"<td></td>".to_string()
        };

        let contact_cells = if show_contact {
            format!(r"<td>{}</td><td>{}</td>", email_display, phone)
        } else {
            String::new()
        };

        rows_html.push_str(&format!(
            r#"
                <tr class="{row_class}">
                    <td><a href="{p}/person/{id}"><strong>{full_name}</strong></a></td>
                    {contact_cells}
                    <td><span class="tag {season_tag_class}">{season_display}</span></td>
                    {atelier_cells}
                    {admin_cell}
                    <td><small>{comment}</small></td>
                </tr>"#,
            row_class = row_class,
            p = prefix,
            id = staff.id,
            full_name = full_name,
            contact_cells = contact_cells,
            season_tag_class = season_tag_class,
            season_display = season_display,
            atelier_cells = atelier_cells,
            admin_cell = admin_cell,
            comment = comment_display
        ));
    }

    let extra_head = r"    <style>
        .table-container {
            overflow: auto;
            max-height: calc(100vh - 300px);
        }
        .atelier-col {
            border-left: 1px solid var(--bulma-border) !important;
            border-right: 1px solid var(--bulma-border) !important;
        }
        thead th {
            vertical-align: bottom !important;
            position: sticky;
            top: 0;
            background: var(--bulma-scheme-main) !important;
            z-index: 10;
        }
        th.atelier-col {
            background: var(--bulma-scheme-main) !important;
            padding: 8px 4px !important;
            min-width: 30px;
            max-width: 30px;
        }
        th.atelier-col .vertical-text {
            writing-mode: vertical-rl;
            text-orientation: mixed;
            font-size: 0.85em;
            line-height: 1.2;
        }
        tr.inactive-staff {
            opacity: 0.4;
        }
        @media screen and (min-width: 769px) {
            .table td, .table th {
                white-space: nowrap;
            }
        }
    </style>";

    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="level mb-4">
                <div class="level-left">
                    <h1 class="title is-3">
                        <span class="icon"><i class="fa-solid fa-user-group"></i></span>
                        Liste des Staff
                    </h1>
                </div>
                <div class="level-right">
                    <span class="tag is-info is-medium">{staff_count} membres</span>
                </div>
            </div>

            <div class="notification is-info is-light mb-4">
                <p>
                    <span class="icon"><i class="fa-solid fa-circle-info"></i></span>
                    <strong>Légende saison:</strong>
                    <span class="tag is-success ml-2">Saison courante ({current_season})</span>
                    <span class="tag is-danger ml-2">Saison précédente</span>
                    <span class="tag is-light ml-2">Aucun paiement</span>
                </p>
                <p class="mt-2">
                    <strong>Légende ateliers:</strong>
                    <span class="tag is-warning ml-2"><span class="icon"><i class="fa-solid fa-crown"></i></span> Chef</span>
                    <span class="tag is-info ml-2"><span class="icon"><i class="fa-solid fa-check"></i></span> Validé</span>
                    <span class="tag is-grey ml-2"><span class="icon"><i class="fa-solid fa-clock"></i></span> En attente</span>
                </p>
            </div>

            <div class="box">
                <div class="table-container">
                    <table class="table is-fullwidth is-striped is-hoverable">
                        <thead>
                            <tr>
                                <th>Nom</th>
                                {contact_headers}
                                <th class="has-text-centered atelier-col"><span class="vertical-text">Dernière saison</span></th>
                                {atelier_headers}
                                <th class="has-text-centered atelier-col"><span class="vertical-text">Admin</span></th>
                                <th>Commentaire</th>
                            </tr>
                        </thead>
                        <tbody>
                            {rows_html}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    </section>"#,
        staff_count = staff_count,
        current_season = current_season,
        atelier_headers = atelier_headers,
        rows_html = rows_html
    );

    page(
        "Liste des Staff - AGHIL",
        prefix,
        &NavKind::Standard,
        "staff",
        extra_head,
        &content,
        "",
    )
}

#[allow(clippy::too_many_arguments)]
pub fn person_detail(
    staff: &Staff,
    ateliers: &[Atelier],
    roles: &[Role],
    current_season: i16,
    prefix: &str,
    is_self: bool,
    is_admin: bool,
    show_contact: bool,
    todos: &[TodoItem],
    payment_history: &[crate::models::PaymentHistoryEntry],
    person_calendar: &[(crate::models::Need, String, String, String, bool, bool)],
) -> String {
    let can_edit_ateliers = is_self || is_admin;
    let mut ateliers_html = String::new();

    for atelier in ateliers {
        let role = roles.iter().find(|r| r.atelier == atelier.id);
        let is_checked = role.is_some();
        let checked_attr = if is_checked { "checked" } else { "" };
        let disabled_attr = if can_edit_ateliers { "" } else { "disabled" };
        let validation_badge = if atelier.needs_validation {
            r#"<span class="tag is-warning is-light ml-2">Validation requise</span>"#
        } else {
            ""
        };

        // Validated/chief checkboxes: admin only; or read-only badge for self
        let role_options = if let Some(r) = role {
            if is_admin {
                let validated_checked = if r.validated || r.chief {
                    "checked"
                } else {
                    ""
                };
                let validated_disabled = if r.chief { "disabled" } else { "" };
                let chief_checked = if r.chief { "checked" } else { "" };
                format!(
                    r#"
                    <div class="ml-5 mt-1">
                        <label class="checkbox mr-4">
                            <input type="checkbox"
                                   class="role-validated-checkbox"
                                   data-atelier-id="{atelier_id}"
                                   {validated_checked} {validated_disabled}>
                            <span class="icon has-text-info"><i class="fa-solid fa-check"></i></span>
                            <span>Validé</span>
                        </label>
                        <label class="checkbox">
                            <input type="checkbox"
                                   class="role-chief-checkbox"
                                   data-atelier-id="{atelier_id}"
                                   {chief_checked}>
                            <span class="icon has-text-warning"><i class="fa-solid fa-crown"></i></span>
                            <span>Chef</span>
                        </label>
                    </div>"#,
                    atelier_id = atelier.id,
                    validated_checked = validated_checked,
                    validated_disabled = validated_disabled,
                    chief_checked = chief_checked
                )
            } else {
                // Non-admin: show read-only validation status badge
                if r.chief {
                    r#"<span class="tag is-warning ml-5 mt-1"><i class="fa-solid fa-crown mr-1"></i> Chef</span>"#.to_string()
                } else if r.validated {
                    r#"<span class="tag is-success ml-5 mt-1"><i class="fa-solid fa-check mr-1"></i> Validé</span>"#.to_string()
                } else if atelier.needs_validation {
                    r#"<span class="tag is-warning is-light ml-5 mt-1"><i class="fa-solid fa-clock mr-1"></i> En attente de validation</span>"#.to_string()
                } else {
                    String::new()
                }
            }
        } else {
            String::new()
        };

        ateliers_html.push_str(&format!(
            r#"
            <div class="field mb-4">
                <div class="is-flex is-align-items-center">
                    <label class="checkbox is-flex is-align-items-center">
                        <input type="checkbox"
                               class="atelier-checkbox mr-2"
                               data-atelier-id="{atelier_id}"
                               {checked} {disabled}>
                        <span class="is-size-5">{name}</span>
                        {validation_badge}
                    </label>
                </div>
                {role_options}
            </div>"#,
            atelier_id = atelier.id,
            checked = checked_attr,
            disabled = disabled_attr,
            name = atelier.name,
            validation_badge = validation_badge,
            role_options = role_options,
        ));
    }

    // Build "Mes plannings" section: links to calendars for ateliers the staff has a role in
    let mut plannings_html = String::new();
    for atelier in ateliers {
        let has_role = roles.iter().any(|r| r.atelier == atelier.id);
        if has_role && !atelier.slug.is_empty() {
            plannings_html.push_str(&format!(
                r#"<a href="{p}/calendar/{slug}" class="button is-link is-outlined mr-2 mb-2">
                    <span class="icon"><i class="fa-solid fa-{icon}"></i></span>&nbsp;
                    <span>{name}</span>
                </a>"#,
                p = prefix,
                slug = atelier.slug,
                icon = atelier.icon,
                name = atelier.name,
            ));
        }
    }

    let plannings_box = if plannings_html.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="box">
                        <h2 class="title is-4">
                            <span class="icon"><i class="fa-solid fa-calendar-days"></i></span>
                            Mes plannings
                        </h2>
                        <div class="buttons">
                            {plannings_html}
                        </div>
                    </div>"#,
            plannings_html = plannings_html,
        )
    };

    let can_edit_contact = is_self || is_admin;
    let contact_html = if can_edit_contact && show_contact {
        let phone_value = staff.phone.as_deref().unwrap_or("");
        format!(
            r#"<div class="field">
                                <label class="label">Email:</label>
                                <div class="control has-icons-left">
                                    <input class="input" type="email" id="edit-email" value="{email}">
                                    <span class="icon is-left"><i class="fa-solid fa-envelope"></i></span>
                                </div>
                            </div>
                            <div class="field">
                                <label class="label">Téléphone:</label>
                                <div class="control has-icons-left">
                                    <input class="input" type="tel" id="edit-phone" value="{phone}">
                                    <span class="icon is-left"><i class="fa-solid fa-phone"></i></span>
                                </div>
                            </div>
                            <div class="control mt-2">
                                <button class="button is-small is-info" id="save-contact-btn">
                                    <span class="icon"><i class="fa-solid fa-floppy-disk"></i></span>
                                    <span>Enregistrer</span>
                                </button>
                            </div>"#,
            email = staff.email,
            phone = phone_value,
        )
    } else if show_contact {
        let phone_display = staff.phone.as_deref().unwrap_or("—");
        format!(
            r#"<p>
                                <strong>Email:</strong><br>
                                <a href="mailto:{email}">{email}</a>
                            </p>
                            <p>
                                <strong>Téléphone:</strong><br>
                                {phone}
                            </p>"#,
            email = staff.email,
            phone = phone_display,
        )
    } else {
        String::new()
    };

    let comment_display = if staff.comment.is_empty() {
        "—"
    } else {
        &staff.comment
    };

    // Comment section: editable for admins, read-only for others
    let comment_html = if is_admin {
        format!(
            r#"<div class="field">
                                <label class="label">Commentaire:</label>
                                <div class="control">
                                    <textarea class="textarea" id="comment-input" rows="3">{comment}</textarea>
                                </div>
                                <div class="control mt-2">
                                    <button class="button is-small is-info" id="save-comment-btn">
                                        <span class="icon"><i class="fa-solid fa-floppy-disk"></i></span>
                                        <span>Enregistrer</span>
                                    </button>
                                </div>
                            </div>"#,
            comment = comment_display
        )
    } else if !staff.comment.is_empty() {
        format!(
            r"<p>
                                <strong>Commentaire:</strong><br>
                                {comment}
                            </p>",
            comment = comment_display
        )
    } else {
        String::new()
    };

    // Admin/god box: only visible to admins
    let admin_box_html = if is_admin {
        let admin_checked = if staff.is_admin { "checked" } else { "" };
        let god_checked = if staff.is_god { "checked" } else { "" };
        format!(
            r#"<div class="box">
                        <h2 class="title is-4">
                            <span class="icon"><i class="fa-solid fa-shield-halved"></i></span>
                            Administration
                        </h2>
                        <div class="field mb-3">
                            <label class="checkbox">
                                <input type="checkbox" id="admin-cb" {admin_checked}>
                                <span class="icon has-text-info"><i class="fa-solid fa-check"></i></span>
                                <span>Admin</span>
                            </label>
                        </div>
                        <div class="field">
                            <label class="checkbox">
                                <input type="checkbox" id="god-cb" {god_checked}>
                                <span class="icon has-text-warning"><i class="fa-solid fa-crown"></i></span>
                                <span>God</span>
                            </label>
                        </div>
                    </div>"#,
            admin_checked = admin_checked,
            god_checked = god_checked
        )
    } else {
        String::new()
    };

    // Info text depends on who is viewing
    let info_text = if is_admin {
        "Cochez les ateliers auxquels ce membre participe pour la saison en cours."
    } else if is_self {
        "Cochez les ateliers auxquels vous participez pour la saison en cours."
    } else {
        "Ateliers auxquels ce membre participe pour la saison en cours."
    };

    // Build TODO box
    let todo_html = if todos.is_empty() {
        String::new()
    } else {
        let mut items_html = String::new();
        for item in todos {
            items_html.push_str(&format!(
                r#"<li class="mb-2">
                    <span class="icon has-text-{color}"><i class="fa-solid {icon}"></i></span>
                    {html}
                </li>"#,
                color = item.color,
                icon = item.icon,
                html = item.html,
            ));
        }
        format!(
            r#"<div class="box mb-4" style="border-left: 4px solid var(--bulma-danger);">
                <h2 class="title is-5">
                    <span class="icon has-text-danger"><i class="fa-solid fa-clipboard-list"></i></span>
                    À faire
                </h2>
                <ul class="ml-2" style="list-style:none;">{items_html}</ul>
            </div>"#,
            items_html = items_html,
        )
    };

    // Build payment history section
    let payment_history_html = if payment_history.is_empty() {
        String::new()
    } else {
        let mut items_html = String::new();
        for entry in payment_history {
            let icon = match entry.source.as_str() {
                "helloasso" => {
                    r#"<span class="icon has-text-link"><i class="fa-solid fa-ticket"></i></span>"#
                }
                "check" => {
                    r#"<span class="icon has-text-success"><i class="fa-solid fa-money-check"></i></span>"#
                }
                _ => {
                    r#"<span class="icon has-text-warning"><i class="fa-solid fa-coins"></i></span>"#
                }
            };
            let date_display = entry.date.as_deref().unwrap_or("—");
            let amount_display = entry.amount.map_or_else(
                || "—".to_string(),
                |a| {
                    if entry.source == "helloasso" {
                        format!("{:.2}€", a as f32 / 100.0)
                    } else {
                        format!("{}€", a)
                    }
                },
            );
            let name = format!(
                "{} {}",
                capitalize_words(&entry.first_name),
                capitalize_words(&entry.last_name)
            );
            let email_display = entry.email.as_deref().unwrap_or("—");
            let phone_display = entry
                .phone
                .as_deref()
                .map_or_else(|| "—".to_string(), format_phone_international);
            let payer_line = if let Some(ref payer) = entry.payer_email {
                if entry.email.as_deref() == Some(payer.as_str()) {
                    String::new()
                } else {
                    format!(
                        r#"<span class="is-size-7 has-text-grey">Payeur: {}</span><br>"#,
                        payer
                    )
                }
            } else {
                String::new()
            };

            let item_type = format!(
                "{} {}",
                entry.item_type,
                match entry.source.as_str() {
                    "helloasso" => "HelloAsso",
                    "check" => "Chèque",
                    _ => "Liquide",
                }
            );
            items_html.push_str(&format!(
                r#"<div class="box mb-3 p-3">
                    <div class="columns is-mobile is-vcentered is-multiline">
                        <div class="column is-narrow">
                            {icon}
                        </div>
                        <div class="column">
                            <strong>{item_type}</strong> — Saison {season}<br>
                            <span class="is-size-7 has-text-grey">{date} — {amount}</span>
                        </div>
                        <div class="column is-5-tablet is-12-mobile">
                            <span class="is-size-7">{name}</span><br>
                            <span class="is-size-7">{email}</span><br>
                            <span class="is-size-7">{phone}</span><br>
                            {payer_line}
                        </div>
                    </div>
                </div>"#,
                icon = icon,
                item_type = item_type,
                season = entry.season,
                date = date_display,
                amount = amount_display,
                name = name,
                email = email_display,
                phone = phone_display,
                payer_line = payer_line,
            ));
        }
        format!(
            r#"<div class="box">
                        <h2 class="title is-4">
                            <span class="icon"><i class="fa-solid fa-clock-rotate-left"></i></span>
                            Historique des cotisations
                        </h2>
                        {items_html}
                    </div>"#,
            items_html = items_html,
        )
    };

    let extra_head = r"    <style>
        .atelier-checkbox {
            width: 1.25rem;
            height: 1.25rem;
        }
        .checkbox:hover {
            background-color: var(--bulma-background);
            border-radius: 4px;
            padding: 0.5rem;
            margin: -0.5rem;
        }
        .notification.is-loading {
            position: fixed;
            top: 20px;
            left: 50%;
            transform: translateX(-50%);
            z-index: 100;
            min-width: 300px;
        }
        .pcal-scroll { overflow-x: auto; }
        .pcal-table { border-collapse: collapse; white-space: nowrap; }
        .pcal-table th, .pcal-table td { border: 1px solid var(--bulma-border); padding: 0.3rem 0.4rem; vertical-align: middle; }
        .pcal-table thead th { background: var(--bulma-scheme-main-bis) !important; }
        .pcal-atelier-col { position: sticky; left: 0; z-index: 2; min-width: 140px; background: var(--bulma-scheme-main); }
        .pcal-table thead th.pcal-atelier-col { z-index: 3; background: var(--bulma-scheme-main-bis) !important; }
        .pcal-sunday { background: var(--bulma-link-light) !important; }
        .pcal-cell.pcal-active { background: var(--bulma-success) !important; color: var(--bulma-success-invert); }
        .pcal-day-col { min-width: 70px; }
        .pcal-day-name { font-size: 0.75rem; }
        .pcal-day-date { font-size: 0.85rem; font-weight: 600; }
        .pcal-check { display: inline-flex; align-items: center; gap: 1px; margin: 0 2px; cursor: pointer; font-size: 0.7rem; }
        .pcal-check input { width: 1rem; height: 1rem; margin: 0; }
        .pcal-cell { white-space: nowrap; }
    </style>";

    // Build calendar widget (self: editable, admin/chief: read-only)
    let my_calendar_html = if person_calendar.is_empty() {
        String::new()
    } else {
        // Group by day, then by atelier within each day
        let mut days: Vec<chrono::NaiveDate> = person_calendar
            .iter()
            .map(|(n, _, _, _, _, _)| n.day)
            .collect();
        days.sort();
        days.dedup();

        // Collect unique ateliers (preserving order of first appearance)
        let mut atelier_order: Vec<(uuid::Uuid, String, String, String)> = Vec::new();
        for (need, name, slug, icon, _, _) in person_calendar {
            if !atelier_order
                .iter()
                .any(|(id, _, _, _)| *id == need.atelier)
            {
                atelier_order.push((need.atelier, name.clone(), slug.clone(), icon.clone()));
            }
        }

        // Build header row (days)
        let mut header_html = String::from(r#"<th class="pcal-atelier-col">Atelier</th>"#);
        for day in &days {
            let day_abbrev = day.format("%a").to_string();
            let day_name = match day_abbrev.as_str() {
                "Mon" => "lun.",
                "Tue" => "mar.",
                "Wed" => "mer.",
                "Thu" => "jeu.",
                "Fri" => "ven.",
                "Sat" => "sam.",
                "Sun" => "dim.",
                _ => &day_abbrev,
            };
            let day_date = day.format("%d/%m").to_string();
            let sunday_class = if day.weekday() == chrono::Weekday::Sun {
                " pcal-sunday"
            } else {
                ""
            };
            header_html.push_str(&format!(
                r#"<th class="pcal-day-col has-text-centered{sunday_class}"><div class="pcal-day-name">{day_name}</div><div class="pcal-day-date">{day_date}</div></th>"#,
                sunday_class = sunday_class,
                day_name = day_name,
                day_date = day_date,
            ));
        }

        // Build body rows (one per atelier)
        let mut rows_html = String::new();
        for (atelier_id, atelier_name, atelier_slug, atelier_icon) in &atelier_order {
            rows_html.push_str(&format!(
                r#"<tr><td class="pcal-atelier-col"><a href="{p}/calendar/{slug}"><span class="icon"><i class="fa-solid fa-{icon}"></i></span>&nbsp;{name}</a></td>"#,
                p = prefix,
                slug = atelier_slug,
                icon = atelier_icon,
                name = atelier_name,
            ));

            for day in &days {
                // Find the need for this atelier+day
                if let Some((need, _, _, _, first_half, second_half)) = person_calendar
                    .iter()
                    .find(|(n, _, _, _, _, _)| n.atelier == *atelier_id && n.day == *day)
                {
                    let (first_label, second_label) = if need.nightly {
                        ("soir", "nuit")
                    } else {
                        ("matin", "a-m")
                    };
                    let active_class = if *first_half || *second_half {
                        " pcal-active"
                    } else {
                        ""
                    };
                    let sunday_class = if day.weekday() == chrono::Weekday::Sun {
                        " pcal-sunday"
                    } else {
                        ""
                    };

                    if is_self {
                        let first_checked = if *first_half { "checked" } else { "" };
                        let second_checked = if *second_half { "checked" } else { "" };
                        rows_html.push_str(&format!(
                            r#"<td class="pcal-cell has-text-centered{active_class}{sunday_class}">
                                <label class="pcal-check" title="{first_title}">
                                    <input type="checkbox" class="pcal-presence-cb" data-need="{need_id}" data-staff="{staff_id}" data-half="first" {first_checked}>
                                    <span>{first_label}</span>
                                </label>
                                <label class="pcal-check" title="{second_title}">
                                    <input type="checkbox" class="pcal-presence-cb" data-need="{need_id}" data-staff="{staff_id}" data-half="second" {second_checked}>
                                    <span>{second_label}</span>
                                </label>
                            </td>"#,
                            active_class = active_class,
                            sunday_class = sunday_class,
                            first_title = if need.nightly { "Soirée" } else { "Matin" },
                            second_title = if need.nightly { "Nuit" } else { "Après-midi" },
                            need_id = need.id,
                            staff_id = staff.id,
                            first_checked = first_checked,
                            second_checked = second_checked,
                            first_label = first_label,
                            second_label = second_label,
                        ));
                    } else {
                        // Read-only view: show check marks instead of checkboxes
                        let first_icon = if *first_half {
                            r#"<span class="icon has-text-success"><i class="fa-solid fa-check"></i></span>"#
                        } else {
                            r#"<span class="icon has-text-grey-lighter"><i class="fa-solid fa-xmark"></i></span>"#
                        };
                        let second_icon = if *second_half {
                            r#"<span class="icon has-text-success"><i class="fa-solid fa-check"></i></span>"#
                        } else {
                            r#"<span class="icon has-text-grey-lighter"><i class="fa-solid fa-xmark"></i></span>"#
                        };
                        rows_html.push_str(&format!(
                            r#"<td class="pcal-cell has-text-centered{active_class}{sunday_class}">
                                <span class="pcal-check" title="{first_title}">{first_icon} <span>{first_label}</span></span>
                                <span class="pcal-check" title="{second_title}">{second_icon} <span>{second_label}</span></span>
                            </td>"#,
                            active_class = active_class,
                            sunday_class = sunday_class,
                            first_title = if need.nightly { "Soirée" } else { "Matin" },
                            second_title = if need.nightly { "Nuit" } else { "Après-midi" },
                            first_icon = first_icon,
                            second_icon = second_icon,
                            first_label = first_label,
                            second_label = second_label,
                        ));
                    }
                } else {
                    // No need for this atelier on this day
                    let sunday_class = if day.weekday() == chrono::Weekday::Sun {
                        " pcal-sunday"
                    } else {
                        ""
                    };
                    rows_html.push_str(&format!(
                        r#"<td class="pcal-cell has-text-centered has-text-grey-lighter{sunday_class}">—</td>"#,
                        sunday_class = sunday_class,
                    ));
                }
            }
            rows_html.push_str("</tr>");
        }

        let calendar_title = if is_self {
            "Mon calendrier".to_string()
        } else {
            format!("Calendrier de {}", staff.first_name)
        };

        format!(
            r#"<div class="box">
                <h2 class="title is-4">
                    <span class="icon"><i class="fa-solid fa-calendar-days"></i></span>
                    {calendar_title}
                </h2>
                <div class="pcal-scroll">
                    <table class="pcal-table table is-bordered is-narrow is-hoverable">
                        <thead><tr>{header_html}</tr></thead>
                        <tbody>{rows_html}</tbody>
                    </table>
                </div>
            </div>"#,
            header_html = header_html,
            rows_html = rows_html,
        )
    };

    let content = format!(
        r##"    <div id="notification-container"></div>

    <section class="section">
        <div class="container is-fluid">
            <nav class="breadcrumb" aria-label="breadcrumbs">
                <ul>
                    <li><a href="{p}/">Accueil</a></li>
                    <li><a href="{p}/staff">Staff</a></li>
                    <li class="is-active"><a href="#" aria-current="page">{first_name} {last_name}</a></li>
                </ul>
            </nav>

            {todo_html}

            <div class="columns">
                <div class="column is-one-third">
                    <div class="box">
                        <h2 class="title is-4">
                            <span class="icon"><i class="fa-solid fa-user"></i></span>
                            Informations
                        </h2>

                        <div class="content">
                            <p>
                                <strong>Nom complet:</strong><br>
                                <span class="is-size-5">{first_name} {last_name}</span>
                            </p>
                            {contact_html}
                            {comment_html}
                        </div>
                    </div>

                    {admin_box_html}
                </div>

                <div class="column">
                    <div class="box">
                        <h2 class="title is-4">
                            <span class="icon"><i class="fa-solid fa-screwdriver-wrench"></i></span>
                            Ateliers (Saison {current_season})
                        </h2>

                        <div class="notification is-info is-light mb-4">
                            <span class="icon"><i class="fa-solid fa-circle-info"></i></span>
                            {info_text}
                        </div>

                        <div class="ateliers-list">
                            {ateliers_html}
                        </div>
                    </div>

                    {plannings_box}

                    {my_calendar_html}
                </div>
            </div>

            {payment_history_html}
        </div>
    </section>"##,
        p = prefix,
        first_name = staff.first_name,
        last_name = staff.last_name,
        contact_html = contact_html,
        comment_html = comment_html,
        admin_box_html = admin_box_html,
        current_season = current_season,
        info_text = info_text,
        ateliers_html = ateliers_html,
        plannings_box = plannings_box,
        my_calendar_html = my_calendar_html,
        todo_html = todo_html,
        payment_history_html = payment_history_html,
    );

    // Build admin-only scripts conditionally
    let admin_scripts = if is_admin {
        r#"
        // Handle validated checkbox changes
        document.querySelectorAll('.role-validated-checkbox').forEach(checkbox => {
            checkbox.addEventListener('change', async function() {
                const atelierId = this.dataset.atelierId;
                const checked = this.checked;

                try {
                    const response = await fetch(`${prefix}/api/person/${staffId}/role`, {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json'
                        },
                        body: JSON.stringify({
                            atelier_id: atelierId,
                            validated: checked
                        })
                    });

                    if (!response.ok) {
                        const error = await response.text();
                        throw new Error(error);
                    }

                    showNotification(
                        checked ? 'Rôle validé' : 'Validation retirée',
                        'success'
                    );
                } catch (error) {
                    console.error('Error:', error);
                    showNotification('Erreur: ' + error.message, 'danger');
                    this.checked = !checked;
                }
            });
        });

        // Handle chief checkbox changes
        document.querySelectorAll('.role-chief-checkbox').forEach(checkbox => {
            checkbox.addEventListener('change', async function() {
                const atelierId = this.dataset.atelierId;
                const checked = this.checked;

                // Find the corresponding validated checkbox
                const validatedCheckbox = document.querySelector(`.role-validated-checkbox[data-atelier-id="${atelierId}"]`);

                try {
                    const response = await fetch(`${prefix}/api/person/${staffId}/role`, {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json'
                        },
                        body: JSON.stringify({
                            atelier_id: atelierId,
                            chief: checked
                        })
                    });

                    if (!response.ok) {
                        const error = await response.text();
                        throw new Error(error);
                    }

                    // Update validated checkbox state based on chief
                    if (validatedCheckbox) {
                        if (checked) {
                            validatedCheckbox.checked = true;
                            validatedCheckbox.disabled = true;
                        } else {
                            validatedCheckbox.disabled = false;
                        }
                    }

                    showNotification(
                        checked ? 'Défini comme chef' : 'Chef retiré',
                        'success'
                    );
                } catch (error) {
                    console.error('Error:', error);
                    showNotification('Erreur: ' + error.message, 'danger');
                    this.checked = !checked;
                }
            });
        });

        // Handle admin/god checkboxes
        document.getElementById('admin-cb').addEventListener('change', async function() {
            const adminCb = document.getElementById('admin-cb');
            const godCb = document.getElementById('god-cb');
            if (!this.checked) {
                godCb.checked = false;
            }
            try {
                const response = await fetch(`${prefix}/api/admin/flags`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ staff_id: staffId, is_admin: adminCb.checked, is_god: godCb.checked })
                });
                const data = await response.json();
                if (data.success) {
                    adminCb.checked = data.is_admin;
                    godCb.checked = data.is_god;
                    showNotification('Droits mis à jour', 'success');
                } else {
                    showNotification('Erreur: ' + (data.error || 'Inconnue'), 'danger');
                    location.reload();
                }
            } catch (error) {
                showNotification('Erreur réseau: ' + error.message, 'danger');
                location.reload();
            }
        });

        document.getElementById('god-cb').addEventListener('change', async function() {
            const adminCb = document.getElementById('admin-cb');
            const godCb = document.getElementById('god-cb');
            if (this.checked) {
                adminCb.checked = true;
            }
            try {
                const response = await fetch(`${prefix}/api/admin/flags`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ staff_id: staffId, is_admin: adminCb.checked, is_god: godCb.checked })
                });
                const data = await response.json();
                if (data.success) {
                    adminCb.checked = data.is_admin;
                    godCb.checked = data.is_god;
                    showNotification('Droits mis à jour', 'success');
                } else {
                    showNotification('Erreur: ' + (data.error || 'Inconnue'), 'danger');
                    location.reload();
                }
            } catch (error) {
                showNotification('Erreur réseau: ' + error.message, 'danger');
                location.reload();
            }
        });

        // Handle comment save
        document.getElementById('save-comment-btn').addEventListener('click', async function() {
            const comment = document.getElementById('comment-input').value;
            const btn = this;
            btn.classList.add('is-loading');

            try {
                const response = await fetch(`${prefix}/api/person/${staffId}/comment`, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify({ comment: comment })
                });

                if (!response.ok) {
                    const error = await response.text();
                    throw new Error(error);
                }

                showNotification('Commentaire enregistré', 'success');
            } catch (error) {
                console.error('Error:', error);
                showNotification('Erreur: ' + error.message, 'danger');
            } finally {
                btn.classList.remove('is-loading');
            }
        });
"#
    } else {
        ""
    };

    let contact_scripts = if can_edit_contact && show_contact {
        r"
        // Handle contact save
        document.getElementById('save-contact-btn').addEventListener('click', async function() {
            if (!confirm('Attention à bien vérifier avant de confirmer !')) return;
            const email = document.getElementById('edit-email').value;
            const phone = document.getElementById('edit-phone').value;
            const btn = this;
            btn.classList.add('is-loading');

            try {
                const response = await fetch(`${prefix}/api/person/${staffId}/contact`, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify({ email: email, phone: phone || null })
                });

                if (!response.ok) {
                    const error = await response.text();
                    throw new Error(error);
                }

                location.reload();
            } catch (error) {
                console.error('Error:', error);
                showNotification('Erreur: ' + error.message, 'danger');
            } finally {
                btn.classList.remove('is-loading');
            }
        });
"
    } else {
        ""
    };

    let calendar_scripts = if is_self && !person_calendar.is_empty() {
        r#"
        // Handle "Mon calendrier" presence toggles
        document.querySelectorAll('.pcal-presence-cb').forEach(cb => {
            cb.addEventListener('change', async function() {
                const needId = this.dataset.need;
                const staffIdVal = this.dataset.staff;
                const half = this.dataset.half;
                const value = this.checked;

                try {
                    const response = await fetch(`${prefix}/api/calendar/toggle`, {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ needs_id: needId, staff_id: staffIdVal, half: half, value: value })
                    });

                    if (!response.ok) {
                        const body = await response.json().catch(() => ({}));
                        throw new Error(body.error || 'Erreur serveur');
                    }

                    // Update cell highlight
                    const cell = this.closest('td');
                    const anyChecked = Array.from(cell.querySelectorAll('.pcal-presence-cb')).some(c => c.checked);
                    cell.classList.toggle('pcal-active', anyChecked);
                } catch (error) {
                    console.error('Error:', error);
                    showNotification('Erreur: ' + error.message, 'danger');
                    this.checked = !value;
                }
            });
        });
"#
    } else {
        ""
    };

    let scripts = format!(
        r#"    <script>
        const staffId = "{staff_id}";
        const prefix = "{p}";

        function showNotification(message, type) {{
            const container = document.getElementById('notification-container');
            const notification = document.createElement('div');
            notification.className = `notification is-${{type}} is-loading`;
            notification.innerHTML = `
                <button class="delete"></button>
                ${{message}}
            `;
            container.appendChild(notification);

            notification.querySelector('.delete').addEventListener('click', () => {{
                notification.remove();
            }});

            setTimeout(() => notification.remove(), 3000);
        }}

        document.querySelectorAll('.atelier-checkbox').forEach(checkbox => {{
            checkbox.addEventListener('change', async function() {{
                const atelierId = this.dataset.atelierId;
                const checked = this.checked;

                try {{
                    const response = await fetch(`${{prefix}}/api/person/${{staffId}}/role`, {{
                        method: 'POST',
                        headers: {{
                            'Content-Type': 'application/json'
                        }},
                        body: JSON.stringify({{
                            atelier_id: atelierId,
                            add: checked
                        }})
                    }});

                    if (!response.ok) {{
                        const error = await response.text();
                        throw new Error(error);
                    }}

                    showNotification(
                        checked ? 'Atelier ajouté' : 'Atelier retiré',
                        'success'
                    );
                    // Reload page to update role options
                    setTimeout(() => location.reload(), 500);
                }} catch (error) {{
                    console.error('Error:', error);
                    showNotification('Erreur: ' + error.message, 'danger');
                    this.checked = !checked; // Revert checkbox state
                }}
            }});
        }});

        {admin_scripts}

        {contact_scripts}

        {calendar_scripts}
    </script>"#,
        p = prefix,
        staff_id = staff.id,
        admin_scripts = admin_scripts,
        contact_scripts = contact_scripts,
        calendar_scripts = calendar_scripts,
    );

    let title = format!("{} {} - AGHIL", staff.first_name, staff.last_name);
    page(
        &title,
        prefix,
        &NavKind::Standard,
        "",
        extra_head,
        &content,
        &scripts,
    )
}

pub fn cash_list(cash_payments: Vec<(Cash, bool)>, current_season: i16, prefix: &str) -> String {
    let mut rows_html = String::new();
    let total_count = cash_payments.len();
    let imported_count = cash_payments
        .iter()
        .filter(|(_, imported)| *imported)
        .count();
    let not_imported_count = total_count - imported_count;

    for (cash, has_staff) in &cash_payments {
        let full_name = format!(
            "{} {}",
            capitalize_words(&cash.first_name),
            capitalize_words(&cash.last_name)
        );
        let email = cash.email.as_deref().unwrap_or("—");
        let phone = cash
            .phone
            .as_deref()
            .map_or_else(|| "—".to_string(), format_phone_international);
        let date = cash.date.format("%d/%m/%Y").to_string();
        let season: i16 = if cash.date.month() >= 6 {
            cash.date.year() as i16 + 1
        } else {
            cash.date.year() as i16
        };
        let amount = format!("{}€", cash.amount);
        let type_label = if cash.is_membership {
            "Adhésion"
        } else {
            "Autre"
        };
        let type_class = if cash.is_membership {
            "is-primary"
        } else {
            "is-info"
        };
        let method_label = if cash.payment_method == "check" {
            "Chèque"
        } else {
            "Espèces"
        };
        let method_icon = if cash.payment_method == "check" {
            "fa-money-check"
        } else {
            "fa-coins"
        };

        let status_html = if *has_staff {
            r#"<span class="tag is-success">Importé</span>"#.to_string()
        } else {
            format!(
                r#"<a href="{}/cash-import/{}" class="tag is-warning">À importer</a>"#,
                prefix, cash.id
            )
        };

        rows_html.push_str(&format!(
            r#"
                <tr>
                    <td><strong>{full_name}</strong></td>
                    <td>{email}</td>
                    <td>{phone}</td>
                    <td><span class="icon-text"><span class="icon"><i class="fa-solid {method_icon}"></i></span><span>{method_label}</span></span></td>
                    <td><span class="tag {type_class}">{type_label}</span></td>
                    <td class="has-text-right"><strong class="has-text-success">{amount}</strong></td>
                    <td>{date}</td>
                    <td><span class="tag {season_tag_class}">{season}</span></td>
                    <td>{status_html}</td>
                </tr>"#,
            full_name = full_name,
            email = email,
            phone = phone,
            method_icon = method_icon,
            method_label = method_label,
            type_class = type_class,
            type_label = type_label,
            amount = amount,
            date = date,
            season_tag_class = if season == current_season { "is-primary" } else { "is-light" },
            season = season,
            status_html = status_html,
        ));
    }

    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="level mb-4">
                <div class="level-left">
                    <h1 class="title is-3">
                        <span class="icon"><i class="fa-solid fa-money-bill-wave"></i></span>
                        Paiements espèces / chèques
                    </h1>
                </div>
                <div class="level-right">
                    <a href="{p}/cash?form=1" class="button is-primary">
                        <span class="icon"><i class="fa-solid fa-plus"></i></span>
                        <span>Nouveau paiement</span>
                    </a>
                </div>
            </div>

            <div class="tags mb-4">
                <span class="tag is-medium">Total: {total_count}</span>
                <span class="tag is-success is-medium">Importés: {imported_count}</span>
                <span class="tag is-warning is-medium">À importer: {not_imported_count}</span>
                <span class="tag is-info is-medium">Saison: {current_season}</span>
            </div>

            <div class="box">
                <table class="table is-fullwidth is-striped is-hoverable">
                    <thead>
                        <tr>
                            <th>Nom</th>
                            <th>Email</th>
                            <th>Téléphone</th>
                            <th>Moyen</th>
                            <th>Type</th>
                            <th class="has-text-right">Montant</th>
                            <th>Date</th>
                            <th>Saison</th>
                            <th>Statut</th>
                        </tr>
                    </thead>
                    <tbody>
                        {rows_html}
                    </tbody>
                </table>
            </div>
        </div>
    </section>"#,
        p = prefix,
        total_count = total_count,
        imported_count = imported_count,
        not_imported_count = not_imported_count,
        current_season = current_season,
        rows_html = rows_html,
    );

    page(
        "Paiements espèces / chèques - AGHIL",
        prefix,
        &NavKind::Standard,
        "cash",
        "",
        &content,
        "",
    )
}

pub fn cash_form(prefix: &str) -> String {
    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="columns is-centered">
                <div class="column is-6">
                    <div class="level mb-5">
                        <div class="level-left">
                            <h1 class="title is-3">Nouveau paiement</h1>
                        </div>
                        <div class="level-right">
                            <a href="{p}/cash" class="button is-light">
                                <span class="icon"><i class="fa-solid fa-arrow-left"></i></span>
                                <span>Retour</span>
                            </a>
                        </div>
                    </div>

                    <div class="box">
                        <form method="POST" action="{p}/cash">
                            <div class="columns">
                                <div class="column">
                                    <div class="field">
                                        <label class="label">Prénom *</label>
                                        <div class="control">
                                            <input class="input" type="text" name="first_name" required>
                                        </div>
                                    </div>
                                </div>
                                <div class="column">
                                    <div class="field">
                                        <label class="label">Nom *</label>
                                        <div class="control">
                                            <input class="input" type="text" name="last_name" required>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <div class="field">
                                <label class="label">Email</label>
                                <div class="control">
                                    <input class="input" type="email" name="email">
                                </div>
                            </div>

                            <div class="field">
                                <label class="label">Téléphone</label>
                                <div class="control">
                                    <input class="input" type="tel" name="phone">
                                </div>
                            </div>

                            <div class="columns">
                                <div class="column">
                                    <div class="field">
                                        <label class="label">Date *</label>
                                        <div class="control">
                                            <input class="input" type="date" name="date" required>
                                        </div>
                                    </div>
                                </div>
                                <div class="column">
                                    <div class="field">
                                        <label class="label">Montant (euros) *</label>
                                        <div class="control">
                                            <input class="input" type="number" name="amount" min="1" required>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <div class="field">
                                <label class="label">Moyen de paiement *</label>
                                <div class="control">
                                    <div class="select is-fullwidth">
                                        <select name="payment_method" required>
                                            <option value="cash">Espèces</option>
                                            <option value="check">Chèque</option>
                                        </select>
                                    </div>
                                </div>
                            </div>

                            <div class="field">
                                <label class="checkbox">
                                    <input type="checkbox" name="is_membership" value="true" checked>
                                    Adhésion (cotisation)
                                </label>
                                <p class="help">Décochez si ce n'est pas une cotisation (ex: don, participation aux frais...)</p>
                            </div>

                            <div class="field mt-5">
                                <div class="control">
                                    <button type="submit" class="button is-primary is-fullwidth">
                                        <span class="icon"><i class="fa-solid fa-floppy-disk"></i></span>
                                        <span>Enregistrer le paiement</span>
                                    </button>
                                </div>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    </section>"#,
        p = prefix,
    );

    page(
        "Nouveau paiement - AGHIL",
        prefix,
        &NavKind::Standard,
        "cash",
        "",
        &content,
        "",
    )
}

pub fn cash_import_form(
    cash: &Cash,
    season: i16,
    candidates: Vec<StaffWithSeason>,
    prefix: &str,
) -> String {
    let beneficiary_first = capitalize_words(&cash.first_name);
    let beneficiary_last = capitalize_words(&cash.last_name);
    let cash_email = cash.email.as_deref().unwrap_or("").to_lowercase();
    let default_email = &cash_email;
    let phone = cash
        .phone
        .as_deref()
        .map(format_phone_international)
        .unwrap_or_default();
    let amount = format!("{}€", cash.amount);
    let date = cash.date.format("%d/%m/%Y").to_string();
    let type_label = if cash.is_membership {
        "Adhésion"
    } else {
        "Autre"
    };
    let method_label = if cash.payment_method == "check" {
        "Chèque"
    } else {
        "Espèces"
    };

    let has_exact_match = candidates.iter().any(|c| {
        matches!(
            c.match_type,
            StaffMatchType::ExactBoth | StaffMatchType::ExactEmail | StaffMatchType::ExactName
        )
    });
    let recommend_create = !has_exact_match;

    // Build candidates HTML
    let mut candidates_html = String::new();
    let mut is_first = !recommend_create;
    let mut option_index = 0usize;

    for candidate in &candidates {
        let staff = &candidate.staff;
        let match_label = match candidate.match_type {
            StaffMatchType::ExactBoth => "Email et nom identiques",
            StaffMatchType::ExactName => "Nom identique",
            StaffMatchType::ExactEmail => "Email identique",
            StaffMatchType::PayerEmailMatch => "Email payeur identique",
            StaffMatchType::SimilarEmail => "Email similaire",
            StaffMatchType::SimilarName => "Nom similaire",
            StaffMatchType::DoubleSubscription => "Double adhésion probable",
        };

        let season_info = candidate.latest_season.map_or_else(
            || "Aucune saison".to_string(),
            |s| format!("Dernière saison: {}", s),
        );

        let is_exact_match = matches!(
            candidate.match_type,
            StaffMatchType::ExactBoth | StaffMatchType::ExactEmail | StaffMatchType::ExactName
        );

        let (highlight, recommended, border_color) =
            if candidate.match_type == StaffMatchType::DoubleSubscription {
                (
                    "is-danger",
                    r#"<span class="tag is-danger ml-2">Double adhésion</span>"#,
                    "var(--bulma-danger)",
                )
            } else if is_first && is_exact_match {
                (
                    "is-primary",
                    r#"<span class="tag is-success ml-2">Probable meilleure option</span>"#,
                    "var(--bulma-primary)",
                )
            } else if is_exact_match {
                (
                    "is-info",
                    r#"<span class="tag is-warning ml-2">Option envisageable</span>"#,
                    "var(--bulma-info)",
                )
            } else {
                ("is-light", "", "var(--bulma-border)")
            };

        let names_match = beneficiary_first.to_lowercase() == staff.first_name.to_lowercase()
            && beneficiary_last.to_lowercase() == staff.last_name.to_lowercase();

        let name_choice_html = if names_match {
            format!(
                r#"<input type="hidden" name="first_name" value="{}">
                   <input type="hidden" name="last_name" value="{}">"#,
                beneficiary_first, beneficiary_last
            )
        } else {
            format!(
                r#"<div class="field">
                    <label class="label">Garder le prénom et nom</label>
                    <div class="control">
                        <label class="radio">
                            <input type="radio" name="name_choice" value="cash" checked onchange="updateNameFields(this.form, '{}', '{}')">
                            Du paiement: <strong>{} {}</strong>
                        </label>
                        <br>
                        <label class="radio">
                            <input type="radio" name="name_choice" value="staff" onchange="updateNameFields(this.form, '{}', '{}')">
                            Du staff: <strong>{} {}</strong>
                        </label>
                    </div>
                </div>
                <input type="hidden" name="first_name" value="{}">
                <input type="hidden" name="last_name" value="{}">"#,
                beneficiary_first,
                beneficiary_last,
                beneficiary_first,
                beneficiary_last,
                staff.first_name,
                staff.last_name,
                staff.first_name,
                staff.last_name,
                beneficiary_first,
                beneficiary_last
            )
        };

        let staff_email_lower = staff.email.to_lowercase();
        let email_choice_html = if cash_email.is_empty() || cash_email == staff_email_lower {
            let email_value = if cash_email.is_empty() {
                &staff.email
            } else {
                &cash_email
            };
            format!(
                r#"<input type="hidden" name="email" value="{}">"#,
                email_value
            )
        } else {
            format!(
                r#"<div class="field">
                    <label class="label">Garder l'email</label>
                    <div class="control">
                        <label class="radio">
                            <input type="radio" name="email_choice" value="cash" checked onchange="updateEmailField(this.form, '{}')">
                            Du paiement: <strong>{}</strong>
                        </label>
                        <br>
                        <label class="radio">
                            <input type="radio" name="email_choice" value="staff" onchange="updateEmailField(this.form, '{}')">
                            Du staff: <strong>{}</strong>
                        </label>
                    </div>
                </div>
                <input type="hidden" name="email" value="{}">"#,
                cash_email, cash_email, staff.email, staff.email, cash_email
            )
        };

        let bg_color = if option_index.is_multiple_of(2) {
            "var(--bulma-scheme-main)"
        } else {
            "var(--bulma-scheme-main-bis)"
        };
        option_index += 1;

        candidates_html.push_str(&format!(
            r#"
            <div class="box mb-4" style="border: 2px solid {}; background-color: {};">
                <form method="POST">
                    <input type="hidden" name="action" value="update">
                    <input type="hidden" name="staff_id" value="{}">

                    <div class="level mb-3">
                        <div class="level-left">
                            <span class="tag {}">{}</span>
                            {}
                        </div>
                        <div class="level-right">
                            <span class="tag is-info is-light">{}</span>
                        </div>
                    </div>

                    <p class="mb-3"><strong>Staff existant:</strong> {} {} &lt;{}&gt;</p>

                    {}

                    {}

                    <input type="hidden" name="phone" value="{}">

                    <div class="field">
                        <label class="label">Commentaire</label>
                        <div class="control">
                            <textarea class="textarea" name="comment" rows="2"></textarea>
                        </div>
                    </div>

                    <div class="field">
                        <div class="control">
                            <button type="submit" class="button {} is-fullwidth">
                                <span class="icon"><i class="fa-solid fa-arrows-rotate"></i></span>
                                <span>Mettre à jour ce staff</span>
                            </button>
                        </div>
                    </div>
                </form>
            </div>
            "#,
            border_color,
            bg_color,
            staff.id,
            highlight,
            match_label,
            recommended,
            season_info,
            staff.first_name,
            staff.last_name,
            staff.email,
            name_choice_html,
            email_choice_html,
            phone,
            highlight,
        ));

        is_first = false;
    }

    // Create new staff option
    let create_highlight = if recommend_create {
        "is-primary"
    } else {
        "is-light"
    };
    let create_recommended = if recommend_create {
        r#"<span class="tag is-success ml-2">Probable meilleure option</span>"#
    } else {
        ""
    };
    let create_border = if recommend_create {
        "var(--bulma-primary)"
    } else {
        "var(--bulma-border)"
    };
    let create_bg_color = if option_index.is_multiple_of(2) {
        "var(--bulma-scheme-main)"
    } else {
        "var(--bulma-scheme-main-bis)"
    };

    let create_email_html = format!(
        r#"
            <div class="field">
                <label class="label">Email</label>
                <div class="control">
                    <input class="input" type="email" name="email" value="{}">
                </div>
            </div>
        "#,
        default_email
    );

    let create_html = format!(
        r#"
        <div class="box mb-4" style="border: 2px solid {}; background-color: {};">
            <form method="POST">
                <input type="hidden" name="action" value="create">

                <div class="level mb-3">
                    <div class="level-left">
                        <span class="tag {}">Nouveau staff</span>
                        {}
                    </div>
                </div>

                <div class="columns">
                    <div class="column">
                        <div class="field">
                            <label class="label">Prénom</label>
                            <div class="control">
                                <input class="input" type="text" name="first_name" value="{}">
                            </div>
                        </div>
                    </div>
                    <div class="column">
                        <div class="field">
                            <label class="label">Nom</label>
                            <div class="control">
                                <input class="input" type="text" name="last_name" value="{}">
                            </div>
                        </div>
                    </div>
                </div>

                {}

                <div class="field">
                    <label class="label">Téléphone</label>
                    <div class="control">
                        <input class="input" type="tel" name="phone" value="{}">
                    </div>
                </div>

                <div class="field">
                    <label class="label">Commentaire</label>
                    <div class="control">
                        <textarea class="textarea" name="comment" rows="2"></textarea>
                    </div>
                </div>

                <div class="field">
                    <div class="control">
                        <button type="submit" class="button {} is-fullwidth">
                            <span class="icon"><i class="fa-solid fa-plus"></i></span>
                            <span>Créer un nouveau staff</span>
                        </button>
                    </div>
                </div>
            </form>
        </div>
        "#,
        create_border,
        create_bg_color,
        create_highlight,
        create_recommended,
        beneficiary_first,
        beneficiary_last,
        create_email_html,
        phone,
        create_highlight,
    );

    let options_html = if recommend_create {
        format!("{}{}", create_html, candidates_html)
    } else {
        format!("{}{}", candidates_html, create_html)
    };

    let total_options = candidates.len() + 1;
    let multiple_options_warning = if total_options > 1 {
        r#"<div class="notification is-danger mb-4">
            <span class="icon"><i class="fa-solid fa-triangle-exclamation"></i></span>
            <strong>Attention</strong>, il y a plusieurs possibilités, examinez-les bien avant de choisir la bonne.
        </div>"#
    } else {
        ""
    };

    let extra_head = r#"    <script>
        function updateNameFields(form, firstName, lastName) {
            form.querySelector('input[name="first_name"]').value = firstName;
            form.querySelector('input[name="last_name"]').value = lastName;
        }
        function updateEmailField(form, email) {
            form.querySelector('input[name="email"]').value = email;
        }
    </script>"#;

    let email_display = if cash_email.is_empty() {
        "N/A"
    } else {
        &cash_email
    };

    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <div class="level mb-5">
                <div class="level-left">
                    <h1 class="title is-3">Importer un paiement</h1>
                </div>
                <div class="level-right">
                    <a href="{p}/cash" class="button is-light">
                        <span class="icon"><i class="fa-solid fa-arrow-left"></i></span>
                        <span>Retour</span>
                    </a>
                </div>
            </div>

            <div class="columns">
                <div class="column is-5">
                    <div class="box">
                        <h2 class="title is-4 mb-4">Détails du paiement</h2>
                        <div class="content">
                            <table class="table is-fullwidth">
                                <tbody>
                                    <tr>
                                        <th>Nom</th>
                                        <td><strong>{first} {last}</strong></td>
                                    </tr>
                                    <tr>
                                        <th>Email</th>
                                        <td>{email_display}</td>
                                    </tr>
                                    <tr>
                                        <th>Téléphone</th>
                                        <td>{phone}</td>
                                    </tr>
                                    <tr>
                                        <th>Moyen</th>
                                        <td>{method_label}</td>
                                    </tr>
                                    <tr>
                                        <th>Type</th>
                                        <td>{type_label}</td>
                                    </tr>
                                    <tr>
                                        <th>Montant</th>
                                        <td>{amount}</td>
                                    </tr>
                                    <tr>
                                        <th>Date</th>
                                        <td>{date}</td>
                                    </tr>
                                    <tr>
                                        <th>Saison</th>
                                        <td><span class="tag is-info is-medium">{season}</span></td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>

                <div class="column is-7">
                    <h2 class="title is-4 mb-4">Options d'import</h2>
                    {multiple_options_warning}
                    {options_html}
                </div>
            </div>
        </div>
    </section>"#,
        p = prefix,
        first = beneficiary_first,
        last = beneficiary_last,
        email_display = email_display,
        phone = phone,
        method_label = method_label,
        type_label = type_label,
        amount = amount,
        date = date,
        season = season,
        multiple_options_warning = multiple_options_warning,
        options_html = options_html,
    );

    page(
        "Importer paiement - AGHIL",
        prefix,
        &NavKind::Standard,
        "cash",
        extra_head,
        &content,
        "",
    )
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
) -> String {
    // Build nav links for ateliers
    let mut atelier_nav = String::new();
    for a in all_ateliers {
        let active = if a.id == atelier.id { " is-active" } else { "" };
        atelier_nav.push_str(&format!(
            r#"<a class="navbar-item{active}" href="{p}/calendar/{slug}">
            <span class="icon"><i class="fa-solid fa-{icon}"></i></span>&nbsp;
            {name}</a>"#,
            active = active,
            p = prefix,
            slug = a.slug,
            icon = a.icon,
            name = a.name,
        ));
    }

    // Build column headers (days)
    let mut header_html = String::from(r#"<th class="cal-name-col">Nom</th>"#);
    for need in needs {
        // Format day as "sam.\n29/11"
        let day_abbrev = need.day.format("%a").to_string();
        let day_name = match day_abbrev.as_str() {
            "Mon" => "lun.",
            "Tue" => "mar.",
            "Wed" => "mer.",
            "Thu" => "jeu.",
            "Fri" => "ven.",
            "Sat" => "sam.",
            "Sun" => "dim.",
            _ => &day_abbrev,
        };
        let day_date = need.day.format("%d/%m").to_string();
        let is_sunday = need.day.weekday() == chrono::Weekday::Sun;

        // Count filled per half-day
        let filled_first: i16 = staff_list
            .iter()
            .filter(|s| presence.get(&(need.id, s.id)).is_some_and(|(f, _)| *f))
            .count() as i16;
        let filled_second: i16 = staff_list
            .iter()
            .filter(|s| presence.get(&(need.id, s.id)).is_some_and(|(_, s)| *s))
            .count() as i16;
        let both_complete = filled_first >= need.quantity && filled_second >= need.quantity;
        let first_class = if filled_first >= need.quantity {
            "has-text-success"
        } else {
            "has-text-danger"
        };
        let second_class = if filled_second >= need.quantity {
            "has-text-success"
        } else {
            "has-text-danger"
        };

        let sunday_class = if is_sunday { " cal-sunday" } else { "" };
        let complete_class = if both_complete {
            " cal-complete"
        } else {
            " cal-danger"
        };

        let (first_label_h, second_label_h) = if need.nightly {
            ("soir", "nuit")
        } else {
            ("matin", "après-midi")
        };
        header_html.push_str(&format!(
            r#"<th class="cal-day-col has-text-centered{sunday_class}{complete_class}"><div class="cal-day-name">{day_name}</div><div class="cal-day-date">{day_date}</div><div class="cal-day-count"><span class="{first_class}">{first_label_h} {filled_first}/{qty}</span> <span class="{second_class}">{second_label_h} {filled_second}/{qty}</span></div></th>"#,
            sunday_class = sunday_class,
            complete_class = complete_class,
            day_name = day_name,
            day_date = day_date,
            first_class = first_class,
            second_class = second_class,
            first_label_h = first_label_h,
            second_label_h = second_label_h,
            filled_first = filled_first,
            filled_second = filled_second,
            qty = need.quantity,
        ));
    }

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

    // Build rows (staff)
    let mut rows_html = String::new();
    for staff in staff_list {
        let can_toggle = viewer_id.is_some_and(|vid| staff.id == vid);
        let disabled_attr = if can_toggle { "" } else { "disabled" };
        let me_class = if can_toggle { " cal-me" } else { "" };
        let name = format!(
            "{} {}",
            capitalize_words(&staff.first_name),
            capitalize_words(&staff.last_name)
        );
        rows_html.push_str(&format!(
            r#"<tr class="{me_class}"><td class="cal-name-col"><a href="{p}/person/{id}">{name}</a></td>"#,
            me_class = me_class,
            p = prefix,
            id = staff.id,
            name = name,
        ));

        for need in needs {
            let (first_half, second_half) = presence
                .get(&(need.id, staff.id))
                .copied()
                .unwrap_or((false, false));

            let first_checked = if first_half { "checked" } else { "" };
            let second_checked = if second_half { "checked" } else { "" };

            let (first_label, second_label) = if need.nightly {
                ("soir", "nuit")
            } else {
                ("matin", "après-midi")
            };

            let active_class = if first_half || second_half {
                " cal-active"
            } else {
                ""
            };
            let sunday_class = if need.day.weekday() == chrono::Weekday::Sun {
                " cal-sunday"
            } else {
                ""
            };
            let complete_class = if complete_needs.contains(&need.id) {
                " cal-complete"
            } else {
                " cal-danger"
            };

            rows_html.push_str(&format!(
                r#"<td class="cal-cell has-text-centered{active_class}{sunday_class}{complete_class}">
                    <label class="cal-check" title="{first_title}">
                        <input type="checkbox" class="presence-cb" data-need="{need_id}" data-staff="{staff_id}" data-half="first" {first_checked} {disabled}>
                        <span>{first_label}</span>
                    </label>
                    <label class="cal-check" title="{second_title}">
                        <input type="checkbox" class="presence-cb" data-need="{need_id}" data-staff="{staff_id}" data-half="second" {second_checked} {disabled}>
                        <span>{second_label}</span>
                    </label>
                </td>"#,
                active_class = active_class,
                sunday_class = sunday_class,
                complete_class = complete_class,
                first_title = if need.nightly { "Soirée" } else { "Matin" },
                second_title = if need.nightly { "Nuit" } else { "Après-midi" },
                need_id = need.id,
                staff_id = staff.id,
                first_checked = first_checked,
                second_checked = second_checked,
                first_label = first_label,
                second_label = second_label,
                disabled = disabled_attr,
            ));
        }

        rows_html.push_str("</tr>");
    }

    let extra_head = r"    <style>
        .cal-scroll { overflow-x: auto; max-height: 85vh; }
        .cal-table { border-collapse: collapse; white-space: nowrap; }
        .cal-table th, .cal-table td { border: 1px solid var(--bulma-border); padding: 0.3rem 0.4rem; vertical-align: middle; }
        .cal-table thead { position: sticky; top: 0; z-index: 4; }
        .cal-table thead th { background: var(--bulma-scheme-main-bis) !important; }
        .cal-name-col { position: sticky; left: 0; z-index: 2; min-width: 150px; }
        .cal-table thead th.cal-name-col { z-index: 5; background: var(--bulma-scheme-main-bis) !important; }
        .cal-table tbody tr:nth-child(odd) td { background: var(--bulma-scheme-main); }
        .cal-table tbody tr:nth-child(even) td { background: var(--bulma-background); }
        .cal-table tbody tr:nth-child(odd) td.cal-name-col { background: var(--bulma-scheme-main); }
        .cal-table tbody tr:nth-child(even) td.cal-name-col { background: var(--bulma-background); }
        .cal-sunday { background: var(--bulma-link-light) !important; }
        .cal-complete { background: var(--bulma-success-light) !important; }
        .cal-danger { background: var(--bulma-danger-light) !important; }
        .cal-cell.cal-active { background: var(--bulma-success) !important; color: var(--bulma-success-invert); }
        .cal-day-col { min-width: 70px; }
        .cal-day-name { font-size: 0.75rem; }
        .cal-day-date { font-size: 0.85rem; font-weight: 600; }
        .cal-day-count { font-size: 0.7rem; font-weight: bold; }
        .cal-check { display: inline-flex; align-items: center; gap: 1px; margin: 0 2px; cursor: pointer; font-size: 0.7rem; }
        .cal-check input { width: 1rem; height: 1rem; margin: 0; }
        .cal-cell { white-space: nowrap; }
        .cal-me td.cal-name-col { background: var(--bulma-warning) !important; font-weight: 600; }
        .notification.is-loading {
            position: fixed;
            top: 20px;
            left: 50%;
            transform: translateX(-50%);
            z-index: 100;
            min-width: 300px;
        }
        .atelier-nav { display: flex; flex-wrap: wrap; gap: 0.25rem; margin-bottom: 1rem; }
        .atelier-nav a { padding: 0.4rem 0.75rem; border-radius: 4px; background: var(--bulma-background); color: var(--bulma-text); text-decoration: none; font-size: 0.9rem; }
        .atelier-nav a.is-active { background: var(--bulma-link); color: var(--bulma-link-invert); font-weight: 600; }
        .atelier-nav a:hover:not(.is-active) { background: var(--bulma-scheme-main-ter); }
    </style>";

    let empty_message = if needs.is_empty() {
        r#"<div class="notification is-warning is-light mt-4"><span class="icon"><i class="fa-solid fa-triangle-exclamation"></i></span> Aucun besoin déclaré pour cet atelier.</div>"#
    } else if staff_list.is_empty() {
        r#"<div class="notification is-info is-light mt-4"><span class="icon"><i class="fa-solid fa-circle-info"></i></span> Aucun bénévole assigné à cet atelier.</div>"#
    } else {
        ""
    };

    let content = format!(
        r#"    <div id="notification-container"></div>

    <section class="section pt-4 pb-4">
        <div class="container is-fluid">
            <h1 class="title is-4 mb-3">
                <span class="icon"><i class="fa-solid fa-calendar-days"></i></span>
                Planning — {atelier_name}
            </h1>

            <div class="atelier-nav">
                {atelier_nav}
            </div>

            <div class="cal-scroll">
                <table class="cal-table table is-bordered is-narrow is-hoverable">
                    <thead>
                        <tr>{header_html}</tr>
                    </thead>
                    <tbody>
                        {rows_html}
                    </tbody>
                </table>
            </div>

            {empty_message}
        </div>
    </section>"#,
        atelier_name = atelier.name,
        atelier_nav = atelier_nav,
        header_html = header_html,
        rows_html = rows_html,
        empty_message = empty_message,
    );

    let scripts = format!(
        r#"    <script>
        const prefix = "{p}";

        function showNotification(message, type) {{
            const container = document.getElementById('notification-container');
            const notification = document.createElement('div');
            notification.className = `notification is-${{type}} is-loading`;
            notification.innerHTML = `<button class="delete"></button>${{message}}`;
            container.appendChild(notification);
            notification.querySelector('.delete').addEventListener('click', () => notification.remove());
            setTimeout(() => notification.remove(), 3000);
        }}

        document.querySelectorAll('.presence-cb').forEach(cb => {{
            cb.addEventListener('change', async function() {{
                const needId = this.dataset.need;
                const staffId = this.dataset.staff;
                const half = this.dataset.half;
                const value = this.checked;

                try {{
                    const response = await fetch(`${{prefix}}/api/calendar/toggle`, {{
                        method: 'POST',
                        headers: {{ 'Content-Type': 'application/json' }},
                        body: JSON.stringify({{ needs_id: needId, staff_id: staffId, half: half, value: value }})
                    }});

                    if (!response.ok) {{
                        if (response.status === 403) {{
                            throw new Error('Vous ne pouvez modifier que votre propre disponibilité');
                        }}
                        const body = await response.json().catch(() => ({{}}));
                        throw new Error(body.error || 'Erreur serveur');
                    }}

                    // Update cell highlight
                    const cell = this.closest('td');
                    const anyChecked = Array.from(cell.querySelectorAll('.presence-cb')).some(c => c.checked);
                    cell.classList.toggle('cal-active', anyChecked);

                    // Update the counts in the column header
                    const colIndex = cell.cellIndex;
                    const table = this.closest('table');
                    const th = table.querySelector(`thead tr th:nth-child(${{colIndex + 1}})`);
                    if (th) {{
                        // Recount checked staff per half in this column
                        let filledFirst = 0, filledSecond = 0;
                        table.querySelectorAll(`tbody tr`).forEach(row => {{
                            const c = row.cells[colIndex];
                            if (c) {{
                                const cbs = c.querySelectorAll('.presence-cb');
                                cbs.forEach(cb => {{
                                    if (cb.checked && cb.dataset.half === 'first') filledFirst++;
                                    if (cb.checked && cb.dataset.half === 'second') filledSecond++;
                                }});
                            }}
                        }});
                        const countEl = th.querySelector('.cal-day-count');
                        if (countEl) {{
                            const spans = countEl.querySelectorAll('span');
                            if (spans.length === 2) {{
                                // Extract qty from existing text (e.g. "M 2/3")
                                const qtyMatch = spans[0].textContent.match(/\/(\d+)/);
                                const qty = qtyMatch ? parseInt(qtyMatch[1]) : 0;
                                const firstLabel = spans[0].textContent.replace(/\d+\/\d+/, '').trim();
                                const secondLabel = spans[1].textContent.replace(/\d+\/\d+/, '').trim();
                                spans[0].textContent = `${{firstLabel}} ${{filledFirst}}/${{qty}}`;
                                spans[1].textContent = `${{secondLabel}} ${{filledSecond}}/${{qty}}`;
                                spans[0].className = filledFirst >= qty ? 'has-text-success' : 'has-text-danger';
                                spans[1].className = filledSecond >= qty ? 'has-text-success' : 'has-text-danger';
                                const isComplete = filledFirst >= qty && filledSecond >= qty;
                                th.classList.toggle('cal-complete', isComplete);
                                table.querySelectorAll('tbody tr').forEach(row => {{
                                    const c = row.cells[colIndex];
                                    if (c) c.classList.toggle('cal-complete', isComplete);
                                }});
                            }}
                        }}
                    }}
                }} catch (error) {{
                    console.error('Error:', error);
                    showNotification('Erreur: ' + error.message, 'danger');
                    this.checked = !value;
                }}
            }});
        }});
    </script>"#,
        p = prefix,
    );

    let title = format!("Planning {} - AGHIL", atelier.name);
    page(
        &title,
        prefix,
        &NavKind::Standard,
        "",
        extra_head,
        &content,
        &scripts,
    )
}

/// Render the "Semaine à venir" (upcoming week needs) HTML snippet.
/// Used on both the index page and the calendar editor page.
fn render_upcoming_week(upcoming: &[(chrono::NaiveDate, String, i16, i64)]) -> String {
    let mut week_html = String::new();
    if upcoming.is_empty() {
        week_html.push_str(r#"<p class="has-text-grey-light">Aucun besoin déclaré pour les 7 prochains jours.</p>"#);
    } else {
        let mut current_day: Option<chrono::NaiveDate> = None;
        let mut day_deficits: Vec<(String, i64)> = Vec::new();

        let flush_day = |day: chrono::NaiveDate, deficits: &[(String, i64)], html: &mut String| {
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
                html.push_str(&format!(
                    r#"<div class="week-day week-day-ok"><span class="icon has-text-success"><i class="fa-solid fa-circle-check"></i></span> <strong>{}</strong> — complet</div>"#,
                    date_str
                ));
            } else {
                html.push_str(&format!(
                    r#"<div class="week-day week-day-missing"><span class="icon has-text-danger"><i class="fa-solid fa-circle-exclamation"></i></span> <strong>{}</strong> — il manque {}</div>"#,
                    date_str,
                    missing_parts.join(", "),
                ));
            }
        };

        for (day, atelier_name, quantity, filled) in upcoming {
            let missing = i64::from(*quantity) - filled;
            if current_day != Some(*day) {
                if let Some(prev_day) = current_day {
                    flush_day(prev_day, &day_deficits, &mut week_html);
                }
                current_day = Some(*day);
                day_deficits.clear();
            }
            day_deficits.push((atelier_name.clone(), missing.max(0)));
        }
        if let Some(prev_day) = current_day {
            flush_day(prev_day, &day_deficits, &mut week_html);
        }
    }
    week_html
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
) -> String {
    use std::collections::{BTreeMap, BTreeSet};

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

    // --- Build table HTML ---
    // Header row 1: Atelier + date columns
    let mut header1 = String::from(r#"<th rowspan="2" class="cal-name-col">Atelier</th>"#);
    for d in &days {
        let n = subcols(d);
        let dd = format!("{:02}", d.day());
        let mm = format!("{:02}", d.month());
        header1.push_str(&format!(
            r#"<th class="day-start" colspan="{n}">{dow} {dd}/{mm}</th>"#,
            n = n,
            dow = day_abbrev(*d),
            dd = dd,
            mm = mm,
        ));
    }

    // Header row 2: sub-column labels
    let mut header2 = String::new();
    for d in &days {
        let (has_day, has_night) = day_types.get(d).copied().unwrap_or((false, false));
        if has_day && has_night {
            header2.push_str(
                r#"<th class="day-start">matin</th><th>a-m</th><th>soir</th><th>nuit</th>"#,
            );
        } else if has_night {
            header2.push_str(r#"<th class="day-start">soir</th><th>nuit</th>"#);
        } else {
            header2.push_str(r#"<th class="day-start">matin</th><th>a-m</th>"#);
        }
    }

    // Body rows
    let mut body = String::new();
    for atelier in all_ateliers {
        body.push_str(&format!(
            r#"<tr><td class="cal-name-col">{name}</td>"#,
            name = atelier.name,
        ));

        for d in &days {
            let (has_day, has_night) = day_types.get(d).copied().unwrap_or((false, false));
            let mixed = has_day && has_night;
            let n_subcols = if mixed { 4 } else { 2 };
            let day_str = d.format("%Y-%m-%d").to_string();
            let entry = needs_map.get(&(atelier.id, *d));
            let mut first = true; // track first cell of this day group

            // Helper: class string for a cell, adding day-start on the first one
            let mut cell_class = |extra: &str| -> String {
                let ds = if first {
                    first = false;
                    " day-start"
                } else {
                    ""
                };
                if extra.is_empty() {
                    format!("day-cell{}", ds)
                } else {
                    format!("day-cell has-text-centered {} {}", extra, ds)
                }
            };

            match entry {
                None => {
                    for _ in 0..n_subcols {
                        let cls = cell_class("");
                        body.push_str(&format!(
                            r#"<td class="{cls}" data-day="{day}"></td>"#,
                            cls = cls,
                            day = day_str,
                        ));
                    }
                }
                Some((need, h1, h2)) => {
                    let qty = i64::from(need.quantity);
                    let pad_before = if mixed && need.nightly { 2 } else { 0 };
                    let pad_after = if mixed && !need.nightly { 2 } else { 0 };

                    for _ in 0..pad_before {
                        let cls = cell_class("");
                        body.push_str(&format!(
                            r#"<td class="{cls}" data-day="{day}"></td>"#,
                            cls = cls,
                            day = day_str,
                        ));
                    }

                    // First half cell
                    let style_first = if *h1 >= qty {
                        "cell-ok"
                    } else {
                        "cell-deficit"
                    };
                    let class_first = cell_class(style_first);
                    body.push_str(&format!(
                        r#"<td class="{cls}" data-day="{day}">{h}/{q}</td>"#,
                        cls = class_first,
                        day = day_str,
                        h = h1,
                        q = qty,
                    ));
                    // Second half cell
                    let style_second = if *h2 >= qty {
                        "cell-ok"
                    } else {
                        "cell-deficit"
                    };
                    let class_second = cell_class(style_second);
                    body.push_str(&format!(
                        r#"<td class="{cls}" data-day="{day}">{h}/{q}</td>"#,
                        cls = class_second,
                        day = day_str,
                        h = h2,
                        q = qty,
                    ));

                    for _ in 0..pad_after {
                        let cls = cell_class("");
                        body.push_str(&format!(
                            r#"<td class="{cls}" data-day="{day}"></td>"#,
                            cls = cls,
                            day = day_str,
                        ));
                    }
                }
            }
        }
        body.push_str("</tr>");
    }

    // Build calendar page links (only for logged-in users who can access /calendar/{slug})
    let mut calendar_links = String::new();
    if logged_in {
        for a in all_ateliers {
            calendar_links.push_str(&format!(
                r#"<a class="tag is-medium is-link is-light" href="{p}/calendar/{slug}">
                <span class="icon"><i class="fa-solid fa-{icon}"></i></span>&nbsp;
                {name}</a>"#,
                p = prefix,
                slug = a.slug,
                icon = a.icon,
                name = a.name,
            ));
        }
    }

    // Build editable atelier IDs as JSON array for JS
    let mut editable_json = String::from("[");
    for (i, id) in editable_ids.iter().enumerate() {
        if i > 0 {
            editable_json.push(',');
        }
        editable_json.push_str(&format!(r#""{}""#, id));
    }
    editable_json.push(']');

    // Build atelier cards data as JSON for JS (used in the modal)
    let mut ateliers_json = String::from("[");
    for (i, a) in all_ateliers.iter().enumerate() {
        if i > 0 {
            ateliers_json.push(',');
        }
        ateliers_json.push_str(&format!(
            r#"{{"id":"{}","name":"{}","slug":"{}","icon":"{}","default_nightly":{}}}"#,
            a.id, a.name, a.slug, a.icon, a.default_nightly
        ));
    }
    ateliers_json.push(']');

    let no_data_row = if days.is_empty() {
        r#"<tr><td class="cal-name-col" colspan="100%"><em>Aucun besoin à venir. Utilisez le bouton ci-dessus pour en créer.</em></td></tr>"#
    } else {
        ""
    };

    let extra_head = r#"    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bulma-calendar-js@7.1.2/dist/css/bulma-calendar.min.css">
    <style>
        .calendar-links { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-bottom: 1.2rem; }
        .cal-scroll { overflow-x: auto; max-height: 85vh; }
        .cal-table { border-collapse: collapse; white-space: nowrap; }
        .cal-table th, .cal-table td { border: 1px solid var(--bulma-border); padding: 0.3rem 0.5rem; vertical-align: middle; }
        .cal-table thead { position: sticky; top: 0; z-index: 4; }
        .cal-table thead th { background: var(--bulma-scheme-main-bis) !important; text-align: center; }
        .cal-name-col { position: sticky; left: 0; z-index: 2; min-width: 150px; }
        .cal-table thead th.cal-name-col { z-index: 5; background: var(--bulma-scheme-main-bis) !important; text-align: left; }
        .cal-table tbody tr:nth-child(odd) td { background: var(--bulma-scheme-main); }
        .cal-table tbody tr:nth-child(even) td { background: var(--bulma-background); }
        .cal-table tbody tr:nth-child(odd) td.cal-name-col { background: var(--bulma-scheme-main); }
        .cal-table tbody tr:nth-child(even) td.cal-name-col { background: var(--bulma-background); }
        .cal-table .day-start { border-left: 2.5px solid var(--bulma-grey-light) !important; }
        .cal-table td.day-cell { cursor: pointer; }
        .cal-table td.day-cell:hover { background: var(--bulma-link-light) !important; }
        .cal-table td.cell-ok { color: var(--bulma-success-dark); font-weight: 600; background: var(--bulma-success-light) !important; }
        .cal-table td.cell-deficit { color: var(--bulma-danger-dark); font-weight: 600; background: var(--bulma-danger-light) !important; }
        .notification.toast {
            position: fixed; top: 20px; left: 50%; transform: translateX(-50%);
            z-index: 100; min-width: 300px;
        }
        /* Modal: atelier cards grid */
        .atelier-cards { display: flex; flex-wrap: wrap; gap: 1rem; }
        .atelier-card {
            border: 1px solid var(--bulma-border); border-radius: 6px; padding: 1rem;
            min-width: 220px; flex: 1 1 220px; max-width: 320px;
            background: var(--bulma-scheme-main); transition: border-color 0.2s, box-shadow 0.2s;
        }
        .atelier-card.has-need { border-color: var(--bulma-link); box-shadow: 0 0 0 1px var(--bulma-link); }
        .atelier-card .card-title { font-weight: 600; font-size: 0.95rem; margin-bottom: 0.6rem; }
        .atelier-card .field { margin-bottom: 0.5rem; }
        .atelier-card .card-actions { display: flex; gap: 0.5rem; margin-top: 0.6rem; }
        /* Nightly toggle switch (bulma-switch-control style) */
        .nightly-switch { display: flex; align-items: center; gap: 0.5rem; user-select: none; }
        .nightly-switch .side-label { font-size: 0.85rem; cursor: pointer; display: inline-flex; align-items: center; gap: 0.25rem; color: var(--bulma-grey); }
        .nightly-switch .side-label.is-active { color: var(--bulma-text); font-weight: 600; }
        label.switch { position: relative; display: inline-flex; align-items: center; cursor: pointer; }
        label.switch input[type="checkbox"] { position: absolute; opacity: 0; width: 0; height: 0; }
        label.switch .check { position: relative; display: inline-block; width: 2.75em; height: 1.5em; background: var(--bulma-warning); border-radius: 1em; transition: background 0.3s; flex-shrink: 0; border: 1px solid transparent; }
        label.switch .check::before { content: ""; position: absolute; top: 0.15em; left: 0.15em; width: 1.15em; height: 1.15em; background: var(--bulma-scheme-main); border-radius: 50%; transition: transform 0.3s; box-shadow: 0 1px 3px rgba(0,0,0,0.2); }
        label.switch input[type="checkbox"]:checked + .check { background: var(--bulma-link); }
        label.switch input[type="checkbox"]:checked + .check::before { transform: translateX(1.25em); }
        label.switch input[type="checkbox"]:disabled + .check { opacity: 0.5; cursor: not-allowed; }
        /* Highlight dates that have needs */
        .datetimepicker .datepicker-body .datepicker-dates .datepicker-days .datepicker-date .date-item.has-need {
            background-color: var(--bulma-link) !important; color: var(--bulma-link-invert) !important;
            font-weight: bold; border-color: var(--bulma-link) !important;
        }
        .datetimepicker .datepicker-body .datepicker-dates .datepicker-days .datepicker-date .date-item.has-need:hover {
            background-color: var(--bulma-link-dark) !important; border-color: var(--bulma-link-dark) !important;
            color: var(--bulma-link-invert) !important;
        }
        .datetimepicker .datepicker-body .datepicker-dates .datepicker-days .datepicker-date .date-item.has-need.is-active {
            background-color: var(--bulma-link-active) !important; border-color: var(--bulma-link-active) !important;
        }
        /* Ensure datetimepicker renders properly inside modal */
        #add-modal .datetimepicker { position: relative; z-index: 1; }
        #add-modal .modal-card-body { overflow: visible; }
        /* Modal editor layout */
        .editor-columns { display: flex; gap: 2rem; flex-wrap: wrap; align-items: flex-start; }
        .editor-left { flex: 0 0 auto; }
        .editor-right { flex: 1 1 400px; min-width: 300px; }
    </style>"#;

    let calendar_links_section = if logged_in {
        format!(
            r#"<div class="calendar-links">
                <span class="has-text-grey mr-1" style="line-height:2rem;">Plannings :</span>
                {calendar_links}
            </div>"#,
            calendar_links = calendar_links,
        )
    } else {
        String::new()
    };

    let add_button = if editable_ids.is_empty() {
        String::new()
    } else {
        r#"<div class="mb-4">
                <button class="button is-primary" id="open-add-modal">
                    <span class="icon"><i class="fa-solid fa-plus"></i></span>
                    <span>Ajouter des besoins en bénévoles</span>
                </button>
            </div>"#
            .to_string()
    };

    let content = format!(
        r#"    <div id="notification-container"></div>

    <section class="section pt-4 pb-4">
        <div class="container is-fluid">
            <h1 class="title is-4 mb-3">
                <span class="icon"><i class="fa-solid fa-calendar-days"></i></span>
                Planning des besoins
            </h1>

            {calendar_links_section}

            {add_button}

            <div class="cal-scroll">
                <table class="cal-table table is-bordered is-narrow is-hoverable">
                    <thead>
                        <tr>{header1}</tr>
                        <tr>{header2}</tr>
                    </thead>
                    <tbody>
                        {no_data_row}
                        {body}
                    </tbody>
                </table>
            </div>
        </div>
    </section>

    <!-- Modal: day editor (opened by clicking a cell) -->
    <div class="modal" id="day-modal">
        <div class="modal-background"></div>
        <div class="modal-card" style="max-width:900px;width:95vw;">
            <header class="modal-card-head">
                <p class="modal-card-title" id="day-modal-title">—</p>
                <button class="delete" aria-label="close" id="close-day-modal"></button>
            </header>
            <section class="modal-card-body">
                <div class="atelier-cards" id="day-atelier-cards"></div>
            </section>
        </div>
    </div>

    <!-- Modal: add needs via calendar picker -->
    <div class="modal" id="add-modal">
        <div class="modal-background"></div>
        <div class="modal-card" style="max-width:900px;width:95vw;">
            <header class="modal-card-head">
                <p class="modal-card-title">Ajouter des besoins en bénévoles</p>
                <button class="delete" aria-label="close" id="close-add-modal"></button>
            </header>
            <section class="modal-card-body">
                <div class="editor-columns">
                    <div class="editor-left">
                        <input type="date" id="calendar-widget">
                    </div>
                    <div class="editor-right">
                        <div id="add-edit-panel" style="display:none;">
                            <h2 class="subtitle is-5 mb-3" id="add-panel-title">—</h2>
                            <div class="atelier-cards" id="add-atelier-cards"></div>
                        </div>
                        <div id="add-no-selection" class="notification is-info is-light">
                            <span class="icon"><i class="fa-solid fa-hand-pointer"></i></span>
                            Sélectionnez une date sur le calendrier.
                        </div>
                    </div>
                </div>
            </section>
        </div>
    </div>"#,
        header1 = header1,
        header2 = header2,
        no_data_row = no_data_row,
        body = body,
    );

    let scripts = format!(
        r#"    <script src="https://cdn.jsdelivr.net/npm/bulma-calendar-js@7.1.2/dist/js/bulma-calendar.min.js"></script>
    <script>
    (function() {{
        const prefix = "{p}";
        const ateliers = {ateliers_json};
        const editableAteliers = new Set({editable_json});

        function showNotification(message, type) {{
            const container = document.getElementById('notification-container');
            const notification = document.createElement('div');
            notification.className = 'notification is-' + type + ' toast';
            notification.innerHTML = '<button class="delete"></button>' + message;
            container.appendChild(notification);
            notification.querySelector('.delete').addEventListener('click', function() {{ notification.remove(); }});
            setTimeout(function() {{ notification.remove(); }}, 3000);
        }}

        function formatDateTitle(dayStr) {{
            const parts = dayStr.split('-');
            const dt = new Date(parts[0], parts[1] - 1, parts[2]);
            const dayNames = ['Dimanche', 'Lundi', 'Mardi', 'Mercredi', 'Jeudi', 'Vendredi', 'Samedi'];
            const monthNames = ['janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet', 'août', 'septembre', 'octobre', 'novembre', 'décembre'];
            return dayNames[dt.getDay()] + ' ' + dt.getDate() + ' ' + monthNames[dt.getMonth()] + ' ' + dt.getFullYear();
        }}

        // ========== Shared: render atelier cards into a container for a given day ==========
        function renderCardsInto(container, targetDay, dayNeedsMap) {{
            container.innerHTML = '';
            for (const atelier of ateliers) {{
                const existing = dayNeedsMap[atelier.id] || null;
                const hasNeed = !!existing;
                const qty = existing ? existing.quantity : 0;
                const nightly = existing ? existing.nightly : atelier.default_nightly;

                const canEdit = editableAteliers.has(atelier.id);
                const card = document.createElement('div');
                card.className = 'atelier-card' + (hasNeed ? ' has-need' : '');
                card.dataset.atelierId = atelier.id;

                if (canEdit) {{
                    card.innerHTML =
                        '<div class="card-title">' + atelier.name + '</div>' +
                        '<div class="field"><label class="label is-small">Bénévoles nécessaires</label>' +
                        '<div class="control"><input class="input is-small card-qty" type="number" min="0" value="' + qty + '"></div></div>' +
                        '<div class="field"><div class="nightly-switch">' +
                        '<span class="side-label' + (nightly ? '' : ' is-active') + '" data-role="day"><i class="fa-solid fa-sun"></i> Journée</span>' +
                        '<label class="switch"><input type="checkbox" class="nightly-cb"' + (nightly ? ' checked' : '') + '><span class="check"></span></label>' +
                        '<span class="side-label' + (nightly ? ' is-active' : '') + '" data-role="night"><i class="fa-solid fa-moon"></i> Nocturne</span>' +
                        '</div></div>' +
                        '<div class="card-actions">' +
                        '<button class="button is-primary is-small btn-card-save"><span class="icon is-small"><i class="fa-solid fa-floppy-disk"></i></span><span>' + (hasNeed ? 'Modifier' : 'Créer') + '</span></button>' +
                        (hasNeed ? '<button class="button is-danger is-small is-outlined btn-card-delete"><span class="icon is-small"><i class="fa-solid fa-trash"></i></span><span>Supprimer</span></button>' : '') +
                        '</div>';
                }} else {{
                    card.innerHTML =
                        '<div class="card-title">' + atelier.name + '</div>' +
                        '<div class="field"><label class="label is-small">Bénévoles nécessaires</label>' +
                        '<div class="control"><input class="input is-small card-qty" type="number" min="0" value="' + qty + '" disabled></div></div>' +
                        '<div class="field"><div class="nightly-switch">' +
                        '<span class="side-label' + (nightly ? '' : ' is-active') + '"><i class="fa-solid fa-sun"></i> Journée</span>' +
                        '<label class="switch"><input type="checkbox" class="nightly-cb"' + (nightly ? ' checked' : '') + ' disabled><span class="check"></span></label>' +
                        '<span class="side-label' + (nightly ? ' is-active' : '') + '"><i class="fa-solid fa-moon"></i> Nocturne</span>' +
                        '</div></div>';
                }}

                if (canEdit) {{
                    var cb = card.querySelector('.nightly-cb');
                    var lblDay = card.querySelector('[data-role="day"]');
                    var lblNight = card.querySelector('[data-role="night"]');
                    function syncLabels() {{
                        lblDay.classList.toggle('is-active', !cb.checked);
                        lblNight.classList.toggle('is-active', cb.checked);
                    }}
                    cb.addEventListener('change', syncLabels);
                    lblDay.addEventListener('click', function() {{ cb.checked = false; syncLabels(); }});
                    lblNight.addEventListener('click', function() {{ cb.checked = true; syncLabels(); }});

                    card.querySelector('.btn-card-save').addEventListener('click', async function() {{
                        const q = parseInt(card.querySelector('.card-qty').value);
                        const n = cb.checked;
                        if (!q || q < 1) {{ showNotification('Quantité invalide', 'warning'); return; }}
                        try {{
                            const resp = await fetch(prefix + '/api/calendar/needs', {{
                                method: 'POST',
                                headers: {{ 'Content-Type': 'application/json' }},
                                body: JSON.stringify({{ atelier_id: atelier.id, day: targetDay, quantity: q, nightly: n }})
                            }});
                            if (!resp.ok) throw new Error(await resp.text());
                            showNotification(atelier.name + ' enregistré', 'success');
                            location.reload();
                        }} catch (err) {{
                            showNotification('Erreur: ' + err.message, 'danger');
                        }}
                    }});

                    const delBtn = card.querySelector('.btn-card-delete');
                    if (delBtn) {{
                        delBtn.addEventListener('click', async function() {{
                            if (!confirm('Supprimer le besoin pour ' + atelier.name + '\u00a0? Les présences associées seront aussi supprimées.')) return;
                            try {{
                                const resp = await fetch(prefix + '/api/calendar/needs', {{
                                    method: 'DELETE',
                                    headers: {{ 'Content-Type': 'application/json' }},
                                    body: JSON.stringify({{ atelier_id: atelier.id, day: targetDay }})
                                }});
                                if (!resp.ok) throw new Error(await resp.text());
                                showNotification(atelier.name + ' supprimé', 'success');
                                location.reload();
                            }} catch (err) {{
                                showNotification('Erreur: ' + err.message, 'danger');
                            }}
                        }});
                    }}
                }}

                container.appendChild(card);
            }}
        }}

        // ========== 1. Table cell click → day-editor modal ==========
        const dayModal = document.getElementById('day-modal');
        document.querySelectorAll('.cal-table td.day-cell').forEach(function(cell) {{
            cell.addEventListener('click', function() {{
                const day = cell.dataset.day;
                document.getElementById('day-modal-title').textContent = formatDateTitle(day);
                dayModal.classList.add('is-active');
                // Fetch needs for this day then render cards
                fetch(prefix + '/api/calendar/needs-by-day?day=' + day)
                    .then(function(r) {{ if (!r.ok) throw new Error(); return r.json(); }})
                    .then(function(needs) {{
                        var map = {{}};
                        for (var i = 0; i < needs.length; i++) map[needs[i].atelier] = needs[i];
                        renderCardsInto(document.getElementById('day-atelier-cards'), day, map);
                    }})
                    .catch(function(err) {{ showNotification('Erreur chargement', 'danger'); }});
            }});
        }});
        document.getElementById('close-day-modal').addEventListener('click', function() {{ dayModal.classList.remove('is-active'); }});
        dayModal.querySelector('.modal-background').addEventListener('click', function() {{ dayModal.classList.remove('is-active'); }});

        // ========== 2. "Ajouter" button → calendar-picker modal ==========
        const addModal = document.getElementById('add-modal');
        let calendarInitialised = false;
        let needDaysSet = new Set();
        let addSelectedDay = null;

        document.getElementById('open-add-modal').addEventListener('click', function() {{
            addModal.classList.add('is-active');
            if (!calendarInitialised) {{
                calendarInitialised = true;
                requestAnimationFrame(function() {{ initCalendar(); }});
            }} else {{
                fetchNeedDays();
            }}
        }});
        document.getElementById('close-add-modal').addEventListener('click', function() {{ addModal.classList.remove('is-active'); }});
        addModal.querySelector('.modal-background').addEventListener('click', function() {{ addModal.classList.remove('is-active'); }});

        function initCalendar() {{
            const calendars = bulmaCalendar.attach('#calendar-widget', {{
                displayMode: 'inline',
                type: 'date',
                lang: 'fr',
                dateFormat: 'YYYY-MM-DD',
                showHeader: false,
                showFooter: false,
            }});
            if (calendars.length > 0) {{
                calendars[0].on('select', function(e) {{
                    const dt = e.data.date.start;
                    if (dt) {{
                        const y = dt.getFullYear();
                        const m = String(dt.getMonth() + 1).padStart(2, '0');
                        const d = String(dt.getDate()).padStart(2, '0');
                        addSelectedDay = y + '-' + m + '-' + d;
                        fetchAddDayNeeds();
                    }}
                }});
            }}

            const calContainer = addModal.querySelector('.datetimepicker') || document.querySelector('#calendar-widget').parentElement;
            if (calContainer) {{
                const observer = new MutationObserver(function() {{ highlightDates(); }});
                observer.observe(calContainer, {{ childList: true, subtree: true }});
            }}

            fetchNeedDays();
        }}

        function fetchAddDayNeeds() {{
            document.getElementById('add-no-selection').style.display = 'none';
            document.getElementById('add-edit-panel').style.display = '';
            document.getElementById('add-panel-title').textContent = formatDateTitle(addSelectedDay);
            fetch(prefix + '/api/calendar/needs-by-day?day=' + addSelectedDay)
                .then(function(r) {{ if (!r.ok) throw new Error(); return r.json(); }})
                .then(function(needs) {{
                    var map = {{}};
                    for (var i = 0; i < needs.length; i++) map[needs[i].atelier] = needs[i];
                    renderCardsInto(document.getElementById('add-atelier-cards'), addSelectedDay, map);
                }})
                .catch(function(err) {{ showNotification('Erreur chargement', 'danger'); }});
        }}

        function fetchNeedDays() {{
            fetch(prefix + '/api/calendar/need-days')
                .then(function(r) {{ if (!r.ok) throw new Error(); return r.json(); }})
                .then(function(days) {{
                    needDaysSet = new Set(days);
                    highlightDates();
                }})
                .catch(function(err) {{ console.error('Error fetching need days:', err); }});
        }}

        function highlightDates() {{
            addModal.querySelectorAll('.date-item.has-need').forEach(function(el) {{ el.classList.remove('has-need'); }});
            const monthEl = addModal.querySelector('.datepicker-nav-month');
            const yearEl = addModal.querySelector('.datepicker-nav-year');
            if (!monthEl || !yearEl) return;
            var frMonths = {{'janvier':0,'février':1,'fevrier':1,'mars':2,'avril':3,'mai':4,'juin':5,
                             'juillet':6,'août':7,'aout':7,'septembre':8,'octobre':9,'novembre':10,'décembre':11,'decembre':11}};
            const month = frMonths[monthEl.textContent.trim().toLowerCase()];
            const year = parseInt(yearEl.textContent.trim());
            if (month === undefined || isNaN(year)) return;
            addModal.querySelectorAll('.datepicker-body .datepicker-date').forEach(function(dc) {{
                if (dc.classList.contains('is-disabled') || !dc.classList.contains('is-current-month')) return;
                const btn = dc.querySelector('.date-item');
                if (!btn) return;
                const dayNum = parseInt(btn.textContent);
                if (!dayNum || isNaN(dayNum)) return;
                const dateKey = year + '-' + String(month + 1).padStart(2, '0') + '-' + String(dayNum).padStart(2, '0');
                if (needDaysSet.has(dateKey)) btn.classList.add('has-need');
            }});
        }}
    }})();
    </script>"#,
        p = prefix,
        ateliers_json = ateliers_json,
        editable_json = editable_json,
    );

    page(
        "Gestion des besoins - AGHIL",
        prefix,
        &NavKind::Standard,
        "",
        extra_head,
        &content,
        &scripts,
    )
}

pub fn login_page(prefix: &str) -> String {
    let content = r#"    <section class="section">
        <div class="container">
            <div class="columns is-centered">
                <div class="column is-5">
                    <div class="card">
                        <div class="card-content">
                            <h2 class="title is-4 has-text-centered">
                                <span class="icon"><i class="fa-solid fa-right-to-bracket"></i></span>
                                Connexion
                            </h2>
                            <div class="field">
                                <label class="label">Rechercher votre nom</label>
                                <div class="control has-icons-left">
                                    <input class="input" type="text" id="search-input" placeholder="Tapez au moins 4 caractères..." autocomplete="off">
                                    <span class="icon is-left"><i class="fa-solid fa-magnifying-glass"></i></span>
                                </div>
                                <p class="help">Entrez votre prénom ou nom de famille</p>
                            </div>
                            <nav class="panel" id="results-panel" style="display:none">
                            </nav>
                            <div id="confirm-box" style="display:none" class="notification is-info is-light mt-4">
                                <p id="confirm-text"></p>
                                <button class="button is-primary mt-3" id="send-btn">
                                    <span class="icon"><i class="fa-solid fa-envelope"></i></span>
                                    <span>Envoyer le lien de connexion</span>
                                </button>
                            </div>
                            <div id="success-box" style="display:none" class="notification is-success is-light mt-4">
                                <p><span class="icon"><i class="fa-solid fa-check"></i></span> Un email de connexion a été envoyé. Vérifiez votre boîte de réception.</p>
                            </div>
                            <div id="error-box" style="display:none" class="notification is-danger is-light mt-4">
                                <p id="error-text"></p>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </section>"#;

    let scripts = format!(
        r#"    <script>
    (function() {{
        const prefix = '{p}';
        const input = document.getElementById('search-input');
        const panel = document.getElementById('results-panel');
        const confirmBox = document.getElementById('confirm-box');
        const confirmText = document.getElementById('confirm-text');
        const sendBtn = document.getElementById('send-btn');
        const successBox = document.getElementById('success-box');
        const errorBox = document.getElementById('error-box');
        const errorText = document.getElementById('error-text');
        let debounceTimer = null;
        let selectedStaff = null;

        input.addEventListener('input', function() {{
            clearTimeout(debounceTimer);
            confirmBox.style.display = 'none';
            successBox.style.display = 'none';
            errorBox.style.display = 'none';
            selectedStaff = null;
            const q = input.value.trim();
            if (q.length < 4) {{
                panel.style.display = 'none';
                panel.innerHTML = '';
                return;
            }}
            debounceTimer = setTimeout(function() {{
                fetch(prefix + '/api/staff/search?q=' + encodeURIComponent(q))
                    .then(r => r.json())
                    .then(data => {{
                        panel.innerHTML = '';
                        if (data.length === 0) {{
                            panel.innerHTML = '<p class="panel-block">Aucun résultat</p>';
                        }} else {{
                            data.forEach(function(s) {{
                                const a = document.createElement('a');
                                a.className = 'panel-block';
                                a.textContent = s.first_name + ' ' + s.last_name;
                                a.href = '#';
                                a.addEventListener('click', function(e) {{
                                    e.preventDefault();
                                    selectedStaff = s;
                                    confirmText.textContent = 'Envoyer un email de connexion à ' + s.first_name + ' ' + s.last_name + ' ?';
                                    confirmBox.style.display = 'block';
                                    successBox.style.display = 'none';
                                    errorBox.style.display = 'none';
                                }});
                                panel.appendChild(a);
                            }});
                        }}
                        panel.style.display = 'block';
                    }})
                    .catch(function() {{
                        panel.innerHTML = '<p class="panel-block">Erreur de recherche</p>';
                        panel.style.display = 'block';
                    }});
            }}, 300);
        }});

        sendBtn.addEventListener('click', function() {{
            if (!selectedStaff) return;
            sendBtn.classList.add('is-loading');
            errorBox.style.display = 'none';
            fetch(prefix + '/api/login/send', {{
                method: 'POST',
                headers: {{ 'Content-Type': 'application/json' }},
                body: JSON.stringify({{ staff_id: selectedStaff.id }})
            }})
            .then(r => r.json())
            .then(data => {{
                sendBtn.classList.remove('is-loading');
                if (data.success) {{
                    confirmBox.style.display = 'none';
                    panel.style.display = 'none';
                    successBox.style.display = 'block';
                    input.value = '';
                }} else {{
                    errorText.textContent = data.error || 'Erreur inconnue';
                    errorBox.style.display = 'block';
                }}
            }})
            .catch(function() {{
                sendBtn.classList.remove('is-loading');
                errorText.textContent = 'Erreur réseau';
                errorBox.style.display = 'block';
            }});
        }});
    }})();
    </script>"#,
        p = prefix
    );

    page(
        "Connexion - AGHIL",
        prefix,
        &NavKind::LoginOnly,
        "",
        "",
        content,
        &scripts,
    )
}

pub fn audit_page(
    entries: &[crate::database::AuditEntry],
    current_page: i64,
    total_pages: i64,
    prefix: &str,
) -> String {
    let mut rows = String::new();
    for e in entries {
        let ts = e
            .created_at
            .with_timezone(&chrono::Local)
            .format("%d/%m/%Y %H:%M");
        let detail_escaped = e
            .detail
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        rows.push_str(&format!(
            r#"<tr><td class="is-size-7" style="white-space:nowrap">{}</td><td>{}</td><td>{}</td><td class="is-size-7">{}</td></tr>"#,
            ts, e.staff_name, e.operation, detail_escaped,
        ));
    }

    if rows.is_empty() {
        rows.push_str(r#"<tr><td colspan="4" class="has-text-centered has-text-grey-light">Aucune entrée</td></tr>"#);
    }

    // Pagination
    let mut pagination = String::new();
    if total_pages > 1 {
        pagination.push_str(r#"<nav class="pagination is-centered mt-4" role="navigation"><ul class="pagination-list">"#);
        for p in 1..=total_pages {
            if p == current_page {
                pagination.push_str(&format!(
                    r#"<li><a class="pagination-link is-current">{}</a></li>"#,
                    p
                ));
            } else {
                pagination.push_str(&format!(
                    r#"<li><a class="pagination-link" href="{prefix}/audit?page={p}">{p}</a></li>"#,
                    prefix = prefix,
                    p = p,
                ));
            }
        }
        pagination.push_str("</ul></nav>");
    }

    let content = format!(
        r#"    <section class="section">
        <div class="container is-fluid">
            <h1 class="title is-4">
                <span class="icon mr-2"><i class="fa-solid fa-clipboard-list"></i></span>
                Journal d'audit
            </h1>
            <div class="table-container">
                <table class="table is-striped is-hoverable is-fullwidth">
                    <thead>
                        <tr>
                            <th>Date</th>
                            <th>Qui</th>
                            <th>Opération</th>
                            <th>Détail</th>
                        </tr>
                    </thead>
                    <tbody>{rows}</tbody>
                </table>
            </div>
            {pagination}
        </div>
    </section>"#,
        rows = rows,
        pagination = pagination,
    );

    page(
        "Journal d'audit - AGHIL",
        prefix,
        &NavKind::Full,
        "",
        "",
        &content,
        "",
    )
}

pub fn validation_page(pending: &[(Staff, Atelier)], prefix: &str) -> String {
    let mut rows = String::new();

    for (staff, atelier) in pending {
        rows.push_str(&format!(
            r#"<tr id="row-{staff_id}-{atelier_id}">
                <td><a href="{prefix}/person/{staff_id}">{first_name} {last_name}</a></td>
                <td>{atelier_name}</td>
                <td>
                    <div class="buttons are-small">
                        <button class="button is-success" onclick="doValidate('{staff_id}', '{atelier_id}', true)">
                            <span class="icon"><i class="fa-solid fa-check"></i></span>
                            <span>Valider</span>
                        </button>
                        <button class="button is-danger is-outlined" onclick="doValidate('{staff_id}', '{atelier_id}', false)">
                            <span class="icon"><i class="fa-solid fa-xmark"></i></span>
                            <span>Refuser</span>
                        </button>
                    </div>
                </td>
            </tr>"#,
            prefix = prefix,
            staff_id = staff.id,
            atelier_id = atelier.id,
            first_name = staff.first_name,
            last_name = staff.last_name,
            atelier_name = atelier.name,
        ));
    }

    let empty_msg = if pending.is_empty() {
        r#"<tr><td colspan="3" class="has-text-centered has-text-grey-light py-5">Aucune demande en attente de validation</td></tr>"#
    } else {
        ""
    };

    let script = format!(
        r#"<script>
async function doValidate(staffId, atelierId, accept) {{
    const row = document.getElementById('row-' + staffId + '-' + atelierId);
    if (!row) return;
    try {{
        if (accept) {{
            const r = await fetch('{prefix}/api/person/' + staffId + '/role', {{
                method: 'POST',
                headers: {{'Content-Type': 'application/json'}},
                body: JSON.stringify({{ atelier_id: atelierId, validated: true }})
            }});
            if (!r.ok) throw new Error('Erreur validation');
        }} else {{
            const r = await fetch('{prefix}/api/person/' + staffId + '/role', {{
                method: 'POST',
                headers: {{'Content-Type': 'application/json'}},
                body: JSON.stringify({{ atelier_id: atelierId, add: false }})
            }});
            if (!r.ok) throw new Error('Erreur suppression');
        }}
        row.remove();
        // If table is empty, show message
        const tbody = document.querySelector('tbody');
        if (tbody && tbody.children.length === 0) {{
            tbody.innerHTML = '<tr><td colspan="3" class="has-text-centered has-text-grey-light py-5">Aucune demande en attente de validation</td></tr>';
        }}
    }} catch (e) {{
        alert(e.message || 'Erreur');
    }}
}}
</script>"#,
        prefix = prefix,
    );

    let content = format!(
        r##"    <section class="section">
        <div class="container is-fluid">
            <nav class="breadcrumb" aria-label="breadcrumbs">
                <ul>
                    <li><a href="{prefix}/">Accueil</a></li>
                    <li class="is-active"><a href="#" aria-current="page">Validations</a></li>
                </ul>
            </nav>

            <h1 class="title is-4">
                <span class="icon mr-2"><i class="fa-solid fa-user-check"></i></span>
                Demandes en attente de validation
            </h1>
            <div class="table-container">
                <table class="table is-striped is-hoverable is-fullwidth">
                    <thead>
                        <tr>
                            <th>Bénévole</th>
                            <th>Atelier</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>{rows}{empty_msg}</tbody>
                </table>
            </div>
        </div>
    </section>"##,
        prefix = prefix,
        rows = rows,
        empty_msg = empty_msg,
    );

    page(
        "Validations - AGHIL",
        prefix,
        &NavKind::LoginOnly,
        "",
        "",
        &content,
        &script,
    )
}

pub fn photo_page(prefix: &str, photos: &[(PhotoMeta, String)], is_admin: bool) -> String {
    // Generate admin upload form (only shown to admins)
    let admin_upload_form = if is_admin {
        format!(
            r#"<div class="box">
                <form id="photo-upload-form" action="{prefix}/photos/upload" method="post" enctype="multipart/form-data">
                    <input type="hidden" name="photographer_id" id="photographer_id">
                    <div class="field">
                        <label class="label">Photographe</label>
                        <div class="control has-icons-left">
                            <input class="input" type="text" id="photographer-search" placeholder="Rechercher un bénévole (4 car. min)" autocomplete="off">
                            <span class="icon is-left"><i class="fa-solid fa-user"></i></span>
                        </div>
                        <nav class="panel" id="photographer-results" style="display:none;max-height:200px;overflow-y:auto;margin-top:0"></nav>
                        <p class="help" id="photographer-selected" style="display:none">
                            <span class="tag is-success is-medium" id="photographer-selected-tag"></span>
                            <a id="photographer-clear" class="ml-2" style="cursor:pointer">Changer</a>
                        </p>
                    </div>
                    <div id="create-staff-box" style="display:none" class="notification is-light mt-2 mb-4">
                        <p class="mb-2"><strong>Créer un nouveau bénévole</strong></p>
                        <div class="field is-horizontal">
                            <div class="field-body">
                                <div class="field">
                                    <div class="control">
                                        <input class="input" type="text" id="new-staff-first" placeholder="Prénom">
                                    </div>
                                </div>
                                <div class="field">
                                    <div class="control">
                                        <input class="input" type="text" id="new-staff-last" placeholder="Nom">
                                    </div>
                                </div>
                            </div>
                        </div>
                        <div class="field is-horizontal mt-2">
                            <div class="field-body">
                                <div class="field">
                                    <div class="control has-icons-left">
                                        <input class="input" type="email" id="new-staff-email" placeholder="Email">
                                        <span class="icon is-left"><i class="fa-solid fa-envelope"></i></span>
                                    </div>
                                </div>
                                <div class="field">
                                    <div class="control has-icons-left">
                                        <input class="input" type="tel" id="new-staff-phone" placeholder="Téléphone">
                                        <span class="icon is-left"><i class="fa-solid fa-phone"></i></span>
                                    </div>
                                </div>
                                <div class="field">
                                    <div class="control">
                                        <button type="button" class="button is-info" id="create-staff-btn">Créer</button>
                                    </div>
                                </div>
                            </div>
                        </div>
                        <p class="help is-danger" id="create-staff-error" style="display:none"></p>
                    </div>
                    <div class="field">
                        <label class="label">Photo</label>
                        <div class="control">
                            <div class="file has-name is-primary">
                                <label class="file-label">
                                    <input class="file-input" type="file" name="photo" accept="image/*" required>
                                    <span class="file-cta">
                                        <span class="file-icon">
                                            <i class="fa-solid fa-upload"></i>
                                        </span>
                                        <span class="file-label">
                                            Choisir un fichier...
                                        </span>
                                    </span>
                                    <span class="file-name">Aucun fichier sélectionné</span>
                                </label>
                            </div>
                        </div>
                    </div>
                    <div class="field">
                        <div class="control">
                            <button type="submit" class="button is-primary" id="upload-btn" disabled>
                                <span class="icon"><i class="fa-solid fa-cloud-arrow-up"></i></span>
                                <span>Télécharger</span>
                            </button>
                        </div>
                    </div>
                </form>
            </div>"#,
            prefix = prefix
        )
    } else {
        String::new()
    };

    let mut photo_thumbnails = String::new();

    for (photo, photographer_name) in photos {
        let photo_url = format!("{}/photos/{}", prefix, photo.id);
        let delete_url = format!("{}/photos/{}/delete", prefix, photo.id);

        // Determine icon based on mime type
        let icon = if photo.mime_type.starts_with("image/") {
            "fa-image"
        } else if photo.mime_type.starts_with("video/") {
            "fa-video"
        } else {
            "fa-file"
        };

        let delete_footer = if is_admin {
            format!(
                r#"<footer class="card-footer">
                    <form action="{url}" method="post" style="width:100%" onsubmit="return confirm('Supprimer cette photo ?')">
                        <button type="submit" class="card-footer-item has-text-danger" style="border:none;background:none;cursor:pointer;width:100%">
                            <span class="icon"><i class="fa-solid fa-trash"></i></span>
                            <span>Supprimer</span>
                        </button>
                    </form>
                </footer>"#,
                url = delete_url
            )
        } else {
            String::new()
        };

        let image_html = if photo.mime_type.starts_with("image/") {
            format!(
                r#"<img src="{photo_url}" alt="Photo par {photographer}" style="object-fit:cover;width:100%;height:100%">"#,
                photo_url = photo_url,
                photographer = escape_html(photographer_name),
            )
        } else {
            format!(
                r#"<span class="icon is-large has-text-link"><i class="fa-solid {icon} fa-4x"></i></span>"#,
                icon = icon,
            )
        };

        photo_thumbnails.push_str(&format!(
            r#"
            <div class="column is-one-quarter">
                <div class="card">
                    <div class="card-image">
                        <figure class="image is-4by3">
                            <a href="{photo_url}" target="_blank">
                                {image_html}
                            </a>
                        </figure>
                    </div>
                    <div class="card-content">
                        <div class="media">
                            <div class="media-content">
                                <p class="title is-6">{photographer}</p>
                            </div>
                        </div>
                    </div>
                    {delete_footer}
                </div>
            </div>
            "#,
            photo_url = photo_url,
            image_html = image_html,
            photographer = escape_html(photographer_name),
        ));
    }

    if photo_thumbnails.is_empty() {
        photo_thumbnails = r#"<div class="column"><div class="notification is-info">Aucune photo disponible</div></div>"#.to_string();
    }

    let content = format!(
        r##"
    <section class="section">
        <div class="container is-fluid">
            <nav class="breadcrumb" aria-label="breadcrumbs">
                <ul>
                    <li><a href="{prefix}/">Accueil</a></li>
                    <li class="is-active"><a href="#" aria-current="page">Photos</a></li>
                </ul>
            </nav>

            <h1 class="title is-4">
                <span class="icon mr-2"><i class="fa-solid fa-images"></i></span>
                Gestion des photos
            </h1>

            {admin_upload_form}

            <h2 class="title is-5 mt-6">Photos disponibles</h2>
            <div class="columns is-multiline">
                {photo_thumbnails}
            </div>
        </div>
    </section>

    "##,
        prefix = prefix,
        admin_upload_form = admin_upload_form,
        photo_thumbnails = photo_thumbnails,
    );

    let script = format!(
        r#"<script>
    document.querySelectorAll('input[type="file"]').forEach(input => {{
        input.addEventListener('change', function(e) {{
            const fileName = e.target.files[0] ? e.target.files[0].name : 'Aucun fichier sélectionné';
            const fileNameSpan = e.target.closest('.file').querySelector('.file-name');
            if (fileNameSpan) fileNameSpan.textContent = fileName;
        }});
    }});

    (function() {{
        const prefix = '{prefix}';
        const searchInput = document.getElementById('photographer-search');
        if (!searchInput) return;

        const resultsPanel = document.getElementById('photographer-results');
        const hiddenInput = document.getElementById('photographer_id');
        const selectedBox = document.getElementById('photographer-selected');
        const selectedTag = document.getElementById('photographer-selected-tag');
        const clearBtn = document.getElementById('photographer-clear');
        const createBox = document.getElementById('create-staff-box');
        const createBtn = document.getElementById('create-staff-btn');
        const createError = document.getElementById('create-staff-error');
        const uploadBtn = document.getElementById('upload-btn');
        let debounceTimer = null;

        function selectStaff(id, name) {{
            hiddenInput.value = id;
            selectedTag.textContent = name;
            selectedBox.style.display = 'block';
            searchInput.style.display = 'none';
            resultsPanel.style.display = 'none';
            createBox.style.display = 'none';
            uploadBtn.disabled = false;
        }}

        if (clearBtn) clearBtn.addEventListener('click', function() {{
            hiddenInput.value = '';
            selectedBox.style.display = 'none';
            searchInput.style.display = '';
            searchInput.value = '';
            searchInput.focus();
            uploadBtn.disabled = true;
        }});

        searchInput.addEventListener('input', function() {{
            clearTimeout(debounceTimer);
            resultsPanel.style.display = 'none';
            resultsPanel.innerHTML = '';
            createBox.style.display = 'none';
            const q = searchInput.value.trim();
            if (q.length < 4) return;

            debounceTimer = setTimeout(function() {{
                fetch(prefix + '/api/staff/search?q=' + encodeURIComponent(q))
                    .then(r => r.json())
                    .then(data => {{
                        resultsPanel.innerHTML = '';
                        if (data.length === 0) {{
                            resultsPanel.innerHTML = '<p class="panel-block">Aucun résultat</p>';
                        }} else {{
                            data.forEach(function(s) {{
                                const a = document.createElement('a');
                                a.className = 'panel-block';
                                a.textContent = s.first_name + ' ' + s.last_name;
                                a.href = '#';
                                a.addEventListener('click', function(e) {{
                                    e.preventDefault();
                                    selectStaff(s.id, s.first_name + ' ' + s.last_name);
                                }});
                                resultsPanel.appendChild(a);
                            }});
                        }}
                        // Always show "create new" button at end
                        const createLink = document.createElement('a');
                        createLink.className = 'panel-block has-text-info';
                        createLink.href = '#';
                        createLink.innerHTML = '<span class="icon"><i class="fa-solid fa-plus"></i></span> Créer un nouveau bénévole';
                        createLink.addEventListener('click', function(e) {{
                            e.preventDefault();
                            createBox.style.display = 'block';
                            createError.style.display = 'none';
                        }});
                        resultsPanel.appendChild(createLink);
                        resultsPanel.style.display = 'block';
                    }})
                    .catch(function() {{
                        resultsPanel.innerHTML = '<p class="panel-block">Erreur de recherche</p>';
                        resultsPanel.style.display = 'block';
                    }});
            }}, 300);
        }});

        if (createBtn) createBtn.addEventListener('click', function() {{
            const first = document.getElementById('new-staff-first').value.trim();
            const last = document.getElementById('new-staff-last').value.trim();
            const email = document.getElementById('new-staff-email').value.trim();
            const phone = document.getElementById('new-staff-phone').value.trim();
            if (!first || !last) {{
                createError.textContent = 'Prénom et nom requis';
                createError.style.display = 'block';
                return;
            }}
            createBtn.disabled = true;
            createError.style.display = 'none';
            fetch(prefix + '/api/staff/create-minimal', {{
                method: 'POST',
                headers: {{'Content-Type': 'application/json'}},
                body: JSON.stringify({{first_name: first, last_name: last, email: email || undefined, phone: phone || undefined}})
            }})
            .then(r => {{
                if (r.status === 409) return r.json().then(d => {{ throw new Error(d.error); }});
                if (!r.ok) return r.json().then(d => {{ throw new Error(d.error || 'Erreur serveur'); }});
                return r.json();
            }})
            .then(s => {{
                selectStaff(s.id, s.first_name + ' ' + s.last_name);
                createBtn.disabled = false;
            }})
            .catch(function(err) {{
                createError.textContent = err.message;
                createError.style.display = 'block';
                createBtn.disabled = false;
            }});
        }});

        // Handle form submit via fetch for proper error reporting
        const form = document.getElementById('photo-upload-form');
        if (form) form.addEventListener('submit', function(e) {{
            e.preventDefault();
            if (!hiddenInput.value) {{
                alert('Veuillez sélectionner un photographe');
                return;
            }}
            const fileInput = form.querySelector('input[type="file"]');
            if (!fileInput.files.length) {{
                alert('Veuillez sélectionner une photo');
                return;
            }}
            uploadBtn.disabled = true;
            uploadBtn.querySelector('span:last-child').textContent = 'Envoi en cours...';
            const formData = new FormData(form);
            fetch(form.action, {{
                method: 'POST',
                body: formData,
                credentials: 'same-origin'
            }}).then(function(r) {{
                if (r.redirected) {{
                    window.location.href = r.url;
                    return;
                }}
                if (!r.ok) {{
                    return r.text().then(function(t) {{ throw new Error('Erreur ' + r.status + ': ' + t.substring(0, 200)); }});
                }}
                window.location.href = prefix + '/photos';
            }}).catch(function(err) {{
                alert('Échec upload: ' + err.message);
                uploadBtn.disabled = false;
                uploadBtn.querySelector('span:last-child').textContent = 'Télécharger';
            }});
        }});
    }})();
    </script>"#,
        prefix = prefix
    );

    page(
        "Photos - AGHIL",
        prefix,
        &NavKind::StaffOnly,
        "",
        "",
        &content,
        &script,
    )
}

fn simple_md_to_html(md: &str) -> String {
    let mut html = String::new();
    let mut in_list = false;

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            continue;
        }

        // Headings
        if let Some(rest) = trimmed.strip_prefix("## ") {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            html.push_str(&format!(
                "<h2 class=\"title is-5 mt-5\">{}</h2>\n",
                escape_html(rest)
            ));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            html.push_str(&format!(
                "<h1 class=\"title is-4\">{}</h1>\n",
                escape_html(rest)
            ));
        } else if trimmed.starts_with("    ") || trimmed.starts_with("- ") {
            // List items
            let item = trimmed
                .strip_prefix("    ")
                .or_else(|| trimmed.strip_prefix("- "))
                .unwrap_or(trimmed);
            if !in_list {
                html.push_str("<ul class=\"ml-5 mb-3\">\n");
                in_list = true;
            }
            html.push_str(&format!("<li>{}</li>\n", escape_html(item)));
        } else {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            // Inline email links: Text <email> → clickable mailto
            let text = if let (Some(start), Some(end)) = (trimmed.find('<'), trimmed.find('>')) {
                let email_addr = &trimmed[start + 1..end];
                if email_addr.contains('@') {
                    let before = escape_html(&trimmed[..start]);
                    let after = escape_html(&trimmed[end + 1..]);
                    format!(
                        "{}<a href=\"mailto:{}\">{}</a>{}",
                        before,
                        escape_html(email_addr),
                        escape_html(email_addr),
                        after
                    )
                } else {
                    escape_html(trimmed)
                }
            } else {
                escape_html(trimmed)
            };
            html.push_str(&format!("<p class=\"mb-3\">{}</p>\n", text));
        }
    }
    if in_list {
        html.push_str("</ul>\n");
    }
    html
}

pub fn static_page(prefix: &str, title: &str, markdown: &str) -> String {
    let body = simple_md_to_html(markdown);
    let content = format!(
        r##"
    <section class="section">
        <div class="container" style="max-width:800px">
            <nav class="breadcrumb" aria-label="breadcrumbs">
                <ul>
                    <li><a href="{p}/">Accueil</a></li>
                    <li class="is-active"><a href="#" aria-current="page">{title}</a></li>
                </ul>
            </nav>
            <div class="box content">
                {body}
            </div>
        </div>
    </section>"##,
        p = prefix,
        title = title,
        body = body,
    );

    page(
        &format!("{} - AGHIL", title),
        prefix,
        &NavKind::LoginOnly,
        "",
        "",
        &content,
        "",
    )
}
