use super::{NavKind, page};
use crate::models::Staff;
use maud::{Markup, PreEscaped, html};

/// Mailchimp member statuses we display on the newsletter page, in order.
/// `label` is the French label, `class` the Bulma tag class.
const STATUS_META: &[(&str, &str, &str)] = &[
    ("subscribed", "Inscrit", "is-success"),
    ("unsubscribed", "Désinscrit", "is-danger"),
    ("non-subscribed", "Non inscrit", "is-light"),
    ("cleaned", "Email invalide (cleaned)", "is-warning"),
    ("pending", "En attente de confirmation", "is-info"),
];

/// Page showing each staff member's newsletter status, with a button to run a
/// full Mailchimp sync. When `configured` is false, a warning banner is shown
/// and the sync button is disabled since it cannot work.
pub fn mailchimp_page(staff: &[Staff], prefix: &str, configured: bool) -> Markup {
    let p = prefix;

    let count_subscribed = staff
        .iter()
        .filter(|s| s.newsletter_status == "subscribed")
        .count();

    let content = html! {
        section .section {
            div .container.is-fluid {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href=(format!("{p}/")) { "Accueil" } }
                        li { a href=(format!("{p}/admin")) { "Administration" } }
                        li .is-active { a href="#" aria-current="page" { "Newsletter (Mailchimp)" } }
                    }
                }

                div .level.mb-4 {
                    div .level-left {
                        h1 .title.is-3 {
                            span .icon.mr-2 { i .fa-solid.fa-envelope-open-text {} }
                            "Newsletter (Mailchimp)"
                        }
                    }
                    div .level-right {
                        form #mailchimp-sync-form method="POST" action={(p) "/mailchimp"} {
                            button #mailchimp-sync-btn .button.is-primary type="submit" disabled=(!configured) {
                                span .icon { i .fa-solid.fa-arrows-rotate {} }
                                span { @if configured { "Synchroniser maintenant" } @else { "Synchronisation indisponible" } }
                            }
                        }
                    }
                }

                @if !configured {
                    div .notification.is-warning {
                        p {
                            strong { "Mailchimp n'est pas configuré." }
                            " La synchronisation de la newsletter est désactivée. Renseignez les variables "
                            code .has-text-danger { "MAILCHIMP_API_KEY" } ", " code .has-text-danger { "MAILCHIMP_SERVER_PREFIX" } ", "
                            code .has-text-danger { "MAILCHIMP_LIST_ID" } " et " code .has-text-danger { "MAILCHIMP_FROM_EMAIL" }
                            " puis redémarrez le serveur. Les statuts affichés ci-dessous datent de la dernière synchronisation."
                        }
                    }
                }

                div .notification.is-success.is-light {
                    p {
                        (count_subscribed) " personne(s) recevront la newsletter sur " (staff.len()) " membre(s) du staff."
                    }
                }

                div .box {
                    div style="max-height: 60vh; overflow-y: auto;" {
                        table .table.is-fullwidth.is-hoverable {
                            thead {
                                tr {
                                    th { "Nom" }
                                    th { "Email" }
                                    th { "Statut Mailchimp" }
                                }
                            }
                            tbody {
                                @for s in staff {
                                    @let (label, tag_class) = STATUS_META
                                        .iter()
                                        .find(|(status, _, _)| *status == s.newsletter_status)
                                        .map_or(("Autre", "is-light"), |(_, label, class)| (*label, *class));
                                    tr {
                                        td { strong { (s.last_name) } " " (s.first_name) }
                                        td { span .is-size-7 { (s.email) } }
                                        td {
                                            span class={"tag " (tag_class)} {
                                                (label) @if label == "Autre" { " (" (s.newsletter_status) ")" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div .notification.is-info.is-light.mt-4 {
                    p {
                        strong { "Comment ça marche ?" }
                        " À chaque import (HelloAsso, espèces/chèque) et suppression, powpow synchronise le statut de chaque membre depuis Mailchimp "
                        "et met à jour la liste des destinataires. Le bouton "
                        button .button.is-small.is-primary.is-outlined type="button" { "Synchroniser maintenant" }
                        " force cette synchronisation à l'instant."
                    }
                }

                div #mailchimp-sync-overlay .is-hidden style="position: fixed; inset: 0; background: rgba(255,255,255,0.92); z-index: 1000; display: flex; align-items: center; justify-content: center;" {
                    div .has-text-centered {
                        div .loader.is-large {}
                        p .has-text-weight-bold.mt-4 { "Synchronisation Mailchimp en cours…" }
                        p #sync-count-received .mt-3 {}
                        p #sync-count-pushed .mt-1 {}
                        p .mt-3.is-size-7.has-text-grey { "Cela peut prendre quelques minutes. Ne fermez pas cette page." }
                    }
                }
            }
        }
    };

    let script = html! {
        script {
            (PreEscaped(r#"
(function() {
    var form = document.getElementById("mailchimp-sync-form");
    if (!form) return;
    var PREFIX = document.body.dataset.prefix || "";
    var overlay = document.getElementById("mailchimp-sync-overlay");
    var rec = document.getElementById("sync-count-received");
    var pus = document.getElementById("sync-count-pushed");
    var btn = document.getElementById("mailchimp-sync-btn");
    var polling = true;

    function fmt(n) { return n.toLocaleString("fr-FR"); }

    function render(p) {
        if (p.phase === "pulling") {
            rec.textContent = "Récupération des membres depuis Mailchimp : " + fmt(p.pulled) + (p.pulled_total ? " / " + fmt(p.pulled_total) : "");
            pus.textContent = "";
        } else if (p.phase === "storing") {
            rec.textContent = "Récupérés : " + fmt(p.pulled) + " membres";
            pus.textContent = "Enregistrement des statuts en base : " + fmt(p.stored) + " / " + fmt(p.pulled);
        } else if (p.phase === "pushing") {
            rec.textContent = "Récupérés : " + fmt(p.pulled) + " membres, " + fmt(p.stored) + " statut(s) enregistré(s)";
            pus.textContent = "Envoi des destinataires : " + fmt(p.pushed) + " / " + fmt(p.pushed_total);
        } else if (p.phase === "done") {
            rec.textContent = "Récupérés : " + fmt(p.pulled) + " membres, " + fmt(p.stored) + " statut(s) enregistré(s)";
            pus.textContent = "Destinataires envoyés : " + fmt(p.push_ok) + " / " + fmt(p.pushed_total)
                + (p.push_errors ? " (" + fmt(p.push_errors) + " erreur(s))" : "");
        } else if (p.phase === "error") {
            rec.textContent = "Erreur de synchronisation";
            pus.textContent = p.error || "";
        }
    }

    function pollProgress() {
        fetch(PREFIX + "/mailchimp/progress")
            .then(function(r) { if (!r.ok) throw new Error("http " + r.status); return r.json(); })
            .then(function(p) {
                if (!polling) return;
                render(p);
                setTimeout(pollProgress, 1000);
            })
            .catch(function() {
                if (polling) setTimeout(pollProgress, 1000);
            });
    }

    form.addEventListener("submit", function(e) {
        e.preventDefault();
        overlay.classList.remove("is-hidden");
        btn.classList.add("is-loading");
        btn.disabled = true;
        rec.textContent = "Préparation de la synchronisation…";
        pus.textContent = "";
        polling = true;
        pollProgress();

        fetch(PREFIX + "/mailchimp", { method: "POST" })
            .then(function() {
                // sync as finished: grab the final state then fade out
                return fetch(PREFIX + "/mailchimp/progress")
                    .then(function(r) { return r.json(); })
                    .then(function(p) {
                        render(p);
                        polling = false;
                        if (p.phase === "error") return;
                        setTimeout(function() { window.location.reload(); }, 1500);
                    });
            })
            .catch(function(err) {
                polling = false;
                rec.textContent = "Erreur réseau pendant la synchronisation";
                pus.textContent = err && err.message ? err.message : "";
            });
    });
})();
"#))
        }
    };

    page(
        "Newsletter (Mailchimp) - PowPow",
        prefix,
        &NavKind::Standard,
        "",
        html! {},
        content,
        script,
    )
}

/// Result page shown after a manual Mailchimp sync.
pub fn mailchimp_sync_result(prefix: &str, success: bool, message: &str) -> Markup {
    let p = prefix;
    let (icon_class, title, notification_class) = if success {
        ("has-text-success", "Synchronisation réussie", "is-success")
    } else {
        ("has-text-danger", "Erreur de synchronisation", "is-danger")
    };

    let icon = if success {
        "circle-check"
    } else {
        "triangle-exclamation"
    };

    let content = html! {
        section .section {
            div .container.is-fluid {
                div .columns.is-centered {
                    div .column.is-8 {
                        div .box.has-text-centered {
                            span class={"icon is-large " (icon_class) " mb-4"} {
                                i class={"fa-solid fa-" (icon) " fa-4x"} {}
                            }
                            h1 .title.is-3 { (title) }
                            div class={"notification " (notification_class) " is-light"} {
                                p { (message) }
                            }
                            div .buttons.is-centered.mt-5 {
                                a .button.is-primary.is-medium href={(p) "/mailchimp"} {
                                    span .icon { i .fa-solid.fa-envelope-open-text {} }
                                    span { "Retour à la newsletter" }
                                }
                                a .button.is-info.is-medium href={(p) "/"} {
                                    span .icon { i .fa-solid.fa-house {} }
                                    span { "Retour à l'accueil" }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    page(
        "Synchronisation Mailchimp - PowPow",
        prefix,
        &NavKind::Standard,
        "",
        html! {},
        content,
        html! {},
    )
}
