//! Fields and matching helpers for explicit administrator imports.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auto_import::{Identity, Source, normalize_name, normalize_phone},
    models::Staff,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaffFields {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub comment: String,
}

impl From<&Staff> for StaffFields {
    fn from(staff: &Staff) -> Self {
        Self {
            first_name: staff.first_name.clone(),
            last_name: staff.last_name.clone(),
            email: staff.email.clone(),
            phone: staff.phone.clone().unwrap_or_default(),
            comment: staff.comment.clone(),
        }
    }
}

impl StaffFields {
    pub fn validate(&mut self) -> Result<(), String> {
        self.first_name = self.first_name.trim().to_string();
        self.last_name = self.last_name.trim().to_string();
        self.email = self.email.trim().to_lowercase();
        self.phone = self.phone.trim().to_string();
        if !self.first_name.chars().any(char::is_alphabetic)
            || !self.last_name.chars().any(char::is_alphabetic)
        {
            return Err("Le prénom et le nom sont obligatoires.".into());
        }
        if self.first_name.len() > 200
            || self.last_name.len() > 200
            || self.email.len() > 320
            || self.phone.len() > 100
            || self.comment.len() > 10_000
        {
            return Err("Un des champs est trop long.".into());
        }
        if !self.email.is_empty() && !crate::auto_import::valid_email(&self.email) {
            return Err("L'adresse email est invalide.".into());
        }
        if !self.phone.is_empty() && normalize_phone(&self.phone).is_empty() {
            return Err("Le numéro de téléphone est invalide.".into());
        }
        Ok(())
    }

    pub fn same_name(&self, other: &Self) -> bool {
        normalize_name(&self.first_name) == normalize_name(&other.first_name)
            && normalize_name(&self.last_name) == normalize_name(&other.last_name)
    }
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Create,
    Update,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    pub action: Action,
    pub staff_id: Option<Uuid>,
    pub expected_updated_at: Option<DateTime<Utc>>,
    pub fields: StaffFields,
    #[serde(default)]
    pub allow_duplicate: bool,
}

#[derive(Serialize)]
pub struct HistoryEntry {
    pub identity: Identity,
    pub date: Option<DateTime<Utc>>,
    pub season: i16,
    pub method: String,
}

#[derive(Serialize)]
pub struct Candidate {
    pub id: Uuid,
    pub fields: StaffFields,
    pub updated_at: DateTime<Utc>,
    pub last_membership_date: Option<DateTime<Utc>>,
    pub latest_season: Option<i16>,
    pub paid_for_season: bool,
    pub historical_name: Option<String>,
    pub history: Vec<HistoryEntry>,
}

pub struct Context {
    pub source: Source,
    pub season: i16,
    pub fields: StaffFields,
    pub payer_email: String,
    pub details: Vec<(&'static str, String)>,
    pub warnings: Vec<String>,
    pub imported_to: Option<Uuid>,
}

fn rough(value: &str, query: &str) -> bool {
    let value = normalize_name(value);
    let query = normalize_name(query);
    if value.is_empty() || query.is_empty() {
        return false;
    }
    if value.contains(&query) || (query.len() >= 3 && query.contains(&value)) {
        return true;
    }
    if value.len().abs_diff(query.len()) > 2 || query.chars().count() < 3 {
        return false;
    }
    let right: Vec<_> = query.chars().collect();
    let mut row: Vec<_> = (0..=right.len()).collect();
    for (i, c) in value.chars().enumerate() {
        let mut next = vec![i + 1];
        for (j, other) in right.iter().enumerate() {
            next.push(
                (next[j] + 1)
                    .min(row[j + 1] + 1)
                    .min(row[j] + usize::from(c != *other)),
            );
        }
        row = next;
    }
    row[right.len()] <= 2
}

fn name_query(first: &str, last: &str, query: &str) -> bool {
    rough(&format!("{first} {last}"), query)
        || query
            .split_whitespace()
            .all(|word| rough(first, word) || rough(last, word))
}

pub fn matches(identity: &Identity, context: &Context, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        let phone = normalize_phone(&context.fields.phone);
        return rough(&identity.first_name, &context.fields.first_name)
            || rough(&identity.last_name, &context.fields.last_name)
            || [&context.fields.email, &context.payer_email]
                .iter()
                .any(|email| {
                    !email.trim().is_empty()
                        && email.trim().eq_ignore_ascii_case(identity.email.trim())
                })
            || (!phone.is_empty() && phone == normalize_phone(&identity.phone));
    }
    if name_query(&identity.first_name, &identity.last_name, query)
        || identity
            .email
            .to_lowercase()
            .contains(&query.to_lowercase())
    {
        return true;
    }
    if query.chars().any(char::is_alphabetic) {
        return false;
    }
    let digits: String = query.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return false;
    }
    let phone = normalize_phone(&identity.phone);
    let local = phone.strip_prefix("33").map(|number| format!("0{number}"));
    phone.contains(&digits) || local.is_some_and(|local| local.contains(&digits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_matches_typos_accents_emails_and_french_phone_formats() {
        let identity = Identity {
            first_name: "Anne Céline".into(),
            last_name: "Thiery".into(),
            email: "anne@example.org".into(),
            phone: "+33 6 12 34 56 78".into(),
        };
        let context = Context {
            source: Source::HelloAsso(1),
            season: 2027,
            fields: StaffFields {
                first_name: "Anneceline".into(),
                last_name: "Thierri".into(),
                ..Default::default()
            },
            payer_email: String::new(),
            details: vec![],
            warnings: vec![],
            imported_to: None,
        };
        for query in ["", "thierri anne", "@example.org", "061234", "3361234"] {
            assert!(matches(&identity, &context, query), "query={query}");
        }
        assert!(!matches(&identity, &context, "Titouan Lakama"));
    }
}
