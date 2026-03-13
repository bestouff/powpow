use super::{NavKind, page};
use crate::models::ContentBlock;
use maud::{Markup, html};

/// Human-readable French label for a content slug.
fn slug_label(slug: &str) -> &'static str {
    match slug {
        "hero-subtitle" => "Sous-titre du héro",
        "infos-station" => "Infos station (bas de colonne)",
        "about-station" => "À propos — La station",
        "about-association" => "À propos — L'association (encadré doré)",
        "events" => "Événements",
        "salle-hors-sac" => "Salle hors-sac",
        "newsletter" => "Newsletter et adhésion",
        "footer-contact" => "Pied de page — Contact",
        "footer-calendar" => "Pied de page — Calendrier",
        "footer-summer" => "Pied de page — En été",
        "driving-indications" => "Accès à la station",
        "pricing" => "Tarifs",
        "favicon" => "Favicon du site",
        "trail-map" => "Plan des pistes",
        "navbar" => "Barre de navigation",
        _ => "Bloc inconnu",
    }
}

/// Admin page listing all content blocks.
pub fn content_list_page(prefix: &str, blocks: &[ContentBlock]) -> Markup {
    let p = prefix;

    let content = html! {
        section .section {
            div .container.is-fluid {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href=(format!("{p}/")) { "Accueil" } }
                        li { a href=(format!("{p}/admin")) { "Administration" } }
                        li .is-active { a href="#" aria-current="page" { "Contenu éditorial" } }
                    }
                }

                h1 .title.is-3 {
                    span .icon.mr-2 { i .fa-solid.fa-pen-fancy {} }
                    "Contenu éditorial"
                }

                p .mb-4 {
                    "Modifiez le contenu textuel de la page d'accueil. Le corps est en "
                    strong { "Markdown" }
                    "."
                }

                div .table-container {
                    table .table.is-striped.is-hoverable.is-fullwidth {
                        thead {
                            tr {
                                th { "Section" }
                                th { "Slug" }
                                th { "Titre" }
                                th { "Dernière modification" }
                                th {}
                            }
                        }
                        tbody {
                            @for block in blocks {
                                @let ts = block.updated_at
                                    .with_timezone(&chrono::Local)
                                    .format("%d/%m/%Y %H:%M")
                                    .to_string();
                                tr {
                                    td { (slug_label(&block.slug)) }
                                    td .is-family-monospace.is-size-7 { (block.slug) }
                                    td {
                                        @if block.title.is_empty() {
                                            em .has-text-grey-light { "(vide)" }
                                        } @else {
                                            (block.title)
                                        }
                                    }
                                    td .is-size-7 { (ts) }
                                    td {
                                        a .button.is-small.is-info.is-outlined
                                          href=(format!("{p}/admin/contents/{}", block.slug)) {
                                            span .icon { i .fa-solid.fa-pen {} }
                                            span { "Modifier" }
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
        "Contenu éditorial - AGHIL",
        prefix,
        &NavKind::Standard,
        "admin",
        html! {},
        content,
        html! {},
    )
}

/// Admin edit form for a single content block.
pub fn content_edit_page(
    prefix: &str,
    block: &ContentBlock,
    image_filename: Option<&str>,
) -> Markup {
    let p = prefix;
    let label = slug_label(&block.slug);

    let content = html! {
        section .section {
            div .container.is-fluid {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href=(format!("{p}/")) { "Accueil" } }
                        li { a href=(format!("{p}/admin")) { "Administration" } }
                        li { a href=(format!("{p}/admin/contents")) { "Contenu éditorial" } }
                        li .is-active { a href="#" aria-current="page" { (label) } }
                    }
                }

                h1 .title.is-3 {
                    span .icon.mr-2 { i .fa-solid.fa-pen-fancy {} }
                    "Modifier : " (label)
                }

                div .columns {
                    div .column.is-8 {
                        form method="POST" enctype="multipart/form-data"
                             action=(format!("{p}/admin/contents/{}", block.slug)) {
                            // Title
                            div .field {
                                label .label { "Titre" }
                                div .control {
                                    input .input type="text" name="title"
                                          value=(block.title)
                                          placeholder="Titre (laisser vide si non applicable)";
                                }
                            }

                            // Body (markdown)
                            div .field {
                                label .label { "Contenu (Markdown)" }
                                div .control {
                                    textarea .textarea name="body" rows="12"
                                             placeholder="Contenu en Markdown..." {
                                        (block.body)
                                    }
                                }
                            }

                            // Image
                            div .field {
                                label .label { "Image (optionnelle, max 5 Mo)" }
                                @if let Some(img_id) = block.image_id {
                                    div .mb-3 {
                                        p .mb-2 {
                                            strong { "Image actuelle : " }
                                            @if let Some(fname) = image_filename {
                                                (fname)
                                            }
                                        }
                                        img src=(format!("{p}/content-images/{img_id}"))
                                            alt="Image actuelle"
                                            style="max-width:300px;max-height:200px;border-radius:4px;";
                                        div .mt-2 {
                                            label .checkbox {
                                                input type="checkbox" name="remove_image" value="1";
                                                " Supprimer l'image"
                                            }
                                        }
                                    }
                                }
                                div .control {
                                    div .file.has-name {
                                        label .file-label {
                                            input .file-input type="file" name="image"
                                                  accept="image/*"
                                                  onchange="updateFileName(this)";
                                            span .file-cta {
                                                span .file-icon {
                                                    i .fa-solid.fa-upload {}
                                                }
                                                span .file-label { "Choisir un fichier..." }
                                            }
                                            span .file-name #file-name {
                                                "Aucun fichier sélectionné"
                                            }
                                        }
                                    }
                                }
                            }

                            // Link URL
                            div .field {
                                label .label { "URL du lien (optionnel)" }
                                div .control {
                                    input .input type="url" name="link_url"
                                          value=(block.link_url.as_deref().unwrap_or(""))
                                          placeholder="https://...";
                                }
                            }

                            // Link label
                            div .field {
                                label .label { "Libellé du lien (optionnel)" }
                                div .control {
                                    input .input type="text" name="link_label"
                                          value=(block.link_label.as_deref().unwrap_or(""))
                                          placeholder="Texte affiché sur le bouton";
                                }
                            }

                            // Submit
                            div .field.is-grouped {
                                div .control {
                                    button .button.is-primary type="submit" {
                                        span .icon { i .fa-solid.fa-floppy-disk {} }
                                        span { "Enregistrer" }
                                    }
                                }
                                div .control {
                                    a .button.is-light href=(format!("{p}/admin/contents")) {
                                        "Annuler"
                                    }
                                }
                            }
                        }
                    }

                    // Markdown cheat sheet
                    div .column.is-4 {
                        div .box {
                            h3 .title.is-5 {
                                span .icon.mr-1 { i .fa-solid.fa-circle-info {} }
                                "Aide Markdown"
                            }
                            div .content.is-small {
                                table .table.is-narrow.is-fullwidth {
                                    tbody {
                                        tr {
                                            td .is-family-monospace { "**gras**" }
                                            td { strong { "gras" } }
                                        }
                                        tr {
                                            td .is-family-monospace { "*italique*" }
                                            td { em { "italique" } }
                                        }
                                        tr {
                                            td .is-family-monospace { "[texte](url)" }
                                            td { "lien cliquable" }
                                        }
                                        tr {
                                            td .is-family-monospace { "# Titre" }
                                            td { "titre niveau 1" }
                                        }
                                        tr {
                                            td .is-family-monospace { "## Sous-titre" }
                                            td { "titre niveau 2" }
                                        }
                                        tr {
                                            td .is-family-monospace { "- élément" }
                                            td { "liste à puces" }
                                        }
                                        tr {
                                            td .is-family-monospace { "1. élément" }
                                            td { "liste numérotée" }
                                        }
                                        tr {
                                            td .is-family-monospace { "ligne vide" }
                                            td { "nouveau paragraphe" }
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
        &format!("Modifier «{}» - AGHIL", label),
        prefix,
        &NavKind::Standard,
        "admin",
        html! {},
        content,
        html! {},
    )
}
