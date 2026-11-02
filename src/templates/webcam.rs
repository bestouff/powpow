use maud::{Markup, html};

use super::{NavKind, get_navbar_block, page};
use crate::webcam::Gallery;

pub fn webcam_page(prefix: &str, title: &str, gallery: &Gallery, follow_latest: bool) -> Markup {
    let logo = get_navbar_block().and_then(|block| block.image_id);
    let selected_name = gallery
        .selected
        .as_ref()
        .map_or("", |frame| frame.filename.as_str());
    let content = html! {
        section .section #webcam-page data-follow-latest=(follow_latest) data-image=(selected_name) {
            div .container {
                div .level {
                    div .level-left {
                        div {
                            p .heading { "Webcam" }
                            h1 .title { (title) }
                        }
                    }
                    div .level-right {
                        a .button.is-link.is-light href={(prefix) "/webcam"} {
                            span .icon { i .fa-solid.fa-clock-rotate-left {} }
                            span { "Dernière image" }
                        }
                    }
                }
                div .columns.is-variable.is-5 {
                    div .column.is-three-quarters {
                        @if let Some(frame) = &gallery.selected {
                            figure .webcam-view {
                                img .webcam-photo src={(prefix) "/webcam/images/" (frame.filename)} alt={"Vue de la webcam : " (title)};
                                @if let Some(id) = logo {
                                    img .webcam-logo src={(prefix) "/content-images/" (id)} alt="Logo de la station";
                                }
                                figcaption .webcam-caption {
                                    strong { (title) }
                                    time datetime=(frame.received_at.to_rfc3339()) {
                                        (frame.received_at.format("%d/%m/%Y à %H:%M:%S UTC"))
                                    }
                                }
                            }
                            nav .webcam-navigation.mt-4 aria-label="Parcourir les images" {
                                @if let Some(previous) = &gallery.previous {
                                    a .button href={(prefix) "/webcam?image=" (previous.filename)} {
                                        span .icon { i .fa-solid.fa-chevron-left {} }
                                        span { "Précédente" }
                                    }
                                } @else {
                                    button .button disabled { "Précédente" }
                                }
                                @if let Some(next) = &gallery.next {
                                    a .button href={(prefix) "/webcam?image=" (next.filename)} {
                                        span { "Suivante" }
                                        span .icon { i .fa-solid.fa-chevron-right {} }
                                    }
                                } @else {
                                    button .button disabled { "Suivante" }
                                }
                            }
                            p .help.mt-3 { "Heure de réception sur le serveur (UTC)." }
                        } @else {
                            div .notification.is-light {
                                "Aucune image disponible pour cette date."
                            }
                        }
                        @if follow_latest {
                            p .help { "Cette page se met à jour automatiquement à l'arrivée d'une nouvelle image." }
                        }
                    }
                    aside .column {
                        div .box.webcam-history {
                            h2 .title.is-5 { "Images précédentes" }
                            form method="get" action={(prefix) "/webcam"} {
                                div .field {
                                    label .label for="webcam-date" { "Date (UTC)" }
                                    div .control {
                                        input .input #webcam-date type="date" name="date" value=(gallery.date) required;
                                    }
                                }
                                button .button.is-link.is-fullwidth type="submit" { "Afficher" }
                            }
                            @if !gallery.images.is_empty() {
                                div .webcam-times.mt-4 aria-label="Choisir une image" {
                                    @for frame in &gallery.images {
                                        a .webcam-time .is-selected[frame.filename == selected_name]
                                          href={(prefix) "/webcam?image=" (frame.filename)}
                                          aria-current=[(frame.filename == selected_name).then_some("true")]
                                          title=(frame.username) {
                                            (frame.received_at.format("%H:%M:%S"))
                                        }
                                    }
                                }
                                @if gallery.pages > 1 {
                                    nav .webcam-pagination.mt-4 aria-label="Pages d'archives" {
                                        @if gallery.page > 0 {
                                            a .button.is-small href={(prefix) "/webcam?date=" (gallery.date) "&page=" (gallery.page - 1)} { "Plus récentes" }
                                        }
                                        span .is-size-7 { (gallery.page + 1) " / " (gallery.pages) }
                                        @if gallery.page + 1 < gallery.pages {
                                            a .button.is-small href={(prefix) "/webcam?date=" (gallery.date) "&page=" (gallery.page + 1)} { "Plus anciennes" }
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
        title,
        prefix,
        &NavKind::LoginOnly,
        "webcam",
        html! {},
        content,
        html! {},
    )
}
