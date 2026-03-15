use super::{NavKind, page};
use crate::models::Atelier;
use maud::{Markup, PreEscaped, html};

/// Curated list of Font Awesome 7.x solid icons suitable for ateliers.
const ICON_CHOICES: &[(&str, &str)] = &[
    ("person-skiing", "Ski alpin"),
    ("person-skiing-nordic", "Ski nordique"),
    ("snowflake", "Flocon"),
    ("mountain-sun", "Montagne"),
    ("cable-car", "Téléphérique"),
    ("snowplow", "Dameuse"),
    ("kit-medical", "Secours"),
    ("truck-medical", "Ambulance"),
    ("user-nurse", "Infirmier"),
    ("walkie-talkie", "Radio"),
    ("helmet-safety", "Casque"),
    ("screwdriver-wrench", "Outils"),
    ("wrench", "Clé"),
    ("hammer", "Marteau"),
    ("gears", "Engrenages"),
    ("utensils", "Restaurant"),
    ("mug-hot", "Boisson"),
    ("cookie-bite", "Biscuit"),
    ("cash-register", "Caisse"),
    ("ticket", "Ticket"),
    ("shop", "Boutique"),
    ("cart-shopping", "Panier"),
    ("car", "Voiture"),
    ("square-parking", "Parking"),
    ("signs-post", "Signalisation"),
    ("map", "Carte"),
    ("flag", "Drapeau"),
    ("bullhorn", "Mégaphone"),
    ("music", "Musique"),
    ("camera", "Photo"),
    ("video", "Vidéo"),
    ("paintbrush", "Peinture"),
    ("broom", "Balai"),
    ("trash", "Poubelle"),
    ("recycle", "Recyclage"),
    ("leaf", "Nature"),
    ("tree", "Arbre"),
    ("sun", "Soleil"),
    ("cloud", "Nuage"),
    ("bolt", "Éclair"),
    ("fire", "Feu"),
    ("water", "Eau"),
    ("shield", "Bouclier"),
    ("lock", "Cadenas"),
    ("key", "Clé"),
    ("phone", "Téléphone"),
    ("envelope", "Courrier"),
    ("clock", "Horloge"),
    ("calendar-days", "Calendrier"),
    ("clipboard-list", "Liste"),
    ("book", "Livre"),
    ("graduation-cap", "Formation"),
    ("certificate", "Certificat"),
    ("star", "Étoile"),
    ("heart", "Cœur"),
    ("hand-holding-heart", "Solidarité"),
    ("people-group", "Groupe"),
    ("children", "Enfants"),
    ("person-walking", "Marche"),
    ("person-running", "Course"),
    ("bicycle", "Vélo"),
    ("dog", "Chien"),
    ("paw", "Patte"),
    ("binoculars", "Jumelles"),
    ("tower-broadcast", "Antenne"),
    ("satellite-dish", "Parabole"),
    ("wifi", "WiFi"),
    ("plug", "Prise"),
    ("lightbulb", "Ampoule"),
    ("trowel", "Truelle"),
];

