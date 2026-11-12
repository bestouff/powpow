use std::io::Cursor;

use chrono::{DateTime, Local, NaiveDate};
use genpdf::{Alignment, Element as _, elements, fonts, style};
use image::{ImageFormat, Rgba, RgbaImage};

use crate::models::{AtelierDayPlanning, OpeningDayStatus};

const NAVY: style::Color = style::Color::Rgb(45, 57, 92);
const ORANGE: style::Color = style::Color::Rgb(193, 92, 4);

pub fn render(
    day: NaiveDate,
    planning: &[AtelierDayPlanning],
    status: Option<OpeningDayStatus>,
    entity: &str,
    logo: Option<&[u8]>,
    exported_at: DateTime<Local>,
) -> anyhow::Result<Vec<u8>> {
    let regular = fonts::FontData::new(
        include_bytes!("../static/fonts/LiberationSans-Regular.ttf").to_vec(),
        None,
    )?;
    let bold = fonts::FontData::new(
        include_bytes!("../static/fonts/LiberationSans-Bold.ttf").to_vec(),
        None,
    )?;
    let mut document = genpdf::Document::new(fonts::FontFamily {
        italic: regular.clone(),
        bold_italic: bold.clone(),
        regular,
        bold,
    });
    let title = format!("Planning du {}", day.format("%d/%m/%Y"));
    document.set_title(&title);
    document.set_font_size(11);
    document.set_line_spacing(1.2);
    document.set_minimal_conformance();

    let logo = logo.and_then(|data| match logo_image(data) {
        Ok(image) => Some(image),
        Err(error) => {
            tracing::warn!(%error, "Cannot decode planning logo");
            None
        }
    });
    let entity = entity.to_string();
    let stamp = format!(
        "Exporté le {} (UTC{})",
        exported_at.format("%d/%m/%Y à %H:%M:%S"),
        exported_at.format("%:z"),
    );
    let mut decorator = genpdf::SimplePageDecorator::new();
    decorator.set_margins(15);
    decorator.set_header(move |page| {
        let heading = elements::LinearLayout::vertical()
            .element(
                elements::Paragraph::new(&entity).styled(
                    style::Style::new()
                        .bold()
                        .with_font_size(13)
                        .with_color(NAVY),
                ),
            )
            .element(
                elements::Paragraph::new(&title).styled(
                    style::Style::new()
                        .bold()
                        .with_font_size(22)
                        .with_color(NAVY),
                ),
            );
        let mut brand = elements::TableLayout::new(vec![1, 4]);
        let image: Box<dyn genpdf::Element> = match &logo {
            Some(image) => Box::new(image.clone()),
            None => Box::new(elements::Paragraph::default()),
        };
        brand
            .push_row(vec![image, Box::new(heading)])
            .expect("two header columns");
        elements::LinearLayout::vertical()
            .element(brand)
            .element(elements::Break::new(0.5))
            .element(
                elements::Paragraph::new(format!("{stamp} · Page {page}"))
                    .aligned(Alignment::Right)
                    .styled(
                        style::Style::new()
                            .with_font_size(9)
                            .with_color(style::Color::Rgb(90, 90, 90)),
                    ),
            )
            .element(elements::Break::new(1))
    });
    document.set_page_decorator(decorator);

    if let Some(status) = status {
        let label = match status {
            OpeningDayStatus::Reserved => "Ouverture prévue, à confirmer.",
            OpeningDayStatus::Validated => "Ouverture confirmée.",
            OpeningDayStatus::Canceled => "Journée d’ouverture annulée.",
        };
        document.push(
            elements::Paragraph::new(label).styled(style::Style::new().bold().with_color(ORANGE)),
        );
        document.push(elements::Break::new(0.7));
    }
    for atelier in planning {
        document.push(
            elements::Paragraph::new(&atelier.atelier_name).styled(
                style::Style::new()
                    .bold()
                    .with_font_size(14)
                    .with_color(NAVY),
            ),
        );
        document.push(elements::Break::new(0.3));
        let mut shifts = elements::TableLayout::new(vec![1, 1]);
        shifts.set_cell_decorator(elements::FrameCellDecorator::new(true, true, false));
        shifts
            .row()
            .element(shift(atelier, true).padded(3))
            .element(shift(atelier, false).padded(3))
            .push()?;
        document.push(shifts);
        document.push(elements::Break::new(1));
    }
    let mut bytes = Vec::new();
    document.render(&mut bytes)?;
    Ok(bytes)
}

