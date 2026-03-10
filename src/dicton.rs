//! "Dicton du jour" — generate a daily whimsical French paragraph by feeding
//! weather data, station state and a random staff member ("saint du jour") to
//! `DeepSeek` V3-turbo via the Hugging Face Inference API (Novita router).
//!
//! The LLM response is computed once per calendar day (Paris time) on the first
//! request, then cached in memory until the day rolls over.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::RwLock;
use tracing::{info, warn};

use crate::database;
use crate::models::{Equipment, EquipmentStatus, EquipmentType};

// ── Daily cache ──────────────────────────────────────────────────────

/// Cached LLM response keyed by the date it was generated for.
static DICTON_CACHE: RwLock<Option<(NaiveDate, String)>> = RwLock::new(None);

/// Return today's dicton, generating it via the LLM if necessary.
///
/// If `hf_token` is empty the feature is silently disabled (returns `None`).
/// Returns `None` on any generation failure so the home page simply omits it.
pub async fn get_or_generate(pool: &PgPool, season: i16, hf_token: &str) -> Option<String> {
    if hf_token.is_empty() {
        return None;
    }

    let today = paris_today();

    // Fast path: cached value for today
    if let Some(cached) = read_cache(today) {
        return Some(cached);
    }

    // Slow path: build prompt, call LLM, cache result
    match generate_dicton(pool, season, today, hf_token).await {
        Ok(text) => {
            write_cache(today, &text);
            Some(text)
        }
        Err(e) => {
            warn!("dicton du jour: generation failed: {e}");
            None
        }
    }
}

fn read_cache(today: NaiveDate) -> Option<String> {
    let guard = DICTON_CACHE.read().ok()?;
    guard
        .as_ref()
        .filter(|(d, _)| *d == today)
        .map(|(_, s)| s.clone())
}

fn write_cache(today: NaiveDate, text: &str) {
    if let Ok(mut guard) = DICTON_CACHE.write() {
        *guard = Some((today, text.to_string()));
    }
}

/// Current date in the Europe/Paris timezone.
fn paris_today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

// ── Weather fetching (Open-Meteo) ────────────────────────────────────

/// Plateau des Petites Roches, France — approximate coordinates.
const LATITUDE: f64 = 45.31;
const LONGITUDE: f64 = 5.85;

#[derive(Debug, Deserialize)]
struct WeatherResponse {
    current: Option<CurrentWeather>,
    daily: Option<DailyWeather>,
}

#[derive(Debug, Deserialize)]
struct CurrentWeather {
    temperature_2m: Option<f64>,
    relative_humidity_2m: Option<f64>,
    apparent_temperature: Option<f64>,
    precipitation: Option<f64>,
    snowfall: Option<f64>,
    cloud_cover: Option<f64>,
    wind_speed_10m: Option<f64>,
    wind_gusts_10m: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DailyWeather {
    temperature_2m_max: Option<Vec<f64>>,
    temperature_2m_min: Option<Vec<f64>>,
    precipitation_sum: Option<Vec<f64>>,
    snowfall_sum: Option<Vec<f64>>,
    wind_speed_10m_max: Option<Vec<f64>>,
    sunrise: Option<Vec<String>>,
    sunset: Option<Vec<String>>,
}

async fn fetch_weather() -> anyhow::Result<WeatherResponse> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?\
         latitude={LATITUDE}&longitude={LONGITUDE}\
         &current=temperature_2m,relative_humidity_2m,apparent_temperature,\
         precipitation,snowfall,cloud_cover,wind_speed_10m,wind_gusts_10m\
         &daily=temperature_2m_max,temperature_2m_min,precipitation_sum,\
         snowfall_sum,wind_speed_10m_max,sunrise,sunset\
         &timezone=Europe%2FParis&forecast_days=1\
         &elevation=1200"
    );

    let resp = reqwest::get(&url).await?;
    let body = resp.json::<WeatherResponse>().await?;
    Ok(body)
}

