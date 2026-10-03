//! Read-only comparison of matching decisions with recorded admin imports.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{
    auto_import::{Decision, Matcher, Source},
    database::auto_import::{load_catalog, matcher_for},
};

#[derive(Deserialize)]
struct Snapshot {
    staff: Vec<(uuid::Uuid, crate::auto_import::Identity)>,
    records: Vec<crate::database::auto_import::Record>,
}

pub fn run_from_stdin() -> anyhow::Result<()> {
    let snapshot: Snapshot = serde_json::from_reader(std::io::stdin().lock())?;
    replay(&snapshot.staff, &snapshot.records)
}

#[derive(Default, Serialize)]
struct Counts {
    total: usize,
    automatic_correct: usize,
    manual_review: usize,
    mismatches: usize,
}

#[derive(Default, Serialize)]
struct Replay {
    totals: Counts,
    by_source_and_season: BTreeMap<String, Counts>,
    rules: BTreeMap<String, usize>,
    mismatched_sources: Vec<(String, Decision)>,
    reviewed_sources: Vec<(String, String)>,
}

impl Replay {
    fn record(&mut self, source: Source, season: Option<i16>, decision: &Decision, correct: bool) {
        let key = format!(
            "{}:{}",
            if matches!(source, Source::HelloAsso(_)) {
                "helloasso"
            } else {
                "cash"
            },
            season.unwrap_or_default()
        );
        let counts = self.by_source_and_season.entry(key).or_default();
        counts.total += 1;
        self.totals.total += 1;
        match decision {
            Decision::Review(reason) => {
                counts.manual_review += 1;
                self.totals.manual_review += 1;
                self.reviewed_sources
                    .push((source.label(), (*reason).to_string()));
                *self.rules.entry((*reason).to_string()).or_default() += 1;
            }
            _ if correct => {
                counts.automatic_correct += 1;
                self.totals.automatic_correct += 1;
                let rule = match decision {
                    Decision::Link { rule, .. } => *rule,
                    _ => "nouvelle_personne",
                };
                *self.rules.entry(rule.to_string()).or_default() += 1;
            }
            _ => {
                counts.mismatches += 1;
                self.totals.mismatches += 1;
                self.mismatched_sources
                    .push((source.label(), decision.clone()));
            }
        }
    }
}

pub async fn run(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await?;
    let (staff, records) = load_catalog(&mut tx).await?;
    tx.rollback().await?;
    replay(&staff, &records)
}

fn replay(
    staff: &[(uuid::Uuid, crate::auto_import::Identity)],
    records: &[crate::database::auto_import::Record],
) -> anyhow::Result<()> {
    let mut catalog_replay = Replay::default();
    let mut chronological_replay = Replay::default();
    let mut past = Matcher::default();
    let mut seen = BTreeSet::new();
    for record in records {
        let Some(expected) = record.target else {
            continue;
        };
        // Exclude the payment being tested from learned aliases. The profile
        // catalog (including prior payment status) is current, so this measures
        // renewals, not first-time creation or a blind historical reconstruction.
        let decision = matcher_for(staff, records, Some(record.input.source)).decide(&record.input);
        let correct = matches!(decision, Decision::Link { staff, .. } if staff == expected);
        catalog_replay.record(record.input.source, record.input.season, &decision, correct);

        // A stricter replay: only original source fields from preceding imports
        // are known. No current staff names/emails or future payment links.
        let decision = past.decide(&record.input);
        let correct = match decision {
            Decision::Link { staff, .. } => staff == expected,
            Decision::Create => !seen.contains(&expected),
            _ => false,
        };
        chronological_replay.record(record.input.source, record.input.season, &decision, correct);
        if !record.input.repeated_beneficiary {
            past.remember_imported(expected, &record.input.identity);
        }
        seen.insert(expected);
        if let Some(season) = record.paid_season {
            past.remember_payment(expected, season);
        }
    }
    let matcher = matcher_for(staff, records, None);
    let pending: Vec<_> = records
        .iter()
        .filter(|record| record.target.is_none())
        .map(|record| {
            (
                record.input.source.label(),
                if record.eligible {
                    matcher.decide(&record.input)
                } else {
                    Decision::Skip
                },
            )
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "catalog_leave_one_payment_out": catalog_replay,
        "chronological_original_fields": chronological_replay,
        "pending_decisions": pending,
        "limitations": "Purchase-date order is used; historical pre-edit staff profiles were not saved. Catalog replay uses current profiles and paid-status metadata, withholding only the tested payment's alias. Chronological replay cannot know admin corrections absent from original source fields. Both runs are read-only, with no emails or imports.",
        }))?
    );
    Ok(())
}