fn shift(atelier: &AtelierDayPlanning, first_half: bool) -> elements::LinearLayout {
    let label = match (atelier.nightly, first_half) {
        (false, true) => "Matin",
        (false, false) => "Après-midi",
        (true, true) => "Soirée",
        (true, false) => "Nuit",
    };
    let mut layout = elements::LinearLayout::vertical();
    layout.push(
        elements::Paragraph::new(label).styled(style::Style::new().bold().with_color(ORANGE)),
    );
    layout.push(elements::Break::new(0.4));
    let mut count = 0;
    for member in atelier.staff.iter().filter(|member| {
        if first_half {
            member.first_half
        } else {
            member.second_half
        }
    }) {
        layout.push(elements::Paragraph::new(format!(
            "{} {}",
            member.first_name, member.last_name
        )));
        count += 1;
    }
    if count == 0 {
        layout.push(
            elements::Paragraph::new("Aucun bénévole inscrit.")
                .styled(style::Color::Rgb(100, 100, 100)),
        );
    }
    layout
}

fn logo_image(data: &[u8]) -> anyhow::Result<elements::Image> {
    let image = image::load_from_memory(data)?
        .thumbnail(360, 240)
        .to_rgba8();
    // Transparent logos need a white background for printing.
    let mut background =
        RgbaImage::from_pixel(image.width(), image.height(), Rgba([255, 255, 255, 255]));
    image::imageops::overlay(&mut background, &image, 0, 0);
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(background)
        .to_rgb8()
        .write_to(&mut png, ImageFormat::Png)?;
    png.set_position(0);
    Ok(elements::Image::from_reader(png)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PlannedStaff;

    #[test]
    fn pdf_preserves_accents_shifts_logo_and_export_time_across_pages() -> anyhow::Result<()> {
        let day = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let exported_at =
            DateTime::parse_from_rfc3339("2026-11-12T18:27:43+01:00")?.with_timezone(&Local);
        let mut atelier = AtelierDayPlanning {
            atelier_id: uuid::Uuid::new_v4(),
            atelier_name: "Accueil & café".into(),
            atelier_slug: "accueil".into(),
            atelier_icon: "users".into(),
            nightly: false,
            staff: (0..90)
                .map(|index| PlannedStaff {
                    id: uuid::Uuid::new_v4(),
                    first_name: format!("Élodie-{index:03}"),
                    last_name: "O’Neill".into(),
                    first_half: true,
                    second_half: false,
                })
                .collect(),
        };
        let mut logo = Cursor::new(Vec::new());
        RgbaImage::from_pixel(20, 20, Rgba([45, 57, 92, 128]))
            .write_to(&mut logo, ImageFormat::Png)?;
        let bytes = render(
            day,
            &[atelier],
            Some(OpeningDayStatus::Canceled),
            "Station Étoile",
            Some(logo.get_ref()),
            exported_at,
        )?;
        let pdf = lopdf::Document::load_mem(&bytes)?;
        let pages: Vec<_> = pdf.get_pages().keys().copied().collect();
        assert!(pages.len() > 1);
        let text = pdf
            .extract_text(&pages)?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        for expected in [
            "Accueil & café",
            "Matin",
            "Après-midi",
            "Aucun bénévole inscrit.",
            "Élodie-000",
            "Élodie-089",
            "O’Neill",
            "Journée d’ouverture annulée.",
        ] {
            assert!(
                text.contains(expected),
                "Missing {expected:?} in PDF: {text}"
            );
        }
        for page in pages {
            let text = pdf
                .extract_text(&[page])?
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            assert!(text.contains("Planning du 31/12/2026"));
            assert!(text.contains("Station Étoile"));
            assert!(text.contains(&format!(
                "Exporté le {}",
                exported_at.format("%d/%m/%Y à %H:%M:%S")
            )));
            assert!(text.contains(&format!("Page {page}")));
        }
        assert!(pdf.objects.values().any(|object| {
            object
                .as_stream()
                .ok()
                .and_then(|stream| stream.dict.get(b"Subtype").ok())
                .and_then(|value| value.as_name().ok())
                == Some(b"Image")
        }));

        atelier = AtelierDayPlanning {
            atelier_id: uuid::Uuid::new_v4(),
            atelier_name: "Nocturne".into(),
            atelier_slug: "nocturne".into(),
            atelier_icon: "moon".into(),
            nightly: true,
            staff: vec![PlannedStaff {
                id: uuid::Uuid::new_v4(),
                first_name: "Zoé".into(),
                last_name: "Martin".into(),
                first_half: false,
                second_half: true,
            }],
        };
        let bytes = render(day, &[atelier], None, "Station", None, exported_at)?;
        let text = lopdf::Document::load_mem(&bytes)?
            .extract_text(&[1])?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("Soirée"));
        assert!(text.contains("Nuit"));
        assert_eq!(text.matches("Zoé Martin").count(), 1);
        Ok(())
    }
}
