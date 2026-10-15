//! The deliberately small iCalendar subset used by personal planning feeds.

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use uuid::Uuid;

pub const FEED_PREFIX: &str = "/calendar/feed/";
pub const FEED_ROUTE: &str = "/calendar/feed/{token}";
const TOKEN_BYTES: usize = 32;
const TOKEN_LENGTH: usize = 43;

pub fn new_token() -> Result<String> {
    let mut bytes = [0; TOKEN_BYTES];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Cannot generate calendar token"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub fn valid_token(token: &str) -> bool {
    token.len() == TOKEN_LENGTH
        && URL_SAFE_NO_PAD
            .decode(token)
            .is_ok_and(|bytes| bytes.len() == TOKEN_BYTES)
}

#[derive(serde::Serialize)]
pub struct SubscriptionLinks {
    pub url: String,
    pub webcal_url: String,
}

pub fn links(base: &reqwest::Url, token: &str) -> Result<SubscriptionLinks> {
    let url = base
        .join(&format!(
            "{}{token}.ics",
            FEED_PREFIX.trim_start_matches('/')
        ))?
        .to_string();
    let (_, destination) = url.split_once(':').context("Missing calendar URL scheme")?;
    let webcal_url = format!("webcal:{destination}");
    Ok(SubscriptionLinks { url, webcal_url })
}

#[derive(sqlx::FromRow)]
pub struct CalendarEntry {
    pub need_id: Uuid,
    pub day: NaiveDate,
    pub atelier_name: String,
    pub nightly: bool,
    pub first_half: bool,
    pub second_half: bool,
    pub modified_at: DateTime<Utc>,
}

impl CalendarEntry {
    fn period(&self) -> &'static str {
        match (self.nightly, self.first_half, self.second_half) {
            (false, true, true) => "matin et après-midi",
            (false, true, false) => "matin",
            (false, false, _) => "après-midi",
            (true, true, true) => "soirée et nuit",
            (true, true, false) => "soirée",
            (true, false, _) => "nuit",
        }
    }
}

pub fn render(
    staff: &crate::models::Staff,
    entries: &[CalendarEntry],
    base: &reqwest::Url,
    entity: &str,
) -> Result<String> {
    let mut ics = String::new();
    line(&mut ics, "BEGIN:VCALENDAR");
    line(&mut ics, "VERSION:2.0");
    line(&mut ics, "PRODID:-//PowPow//Staff planning//FR");
    line(&mut ics, "CALSCALE:GREGORIAN");
    text(
        &mut ics,
        "X-WR-CALNAME",
        &format!("{entity} — {} {}", staff.first_name, staff.last_name),
    );
    let profile = base.join(&format!("person/{}", staff.id))?;
    for entry in entries
        .iter()
        .filter(|entry| entry.first_half || entry.second_half)
    {
        let end = entry
            .day
            .succ_opt()
            .context("Calendar date has no following day")?;
        ensure!(
            (1..=9999).contains(&entry.day.year()) && end.year() <= 9999,
            "Unsupported calendar date"
        );
        line(&mut ics, "BEGIN:VEVENT");
        line(
            &mut ics,
            &format!("UID:{}-{}@powpow", staff.id, entry.need_id),
        );
        let modified = entry.modified_at.format("%Y%m%dT%H%M%SZ");
        line(&mut ics, &format!("DTSTAMP:{modified}"));
        line(&mut ics, &format!("LAST-MODIFIED:{modified}"));
        line(
            &mut ics,
            &format!("DTSTART;VALUE=DATE:{}", entry.day.format("%Y%m%d")),
        );
        line(
            &mut ics,
            &format!("DTEND;VALUE=DATE:{}", end.format("%Y%m%d")),
        );
        text(
            &mut ics,
            "SUMMARY",
            &format!("{} — {}", entry.atelier_name, entry.period()),
        );
        text(
            &mut ics,
            "DESCRIPTION",
            "Présence inscrite dans PowPow. Modifiez vos disponibilités depuis votre fiche.",
        );
        text(&mut ics, "LOCATION", entity);
        line(&mut ics, &format!("URL:{profile}"));
        line(&mut ics, "STATUS:CONFIRMED");
        line(&mut ics, "END:VEVENT");
    }
    line(&mut ics, "END:VCALENDAR");
    Ok(ics)
}

/// RFC 5545 TEXT escaping, distinct from HTML and URI escaping.
fn text(output: &mut String, property: &str, value: &str) {
    let escaped = value
        .replace('\\', "\\\\")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\\n")
        .replace(';', "\\;")
        .replace(',', "\\,");
    line(output, &format!("{property}:{escaped}"));
}

/// Fold at 75 octets without splitting UTF-8, counting the continuation space.
fn line(output: &mut String, value: &str) {
    let mut width = 0;
    for ch in value.chars() {
        if width + ch.len_utf8() > 75 {
            output.push_str("\r\n ");
            width = 1;
        }
        output.push(ch);
        width += ch.len_utf8();
    }
    output.push_str("\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_cannot_inject_properties_and_utf8_lines_are_folded() {
        let mut output = String::new();
        let value = format!("{}\\,;\r\nBEGIN:VEVENT\nSUMMARY:injected", "é🎿".repeat(30));
        text(&mut output, "SUMMARY", &value);
        assert!(output.split("\r\n").all(|line| line.len() <= 75));
        let unfolded = output.replace("\r\n ", "");
        assert!(!unfolded.contains("\r\nBEGIN:VEVENT"));
        assert!(unfolded.contains("\\\\\\,\\;\\nBEGIN:VEVENT\\nSUMMARY:injected"));
        assert!(unfolded.contains(&"é🎿".repeat(30)));
    }

    #[test]
    fn tokens_and_links_are_separate_from_login_and_preserve_the_public_prefix() {
        let token = new_token().unwrap();
        assert!(valid_token(&token));
        assert!(!valid_token(&Uuid::new_v4().to_string()));
        assert!(!valid_token(&"x".repeat(500)));
        let links = links(&"https://station.example/powpow/".parse().unwrap(), &token).unwrap();
        assert_eq!(
            links.url,
            format!("https://station.example/powpow/calendar/feed/{token}.ics")
        );
        assert_eq!(
            links.webcal_url,
            format!("webcal://station.example/powpow/calendar/feed/{token}.ics")
        );
    }
}