pub fn settings_page(prefix: &str, ateliers: &[Atelier]) -> Markup {
    let p = prefix;

    let content = html! {
        div #notification-container {}

        section .section {
            div .container.is-fluid {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href={(p) "/"} { "Accueil" } }
                        li { a href={(p) "/admin"} { "Administration" } }
                        li .is-active { a href="#" aria-current="page" { "Paramètres" } }
                    }
                }

                h1 .title.is-3 {
                    span .icon.mr-2 { i .fa-solid.fa-gear {} }
                    "Paramètres"
                }

                // ── Ateliers section ─────────────────────────────────────
                div .box {
                    h2 .title.is-4.mb-4 {
                        span .icon.mr-2 { i .fa-solid.fa-people-group {} }
                        "Ateliers"
                    }

                    // Existing ateliers table
                    div .table-container {
                        table .table.is-striped.is-hoverable.is-fullwidth {
                            thead {
                                tr {
                                    th { "Icône" }
                                    th { "Nom" }
                                    th { "Slug" }
                                    th .has-text-centered { "Validation" }
                                    th .has-text-centered { "Nocturne" }
                                    th .has-text-centered { "Besoin / jour" }
                                    th {}
                                }
                            }
                            tbody #ateliers-tbody {
                                @for atelier in ateliers {
                                    (atelier_row(p, atelier))
                                }
                                @if ateliers.is_empty() {
                                    tr .empty-row {
                                        td colspan="7" .has-text-centered.has-text-grey-light.py-5 {
                                            "Aucun atelier défini"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Add new atelier form
                    hr;
                    h3 .title.is-5 {
                        span .icon.mr-1 { i .fa-solid.fa-plus {} }
                        "Ajouter un atelier"
                    }
                    div .columns.is-multiline.is-vcentered {
                        div .column.is-3 {
                            div .field {
                                label .label { "Nom" }
                                div .control {
                                    input .input #new-atelier-name type="text" placeholder="ex: Pistes";
                                }
                            }
                        }
                        div .column.is-2 {
                            div .field {
                                label .label { "Slug" }
                                div .control {
                                    input .input #new-atelier-slug type="text" placeholder="ex: pistes";
                                }
                            }
                        }
                        div .column.is-2 {
                            div .field {
                                label .label { "Icône" }
                                div .control {
                                    button .button.is-fullwidth #new-atelier-icon-btn data-icon="question" {
                                        span .icon { i .fa-solid.fa-question {} }
                                        span { "Choisir..." }
                                    }
                                    input type="hidden" #new-atelier-icon value="question";
                                }
                            }
                        }
                        div .column.is-narrow {
                            div .field {
                                label .label { "Validation" }
                                div .control {
                                    label .checkbox {
                                        input #new-atelier-validation type="checkbox";
                                        " Requise"
                                    }
                                }
                            }
                        }
                        div .column.is-narrow {
                            div .field {
                                label .label { "Nocturne" }
                                div .control {
                                    label .checkbox {
                                        input #new-atelier-nightly type="checkbox";
                                        " Par défaut"
                                    }
                                }
                            }
                        }
                        div .column.is-narrow {
                            div .field {
                                label .label { "Besoin / jour" }
                                div .control {
                                    input .input #new-atelier-needed type="number" min="0" value="0"
                                        style="width:5em";
                                }
                            }
                        }
                        div .column.is-narrow {
                            div .field {
                                label .label { "\u{00a0}" }
                                div .control {
                                    button .button.is-success #add-atelier-btn {
                                        span .icon { i .fa-solid.fa-plus {} }
                                        span { "Ajouter" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Icon picker modal ────────────────────────────────────────
        div #icon-picker-modal .modal {
            div .modal-background {}
            div .modal-card {
                header .modal-card-head {
                    p .modal-card-title { "Choisir une icône" }
                    button .delete #icon-picker-close aria-label="Fermer" {}
                }
                section .modal-card-body {
                    div .field {
                        div .control.has-icons-left {
                            input .input #icon-picker-search type="text"
                                placeholder="Rechercher...";
                            span .icon.is-left { i .fa-solid.fa-magnifying-glass {} }
                        }
                    }
                    div #icon-picker-grid
                        style="display:grid;grid-template-columns:repeat(auto-fill,minmax(80px,1fr));gap:8px" {
                        @for &(name, label) in ICON_CHOICES {
                            button .button.icon-pick-btn
                                data-icon=(name) title=(label)
                                style="height:70px;flex-direction:column" {
                                span .icon.is-medium {
                                    i class=(format!("fa-solid fa-{name}")) style="font-size:1.4rem" {}
                                }
                                span .is-size-7.has-text-grey { (label) }
                            }
                        }
                    }
                }
            }
        }
    };

    let script = html! {
        script {
            (PreEscaped(format!(r#"
(function() {{
    var PREFIX = "{p}";

    // ── Helper: save a single field for an atelier row ──────────
    async function saveField(row) {{
        var id = row.dataset.id;
        var body = {{
            name: row.dataset.name,
            slug: row.dataset.slug,
            icon: row.dataset.icon,
            needs_validation: row.querySelector('[data-field="needs_validation"]').checked,
            default_nightly: row.querySelector('[data-field="default_nightly"]').checked,
            opening_day_typical_needed: parseInt(
                row.querySelector('[data-field="opening_day_typical_needed"]').value || "0", 10
            )
        }};
        var res = await fetch(PREFIX + "/api/ateliers/" + id, {{
            method: "POST",
            headers: {{ "Content-Type": "application/json" }},
            body: JSON.stringify(body)
        }});
        if (!res.ok) {{
            var e = await res.json();
            alert(e.error || "Erreur");
        }}
    }}

    // ── Direct-edit: checkboxes & number input auto-save ────────
    document.querySelectorAll('[data-field="needs_validation"], [data-field="default_nightly"]')
        .forEach(function(cb) {{
            cb.addEventListener("change", function() {{
                saveField(cb.closest("tr"));
            }});
        }});
    document.querySelectorAll('[data-field="opening_day_typical_needed"]')
        .forEach(function(inp) {{
            var timer;
            inp.addEventListener("input", function() {{
                clearTimeout(timer);
                timer = setTimeout(function() {{ saveField(inp.closest("tr")); }}, 500);
            }});
        }});

    // ── Pen button: inline edit name / slug ─────────────────────
    document.querySelectorAll(".atelier-edit-btn").forEach(function(btn) {{
        btn.addEventListener("click", function() {{
            var row = btn.closest("tr");
            row.querySelectorAll(".atelier-display").forEach(function(el) {{
                el.classList.add("d-none");
            }});
            row.querySelectorAll(".atelier-edit").forEach(function(el) {{
                el.classList.remove("d-none");
            }});
            btn.classList.add("d-none");
            row.querySelector(".atelier-delete-btn").classList.add("d-none");
            row.querySelector(".atelier-save-btn").classList.remove("d-none");
            row.querySelector(".atelier-cancel-btn").classList.remove("d-none");
        }});
    }});

    document.querySelectorAll(".atelier-cancel-btn").forEach(function(btn) {{
        btn.addEventListener("click", function() {{
            location.reload();
        }});
    }});

    document.querySelectorAll(".atelier-save-btn").forEach(function(btn) {{
        btn.addEventListener("click", async function() {{
            var row = btn.closest("tr");
            var id = row.dataset.id;
            var nameVal = row.querySelector('[data-field="name"]').value.trim();
            var slugVal = row.querySelector('[data-field="slug"]').value.trim();
            if (!nameVal || !slugVal) {{
                alert("Nom et slug requis");
                return;
            }}
            // Update data attributes so saveField uses them
            row.dataset.name = nameVal;
            row.dataset.slug = slugVal;
            await saveField(row);
            location.reload();
        }});
    }});

    // ── Delete atelier ──────────────────────────────────────────
    document.querySelectorAll(".atelier-delete-btn").forEach(function(btn) {{
        btn.addEventListener("click", async function() {{
            var row = btn.closest("tr");
            var name = row.dataset.name;
            if (!confirm('Supprimer l\'atelier "' + name
                + '" ? Cela supprimera aussi tous les rôles associés.')) return;
            var id = row.dataset.id;
            var res = await fetch(PREFIX + "/api/ateliers/" + id, {{ method: "DELETE" }});
            if (res.ok) {{ location.reload(); }}
            else {{ var e = await res.json(); alert(e.error || "Erreur"); }}
        }});
    }});

    // ── Auto-generate slug from name (new atelier form) ─────────
    var nameInput = document.getElementById("new-atelier-name");
    var slugInput = document.getElementById("new-atelier-slug");
    if (nameInput && slugInput) {{
        nameInput.addEventListener("input", function() {{
            slugInput.value = nameInput.value.trim().toLowerCase()
                .normalize("NFD").replace(/[\u0300-\u036f]/g, "")
                .replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
        }});
    }}

    // ── Create atelier ──────────────────────────────────────────
    var addBtn = document.getElementById("add-atelier-btn");
    if (addBtn) {{
        addBtn.addEventListener("click", async function() {{
            var name = document.getElementById("new-atelier-name").value.trim();
            var slug = document.getElementById("new-atelier-slug").value.trim();
            if (!name || !slug) {{ alert("Nom et slug requis"); return; }}
            var body = {{
                name: name,
                slug: slug,
                icon: document.getElementById("new-atelier-icon").value || "question",
                needs_validation: document.getElementById("new-atelier-validation").checked,
                default_nightly: document.getElementById("new-atelier-nightly").checked,
                opening_day_typical_needed: parseInt(
                    document.getElementById("new-atelier-needed").value || "0", 10
                )
            }};
            var res = await fetch(PREFIX + "/api/ateliers", {{
                method: "POST",
                headers: {{ "Content-Type": "application/json" }},
                body: JSON.stringify(body)
            }});
            if (res.ok) {{ location.reload(); }}
            else {{ var e = await res.json(); alert(e.error || "Erreur"); }}
        }});
    }}

    // ── Icon picker modal ───────────────────────────────────────
    var modal = document.getElementById("icon-picker-modal");
    var pickerTarget = null;  // which element triggered the picker

    function openPicker(target) {{
        pickerTarget = target;
        modal.classList.add("is-active");
        var search = document.getElementById("icon-picker-search");
        if (search) {{ search.value = ""; filterIcons(""); search.focus(); }}
    }}
    function closePicker() {{
        modal.classList.remove("is-active");
        pickerTarget = null;
    }}

    document.getElementById("icon-picker-close").addEventListener("click", closePicker);
    modal.querySelector(".modal-background").addEventListener("click", closePicker);

    // Search / filter
    var searchInput = document.getElementById("icon-picker-search");
    if (searchInput) {{
        searchInput.addEventListener("input", function() {{
            filterIcons(searchInput.value.trim().toLowerCase());
        }});
    }}
    function filterIcons(q) {{
        document.querySelectorAll(".icon-pick-btn").forEach(function(btn) {{
            var icon = btn.dataset.icon;
            var title = (btn.title || "").toLowerCase();
            btn.style.display = (!q || icon.indexOf(q) !== -1 || title.indexOf(q) !== -1) ? "" : "none";
        }});
    }}

    // Pick an icon
    document.querySelectorAll(".icon-pick-btn").forEach(function(btn) {{
        btn.addEventListener("click", function() {{
            var icon = btn.dataset.icon;
            if (!pickerTarget) return;

            if (pickerTarget.id === "new-atelier-icon-btn") {{
                // New atelier form
                document.getElementById("new-atelier-icon").value = icon;
                pickerTarget.querySelector("i").className = "fa-solid fa-" + icon;
                pickerTarget.querySelector("span:last-child").textContent = icon;
            }} else {{
                // Existing atelier row icon button
                var row = pickerTarget.closest("tr");
                row.dataset.icon = icon;
                // Update displayed icon
                row.querySelector(".atelier-icon-display i").className = "fa-solid fa-" + icon;
                pickerTarget.querySelector("i").className = "fa-solid fa-" + icon;
                saveField(row);
            }}
            closePicker();
        }});
    }});

    // Open picker from new-atelier button
    var newIconBtn = document.getElementById("new-atelier-icon-btn");
    if (newIconBtn) {{
        newIconBtn.addEventListener("click", function() {{
            openPicker(newIconBtn);
        }});
    }}

    // Open picker from existing row icon buttons
    document.querySelectorAll(".atelier-icon-change-btn").forEach(function(btn) {{
        btn.addEventListener("click", function() {{
            openPicker(btn);
        }});
    }});
}})();
"#)))
        }
    };

    page(
        "Paramètres - PowPow",
        prefix,
        &NavKind::Standard,
        "admin",
        html! {},
        content,
        script,
    )
}

fn atelier_row(_prefix: &str, a: &Atelier) -> Markup {
    html! {
        tr data-id=(a.id) data-name=(a.name) data-slug=(a.slug) data-icon=(a.icon) {
            // Icon column: display + change button
            td {
                span .atelier-icon-display {
                    span .icon { i class=(format!("fa-solid fa-{}", a.icon)) {} }
                }
                button .button.is-small.is-ghost.atelier-icon-change-btn
                    title="Changer l'icône" {
                    i .fa-solid.fa-pen.is-size-7 {}
                }
            }
            // Name: display + inline edit
            td {
                span .atelier-display { (a.name) }
                input .input.is-small.atelier-edit.d-none type="text" value=(a.name) data-field="name";
            }
            // Slug: display + inline edit
            td {
                span .atelier-display { code { (a.slug) } }
                input .input.is-small.atelier-edit.d-none type="text" value=(a.slug) data-field="slug";
            }
            // Validation: always a live checkbox
            td .has-text-centered {
                input type="checkbox" data-field="needs_validation"
                    checked[a.needs_validation];
            }
            // Nocturne: always a live checkbox
            td .has-text-centered {
                input type="checkbox" data-field="default_nightly"
                    checked[a.default_nightly];
            }
            // Besoin / jour: always a live number input
            td .has-text-centered {
                input .input.is-small type="number" min="0"
                    value=(a.opening_day_typical_needed) data-field="opening_day_typical_needed"
                    style="width:5em;text-align:center";
            }
            // Actions: edit name/slug, delete
            td {
                div .buttons.are-small {
                    button .button.is-info.is-outlined.atelier-edit-btn
                        title="Modifier nom / slug" {
                        span .icon { i .fa-solid.fa-pen {} }
                    }
                    button .button.is-success.atelier-save-btn.d-none {
                        span .icon { i .fa-solid.fa-check {} }
                    }
                    button .button.is-light.atelier-cancel-btn.d-none {
                        span .icon { i .fa-solid.fa-xmark {} }
                    }
                    button .button.is-danger.is-outlined.atelier-delete-btn {
                        span .icon { i .fa-solid.fa-trash {} }
                    }
                }
            }
        }
    }
}