fn format_weather(w: &WeatherResponse) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(ref c) = w.current {
        if let Some(t) = c.temperature_2m {
            parts.push(format!("Température actuelle : {t:.1}°C"));
        }
        if let Some(t) = c.apparent_temperature {
            parts.push(format!("Température ressentie : {t:.1}°C"));
        }
        if let Some(h) = c.relative_humidity_2m {
            parts.push(format!("Humidité : {h:.0}%"));
        }
        if let Some(p) = c.precipitation
            && p > 0.0
        {
            parts.push(format!("Précipitations en cours : {p:.1} mm"));
        }
        if let Some(s) = c.snowfall
            && s > 0.0
        {
            parts.push(format!("Chutes de neige en cours : {s:.1} cm"));
        }
        if let Some(cc) = c.cloud_cover
            && cc >= 0.0
        {
            let label = if cc <= 20.0 {
                "Ciel dégagé"
            } else if cc <= 50.0 {
                "Partiellement nuageux"
            } else if cc <= 80.0 {
                "Nuageux"
            } else {
                "Très couvert"
            };
            parts.push(format!("Couverture nuageuse : {cc:.0}% ({label})"));
        }
        if let Some(ws) = c.wind_speed_10m {
            let gust = c
                .wind_gusts_10m
                .map_or(String::new(), |g| format!(", rafales {g:.0} km/h"));
            parts.push(format!("Vent : {ws:.0} km/h{gust}"));
        }
    }

    if let Some(ref d) = w.daily {
        if let (Some(maxs), Some(mins)) = (&d.temperature_2m_max, &d.temperature_2m_min)
            && let (Some(&mx), Some(&mn)) = (maxs.first(), mins.first())
        {
            parts.push(format!("Prévision du jour : {mn:.0}°C → {mx:.0}°C"));
        }
        if let Some(ref sums) = d.snowfall_sum
            && let Some(&s) = sums.first()
            && s > 0.0
        {
            parts.push(format!("Neige prévue : {s:.1} cm cumulés"));
        }
        if let Some(ref sums) = d.precipitation_sum
            && let Some(&p) = sums.first()
            && p > 0.0
        {
            parts.push(format!("Précipitations prévues : {p:.1} mm cumulés"));
        }
        if let Some(ref maxs) = d.wind_speed_10m_max
            && let Some(&w) = maxs.first()
            && w > 50.0
        {
            parts.push(format!("Vent max prévu : {w:.0} km/h (attention !)"));
        }
        if let (Some(sr), Some(ss)) = (&d.sunrise, &d.sunset)
            && let (Some(rise), Some(set)) = (sr.first(), ss.first())
        {
            // Extract HH:MM from ISO strings
            let rise_hm = rise.get(11..16).unwrap_or(rise);
            let set_hm = set.get(11..16).unwrap_or(set);
            parts.push(format!("Lever/coucher du soleil : {rise_hm} / {set_hm}"));
        }
    }

    if parts.is_empty() {
        "Météo indisponible.".to_string()
    } else {
        parts.join("\n")
    }
}

// ── Station state helpers ────────────────────────────────────────────

fn format_station_state(equipments: &[Equipment], station_open: bool) -> String {
    let slopes: Vec<&Equipment> = equipments
        .iter()
        .filter(|e| e.equipment_type == EquipmentType::SkiSlope)
        .collect();
    let tows: Vec<&Equipment> = equipments
        .iter()
        .filter(|e| e.equipment_type == EquipmentType::SkiTow)
        .collect();

    let open_slopes = slopes
        .iter()
        .filter(|e| e.status == EquipmentStatus::Open)
        .count();
    let partial_slopes = slopes
        .iter()
        .filter(|e| e.status == EquipmentStatus::Partial)
        .count();
    let open_tows = tows
        .iter()
        .filter(|e| e.status == EquipmentStatus::Open)
        .count();
    let partial_tows = tows
        .iter()
        .filter(|e| e.status == EquipmentStatus::Partial)
        .count();

    let status = if station_open { "OUVERTE" } else { "FERMÉE" };

    let mut lines = vec![format!("La station est {status} aujourd'hui.")];
    lines.push(format!(
        "Pistes : {open_slopes}/{} ouvertes, {partial_slopes} partielles.",
        slopes.len()
    ));
    lines.push(format!(
        "Téléskis : {open_tows}/{} ouverts, {partial_tows} partiels.",
        tows.len()
    ));

    lines.join("\n")
}

fn format_needs(upcoming: &[(NaiveDate, String, i16, i64)]) -> String {
    if upcoming.is_empty() {
        return "Aucun besoin en bénévoles cette semaine.".to_string();
    }

    let mut lines = vec!["Besoins en bénévoles cette semaine :".to_string()];
    for (day, atelier, needed, filled) in upcoming {
        let deficit = i64::from(*needed) - filled;
        let day_str = format_date_fr(*day);
        if deficit > 0 {
            lines.push(format!(
                "  - {day_str}, {atelier} : {filled}/{needed} (manque {deficit})"
            ));
        } else {
            lines.push(format!(
                "  - {day_str}, {atelier} : {filled}/{needed} (complet !)"
            ));
        }
    }
    lines.join("\n")
}

