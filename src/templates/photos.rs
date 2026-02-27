use super::{page, NavKind};
use crate::models::PhotoMeta;
use maud::{html, Markup};

pub fn photo_page(prefix: &str, photos: &[(PhotoMeta, String)], is_admin: bool) -> Markup {
    let content = html! {
        section .section {
            div .container.is-fluid {
                nav .breadcrumb aria-label="breadcrumbs" {
                    ul {
                        li { a href=(format!("{prefix}/")) { "Accueil" } }
                        li .is-active { a href="#" aria-current="page" { "Photos" } }
                    }
                }

                h1 .title.is-4 {
                    span .icon.mr-2 { i .fa-solid.fa-images {} }
                    "Gestion des photos"
                }

                @if is_admin {
                    div .box {
                        form #photo-upload-form action=(format!("{prefix}/photos/upload")) method="post" enctype="multipart/form-data" {
                            input type="hidden" name="photographer_id" #photographer_id;
                            div .field {
                                label .label { "Photographe" }
                                div .control.has-icons-left {
                                    input .input type="text" #photographer-search
                                        placeholder="Rechercher un bénévole (4 car. min)"
                                        autocomplete="off";
                                    span .icon.is-left { i .fa-solid.fa-user {} }
                                }
                                nav .panel.search-dropdown #photographer-results {}
                                p .help.d-none #photographer-selected {
                                    span .tag.is-success.is-medium #photographer-selected-tag {}
                                    a #photographer-clear .ml-2.is-clickable { "Changer" }
                                }
                            }
                            div #create-staff-box .d-none.notification.is-light.mt-2.mb-4 {
                                p .mb-2 { strong { "Créer un nouveau bénévole" } }
                                div .field.is-horizontal {
                                    div .field-body {
                                        div .field {
                                            div .control {
                                                input .input type="text" #new-staff-first placeholder="Prénom";
                                            }
                                        }
                                        div .field {
                                            div .control {
                                                input .input type="text" #new-staff-last placeholder="Nom";
                                            }
                                        }
                                    }
                                }
                                div .field.is-horizontal.mt-2 {
                                    div .field-body {
                                        div .field {
                                            div .control.has-icons-left {
                                                input .input type="email" #new-staff-email placeholder="Email";
                                                span .icon.is-left { i .fa-solid.fa-envelope {} }
                                            }
                                        }
                                        div .field {
                                            div .control.has-icons-left {
                                                input .input type="tel" #new-staff-phone placeholder="Téléphone";
                                                span .icon.is-left { i .fa-solid.fa-phone {} }
                                            }
                                        }
                                        div .field {
                                            div .control {
                                                button type="button" .button.is-info #create-staff-btn { "Créer" }
                                            }
                                        }
                                    }
                                }
                                p .help.is-danger.d-none #create-staff-error {}
                            }
                            div .field {
                                label .label { "Photo" }
                                div .control {
                                    div .file.has-name.is-primary {
                                        label .file-label {
                                            input .file-input type="file" name="photo" accept="image/*" required;
                                            span .file-cta {
                                                span .file-icon { i .fa-solid.fa-upload {} }
                                                span .file-label { "Choisir un fichier..." }
                                            }
                                            span .file-name { "Aucun fichier sélectionné" }
                                        }
                                    }
                                }
                            }
                            div .field {
                                div .control {
                                    button type="submit" .button.is-primary #upload-btn disabled {
                                        span .icon { i .fa-solid.fa-cloud-arrow-up {} }
                                        span { "Télécharger" }
                                    }
                                }
                            }
                        }
                    }
                }

                h2 .title.is-5.mt-6 { "Photos disponibles" }
                div .columns.is-multiline {
                    @if photos.is_empty() {
                        div .column {
                            div .notification.is-info { "Aucune photo disponible" }
                        }
                    }
                    @for (photo, photographer_name) in photos {
                        @let photo_url = format!("{}/photos/{}", prefix, photo.id);
                        @let icon = if photo.mime_type.starts_with("image/") {
                            "fa-image"
                        } else if photo.mime_type.starts_with("video/") {
                            "fa-video"
                        } else {
                            "fa-file"
                        };
                        div .column.is-one-quarter {
                            div .card {
                                div .card-image {
                                    figure .image.is-4by3 {
                                        a href=(photo_url) target="_blank" {
                                            @if photo.mime_type.starts_with("image/") {
                                                img .image-cover src=(photo_url) alt=(format!("Photo par {photographer_name}"));
                                            } @else {
                                                span .icon.is-large.has-text-link {
                                                    i class={"fa-solid " (icon) " fa-4x"} {}
                                                }
                                            }
                                        }
                                    }
                                }
                                div .card-content {
                                    div .media {
                                        div .media-content {
                                            p .title.is-6 { (photographer_name) }
                                        }
                                    }
                                }
                                @if is_admin {
                                    footer .card-footer {
                                        form .is-fullwidth action=(format!("{}/photos/{}/delete", prefix, photo.id))
                                            method="post"
                                            onsubmit="return confirm('Supprimer cette photo ?')" {
                                            button type="submit" .card-footer-item.has-text-danger.button-unstyled {
                                                span .icon { i .fa-solid.fa-trash {} }
                                                span { "Supprimer" }
                                            }
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

    let script = html! {};

    page(
        "Photos - AGHIL",
        prefix,
        &NavKind::StaffOnly,
        "",
        html! {},
        content,
        script,
    )
}