fn format_admin_state(
    unimported_memberships: i64,
    unimported_cash: i64,
    pending_validations: i64,
) -> String {
    let mut lines = Vec::new();
    if unimported_memberships > 0 {
        lines.push(format!(
            "{unimported_memberships} inscription(s) HelloAsso non importée(s)."
        ));
    }
    if unimported_cash > 0 {
        lines.push(format!(
            "{unimported_cash} paiement(s) espèces/chèque non importé(s)."
        ));
    }
    if pending_validations > 0 {
        lines.push(format!(
            "{pending_validations} validation(s) de rôle en attente."
        ));
    }
    if lines.is_empty() {
        "Pas de tâches administratives en attente.".to_string()
    } else {
        lines.join("\n")
    }
}

/// Format a `NaiveDate` as a French day name + date (e.g. "Lundi 15 janvier").
fn format_date_fr(d: NaiveDate) -> String {
    let day_name = match d.weekday() {
        chrono::Weekday::Mon => "Lundi",
        chrono::Weekday::Tue => "Mardi",
        chrono::Weekday::Wed => "Mercredi",
        chrono::Weekday::Thu => "Jeudi",
        chrono::Weekday::Fri => "Vendredi",
        chrono::Weekday::Sat => "Samedi",
        chrono::Weekday::Sun => "Dimanche",
    };
    let month_name = match d.month() {
        1 => "janvier",
        2 => "février",
        3 => "mars",
        4 => "avril",
        5 => "mai",
        6 => "juin",
        7 => "juillet",
        8 => "août",
        9 => "septembre",
        10 => "octobre",
        11 => "novembre",
        12 => "décembre",
        _ => "???",
    };
    format!("{day_name} {d_day} {month_name}", d_day = d.day())
}

// ── Saint du jour ────────────────────────────────────────────────────

/// Pick one staff member deterministically for today (using the day-of-year
/// as an index into the alphabetically-sorted list).
fn pick_saint_du_jour(
    staff: &[(crate::models::Staff, Option<i16>)],
    today: NaiveDate,
) -> Option<String> {
    if staff.is_empty() {
        return None;
    }
    let doy = today.ordinal0() as usize;
    let idx = doy % staff.len();
    let (s, _) = &staff[idx];
    Some(format!("{} {}", s.first_name, s.last_name))
}

// ── Hugging Face Inference API (chat completion) ─────────────────────

const HF_MODEL: &str = "deepseek/deepseek-v3-turbo";
const HF_API_URL: &str = "https://router.huggingface.co/novita/v3/openai/chat/completions";

#[derive(Serialize)]
struct ChatRequest {
    model: &'static str,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f64,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Option<Vec<ChatChoice>>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: Option<ChatMessageResponse>,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

/// Call the Hugging Face Inference API with the given prompt.
async fn call_llm(prompt: &str, hf_token: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();

    let body = ChatRequest {
        model: HF_MODEL,
        messages: vec![ChatMessage {
            role: "user",
            content: prompt.to_string(),
        }],
        max_tokens: 512,
        temperature: 0.9,
    };

    let resp = client
        .post(HF_API_URL)
        .header("Authorization", format!("Bearer {hf_token}"))
        .json(&body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("HuggingFace API error {status}: {text}");
    }

    let chat: ChatResponse = resp.json().await?;
    let raw = chat
        .choices
        .and_then(|mut c| c.pop())
        .and_then(|c| c.message)
        .and_then(|m| m.content)
        .unwrap_or_default();

    Ok(strip_think_tags(&raw))
}

/// Remove `<think>…</think>` reasoning blocks that DeepSeek-R1 emits.
fn strip_think_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<think>") {
        result.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find("</think>") {
            rest = &rest[start + end + "</think>".len()..];
        } else {
            // Unclosed <think> — drop everything after it
            return result.trim().to_string();
        }
    }
    result.push_str(rest);
    result.trim().to_string()
}

// ── Top-level generation ─────────────────────────────────────────────

async fn generate_dicton(
    pool: &PgPool,
    season: i16,
    today: NaiveDate,
    hf_token: &str,
) -> anyhow::Result<String> {
    let prompt = build_prompt(pool, season, today).await?;

    info!("dicton du jour: calling LLM for {today}");
    let text = call_llm(&prompt, hf_token).await?;
    info!(
        "dicton du jour: LLM response received ({} chars)",
        text.len()
    );

    if text.is_empty() {
        anyhow::bail!("LLM returned empty response");
    }

    Ok(md_to_html(&text))
}

/// Convert Markdown to sanitised HTML, reusing the same pipeline as
/// `ContentBlock::render_body`.
fn md_to_html(md: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let parser = Parser::new_ext(md, Options::all());
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    ammonia::clean(&html_output)
}

// ── Prompt builder ───────────────────────────────────────────────────

async fn build_prompt(pool: &PgPool, season: i16, today: NaiveDate) -> anyhow::Result<String> {
    // Gather all data concurrently
    let (
        weather_res,
        staff_res,
        equip_res,
        station_open_res,
        upcoming_res,
        memberships_res,
        cash_res,
        validations_res,
    ) = tokio::join!(
        fetch_weather(),
        database::get_all_staff_with_season(pool),
        database::get_all_equipments(pool),
        database::is_station_open_today(pool),
        async {
            let week_end = today + chrono::Duration::days(7);
            database::get_upcoming_needs_deficit(pool, today, week_end).await
        },
        database::count_unimported_memberships(pool, season),
        database::count_unimported_cash(pool),
        database::count_pending_validations(pool, None),
    );

    // Weather — log but don't fail if unavailable
    let weather_text = match weather_res {
        Ok(ref w) => format_weather(w),
        Err(ref e) => {
            warn!("dicton: weather fetch failed: {e}");
            "Météo indisponible (erreur de connexion).".to_string()
        }
    };

    let staff = staff_res.unwrap_or_default();
    let equipments = equip_res.unwrap_or_default();
    let station_open = station_open_res.unwrap_or(false);
    let upcoming = upcoming_res.unwrap_or_default();
    let unimported_memberships = memberships_res.unwrap_or(0);
    let unimported_cash = cash_res.unwrap_or(0);
    let pending_validations = validations_res.unwrap_or(0);

    let saint =
        pick_saint_du_jour(&staff, today).unwrap_or_else(|| "un mystérieux inconnu".to_string());

    let date_str = format_date_fr(today);

    let station_text = format_station_state(&equipments, station_open);
    let needs_text = format_needs(&upcoming);
    let admin_text =
        format_admin_state(unimported_memberships, unimported_cash, pending_validations);

    Ok(format!(
        "\
=== DICTON DU JOUR — {date_str} ===

Saint du jour : {saint}

--- Météo (Plateau des Petites Roches, ~1200m) ---
{weather_text}

--- État de la station ---
{station_text}

--- Bénévolat ---
{needs_text}

--- Administration ---
{admin_text}

---

Tu es le barde officiel de la station de ski bénévole de Saint-Hilaire du Touvet, \
perchée sur le Plateau des Petites Roches dans les Alpes françaises (alt. 1000-1300m). \
En t'inspirant des données ci-dessus, rédige un « dicton du jour » en français : \
un court paragraphe (3 à 5 phrases) mêlant humour, ironie douce, poésie montagnarde \
et esprit bénévole. \
Mentionne le (la) saint(e) du jour ({saint}) d'une façon amusante. \
Fais référence à la météo réelle et à l'état de la station. \
Si des bénévoles manquent, glisse un appel déguisé. \
Le ton doit être chaleureux, un peu décalé, et donner le sourire aux bénévoles \
qui liront ça en arrivant le matin."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_think_tags_basic() {
        let input = "<think>some reasoning</think>The actual output.";
        assert_eq!(strip_think_tags(input), "The actual output.");
    }

    #[test]
    fn test_strip_think_tags_multiple() {
        let input = "A<think>x</think>B<think>y</think>C";
        assert_eq!(strip_think_tags(input), "ABC");
    }

    #[test]
    fn test_strip_think_tags_unclosed() {
        let input = "Hello<think>dangling reasoning";
        assert_eq!(strip_think_tags(input), "Hello");
    }

    #[test]
    fn test_strip_think_tags_none() {
        let input = "Just a normal response.";
        assert_eq!(strip_think_tags(input), "Just a normal response.");
    }

    #[test]
    fn test_strip_think_tags_whitespace() {
        let input = "  <think>reasoning\nmore</think>\n\nThe result.  ";
        assert_eq!(strip_think_tags(input), "The result.");
    }
}
