//! Deterministic membership matching. No database, HTTP, or email side effects.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DUPLICATE_REASON: &str = "Adhésion déjà enregistrée pour cette personne et cette saison";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Identity {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Source {
    HelloAsso(i64),
    Cash(Uuid),
}

impl Source {
    pub fn label(self) -> String {
        match self {
            Self::HelloAsso(id) => format!("helloasso:{id}"),
            Self::Cash(id) => format!("cash:{id}"),
        }
    }

    pub fn import_path(self) -> String {
        match self {
            Self::HelloAsso(id) => format!("/import/{id}"),
            Self::Cash(id) => format!("/cash-import/{id}"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Input {
    pub source: Source,
    pub identity: Identity,
    pub season: Option<i16>,
    /// Raw source amount (cents online, euros for cash); only positivity is used.
    pub amount: Option<i32>,
    pub is_membership: bool,
    pub repeated_beneficiary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Decision {
    Link { staff: Uuid, rule: &'static str },
    Create,
    Review(&'static str),
    Skip,
}

#[derive(Clone)]
struct KnownIdentity {
    staff: Uuid,
    first: String,
    last: String,
    email: String,
    phone: String,
}

#[derive(Default)]
pub struct Matcher {
    identities: Vec<KnownIdentity>,
    confirmed_staff: BTreeSet<Uuid>,
    current_names: BTreeMap<Uuid, (String, String)>,
    suspicious_staff: BTreeSet<Uuid>,
    paid_seasons: BTreeSet<(Uuid, i16)>,
}

// Ignore typography, not meaningful letters: accents, case, hyphens, spaces,
// apostrophes and decomposed combining marks do not establish a new person.
pub fn normalize_name(value: &str) -> String {
    let mut normalized = String::new();
    for c in value.to_lowercase().chars() {
        let c = match c {
            'œ' => {
                normalized.push_str("oe");
                continue;
            }
            'æ' => {
                normalized.push_str("ae");
                continue;
            }
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'ô' | 'ö' | 'ó' | 'ò' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ÿ' | 'ý' => 'y',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        };
        if c.is_alphanumeric() {
            normalized.push(c);
        }
    }
    normalized
}

fn normalize_phone(value: &str) -> String {
    let digits: String = value.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.strip_prefix("00").unwrap_or(&digits);
    if digits.len() == 10 && digits.starts_with('0') {
        format!("33{}", &digits[1..])
    } else if (9..=15).contains(&digits.len()) {
        digits.to_string()
    } else {
        String::new()
    }
}

pub fn valid_email(value: &str) -> bool {
    value.parse::<lettre::Address>().is_ok()
}

fn group_name(value: &str) -> bool {
    value.contains([',', '/', '&', ';'])
        || value
            .split_whitespace()
            .any(|word| matches!(word.to_lowercase().as_str(), "et" | "and"))
}

fn similar(left: &str, right: &str) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    if left == right {
        return true;
    }
    if left.len().min(right.len()) >= 3 && (left.starts_with(right) || right.starts_with(left)) {
        return true;
    }
    let limit = if left.chars().count().min(right.chars().count()) >= 4 {
        2
    } else {
        1
    };
    let right: Vec<_> = right.chars().collect();
    let mut row: Vec<_> = (0..=right.len()).collect();
    for (i, c) in left.chars().enumerate() {
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
    row[right.len()] <= limit
}

fn abbreviated_first_name(left: &str, right: &str) -> bool {
    if similar(left, right) {
        return true;
    }
    let (short, long) = if left.len() < right.len() {
        (left, right)
    } else {
        (right, left)
    };
    if short.len() < 3 || !short.starts_with(long.chars().next().unwrap_or_default()) {
        return false;
    }
    let mut letters = long.chars();
    short.chars().all(|c| letters.any(|next| next == c))
}

impl Matcher {
    pub fn remember(&mut self, staff: Uuid, identity: &Identity) {
        self.current_names.insert(
            staff,
            (
                normalize_name(&identity.first_name),
                normalize_name(&identity.last_name),
            ),
        );
        self.push_identity(staff, identity);
    }

    fn push_identity(&mut self, staff: Uuid, identity: &Identity) {
        self.identities.push(KnownIdentity {
            staff,
            first: normalize_name(&identity.first_name),
            last: normalize_name(&identity.last_name),
            email: identity.email.trim().to_lowercase(),
            phone: normalize_phone(&identity.phone),
        });
    }

    pub fn remember_imported(&mut self, staff: Uuid, identity: &Identity) {
        self.confirm_staff(staff);
        let historical_first = normalize_name(&identity.first_name);
        let historical_last = normalize_name(&identity.last_name);
        if let Some((first, last)) = self.current_names.get(&staff)
            && historical_first.chars().count() >= 2
            && historical_last.chars().count() >= 2
            && !abbreviated_first_name(first, &historical_first)
            && !similar(last, &historical_last)
        {
            self.suspicious_staff.insert(staff);
        }
        self.push_identity(staff, identity);
    }

    pub fn confirm_staff(&mut self, staff: Uuid) {
        self.confirmed_staff.insert(staff);
    }

    pub fn remember_payment(&mut self, staff: Uuid, season: i16) {
        self.confirm_staff(staff);
        self.paid_seasons.insert((staff, season));
    }

    fn link(&self, input: &Input, staff: Uuid, rule: &'static str) -> Decision {
        if input
            .season
            .is_some_and(|season| self.paid_seasons.contains(&(staff, season)))
        {
            return Decision::Review(DUPLICATE_REASON);
        }
        if self.suspicious_staff.contains(&staff) {
            return Decision::Review(
                "Fiche staff incompatible avec des imports précédents : vérifier l'identité",
            );
        }
        Decision::Link { staff, rule }
    }

    pub fn decide(&self, input: &Input) -> Decision {
        if !input.is_membership {
            return Decision::Skip;
        }
        if input.season.is_none() {
            return Decision::Review("Date de paiement manquante");
        }
        if input.amount.is_none_or(|amount| amount <= 0) {
            return Decision::Review(
                "Montant manquant, nul ou négatif : vérification du paiement requise",
            );
        }
        if input.repeated_beneficiary {
            return Decision::Review("Plusieurs adhésions au même nom dans la même commande");
        }
        let identity = &input.identity;
        let first = normalize_name(&identity.first_name);
        let last = normalize_name(&identity.last_name);
        if first.chars().count() < 2
            || last.chars().count() < 2
            || !first.chars().any(char::is_alphabetic)
            || !last.chars().any(char::is_alphabetic)
        {
            return Decision::Review("Nom ou prénom incomplet");
        }
        let group = group_name(&identity.first_name) || group_name(&identity.last_name);
        if group {
            return Decision::Review("Adhésion au nom de plusieurs personnes");
        }
        let email = identity.email.trim().to_lowercase();
        let phone = normalize_phone(&identity.phone);
        let contact_matches = |known: &&KnownIdentity| {
            (!email.is_empty() && email == known.email)
                || (!phone.is_empty() && phone == known.phone)
        };
        let exact: Vec<_> = self
            .identities
            .iter()
            .filter(|known| known.first == first && known.last == last)
            .collect();
        let targets: BTreeSet<_> = exact.iter().map(|known| known.staff).collect();
        if targets.len() == 1 {
            let target = *targets.first().unwrap();
            if !self.confirmed_staff.contains(&target)
                && self
                    .identities
                    .iter()
                    .any(|known| known.staff != target && known.first == first)
            {
                return Decision::Review(
                    "Fiche sans import confirmé et prénom partagé avec d'autres personnes",
                );
            }
            return self.link(input, target, "nom_complet_unique");
        }
        if targets.len() > 1 {
            let corroborated: BTreeSet<_> = exact
                .iter()
                .copied()
                .filter(contact_matches)
                .map(|known| known.staff)
                .collect();
            return if corroborated.len() == 1 {
                self.link(
                    input,
                    *corroborated.first().unwrap(),
                    "homonyme_avec_contact",
                )
            } else {
                Decision::Review("Plusieurs personnes portent ce nom, sans contact distinctif")
            };
        }
        let mut contacts: Vec<_> = self.identities.iter().filter(contact_matches).collect();
        // Prefer an exact email over a phone that may be shared by a household.
        // When the email itself is shared, fuzzy names remain ambiguous.
        if !email.is_empty() && contacts.iter().any(|known| known.email == email) {
            contacts.retain(|known| known.email == email);
        }
        let corroborated: BTreeSet<_> = contacts
            .iter()
            .filter(|known| {
                known.first == first
                    || (known.last == last && abbreviated_first_name(&known.first, &first))
                    || (known.first == last && known.last == first)
            })
            .map(|known| known.staff)
            .collect();
        if corroborated.len() == 1 {
            let contact_owners: BTreeSet<_> = contacts.iter().map(|known| known.staff).collect();
            if contact_owners.len() > 1 {
                return Decision::Review(
                    "Coordonnées partagées : une ressemblance de nom ne suffit pas",
                );
            }
            return self.link(input, *corroborated.first().unwrap(), "nom_et_contact");
        }
        if corroborated.len() > 1 {
            return Decision::Review("Nom et coordonnées correspondent à plusieurs personnes");
        }
        if self.identities.iter().any(|known| {
            (known.first == first && similar(&known.last, &last))
                || (known.last == last && similar(&known.first, &first))
        }) {
            return Decision::Review(
                "Nom proche d'une personne existante, sans coordonnées concordantes",
            );
        }
        // A shared family email does not make two different first names the same
        // person. Allow creating a clearly distinct family member.
        if !contacts.is_empty()
            && !contacts
                .iter()
                .all(|known| known.last == last && !similar(&known.first, &first))
        {
            return Decision::Review(
                "Coordonnées d'une autre personne, sans lien de nom suffisamment clair",
            );
        }
        if contacts.is_empty() && self.identities.iter().any(|known| known.first == first) {
            return Decision::Review(
                "Prénom connu mais nom et coordonnées différents : nouvelle personne ou changement de nom",
            );
        }
        if !valid_email(&email) {
            return Decision::Review(
                "Adresse email manquante ou invalide pour une nouvelle personne",
            );
        }
        Decision::Create
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(first: &str, last: &str, email: &str) -> Input {
        Input {
            source: Source::HelloAsso(1),
            identity: Identity {
                first_name: first.into(),
                last_name: last.into(),
                email: email.into(),
                phone: String::new(),
            },
            season: Some(2027),
            amount: Some(2000),
            is_membership: true,
            repeated_beneficiary: false,
        }
    }

    #[test]
    fn typography_and_contact_changes_do_not_duplicate_people() {
        let staff = Uuid::new_v4();
        let mut matcher = Matcher::default();
        matcher.remember(
            staff,
            &input("Éléonore", "Le-Corre", "old@example.org").identity,
        );
        assert!(
            matches!(matcher.decide(&input(" eleonore ", "LE CORRE", "new@example.org")), Decision::Link { staff: id, .. } if id == staff)
        );
        assert_eq!(normalize_name("E\u{301}léonore"), "eleonore");
    }

    #[test]
    fn shared_family_email_never_merges_different_people() {
        let mut matcher = Matcher::default();
        matcher.remember(
            Uuid::new_v4(),
            &input("Alice", "Martin", "family@example.org").identity,
        );
        assert_eq!(
            matcher.decide(&input("Bob", "Martin", "family@example.org")),
            Decision::Create
        );
        assert!(matches!(
            matcher.decide(&input("Claude", "Dupont", "family@example.org")),
            Decision::Review(_)
        ));
    }

    #[test]
    fn repeated_order_names_and_groups_require_review() {
        let mut matcher = Matcher::default();
        let mut record = input("Alice", "Martin", "alice@example.org");
        matcher.remember(Uuid::new_v4(), &record.identity);
        record.repeated_beneficiary = true;
        assert!(matches!(matcher.decide(&record), Decision::Review(_)));
        assert!(matches!(
            matcher.decide(&input("Alice et Bob", "Martin", "family@example.org")),
            Decision::Review(_)
        ));
    }

    #[test]
    fn homonyms_need_a_distinctive_contact() {
        let mut matcher = Matcher::default();
        let alice = Uuid::new_v4();
        matcher.remember(
            alice,
            &input("Alice", "Martin", "first@example.org").identity,
        );
        matcher.remember(
            Uuid::new_v4(),
            &input("Alice", "Martin", "second@example.org").identity,
        );
        assert!(
            matches!(matcher.decide(&input("Alice", "Martin", "first@example.org")), Decision::Link { staff, .. } if staff == alice)
        );
        assert!(matches!(
            matcher.decide(&input("Alice", "Martin", "family@example.org")),
            Decision::Review(_)
        ));
    }

    #[test]
    fn typos_need_corroboration() {
        let mut matcher = Matcher::default();
        let staff = Uuid::new_v4();
        matcher.remember(
            staff,
            &input("Christine", "Gachet", "chris@example.org").identity,
        );
        assert!(
            matches!(matcher.decide(&input("Chris", "Gachet", "chris@example.org")), Decision::Link { staff: id, .. } if id == staff)
        );
        assert!(matches!(
            matcher.decide(&input("Chris", "Gachet", "unknown@example.org")),
            Decision::Review(_)
        ));
    }

    #[test]
    fn donations_and_incomplete_records_do_not_grant_membership() {
        let matcher = Matcher::default();
        let mut record = input("Alice", "Martin", "alice@example.org");
        record.is_membership = false;
        assert_eq!(matcher.decide(&record), Decision::Skip);
        record.is_membership = true;
        record.season = None;
        assert!(matches!(matcher.decide(&record), Decision::Review(_)));
        assert!(matches!(
            matcher.decide(&input("Alice", "Martin", "")),
            Decision::Review(_)
        ));
    }

    #[test]
    fn confirmed_aliases_reuse_the_same_person() {
        let mut matcher = Matcher::default();
        let staff = Uuid::new_v4();
        matcher.remember(
            staff,
            &input("Christine", "Gachet", "new@example.org").identity,
        );
        matcher.remember_imported(staff, &input("Chris", "Gachet", "old@example.org").identity);
        assert!(
            matches!(matcher.decide(&input("Chris", "Gachet", "old@example.org")), Decision::Link { staff: id, .. } if id == staff)
        );
    }

    #[test]
    fn a_placeholder_cannot_override_an_ambiguous_existing_person() {
        let mut matcher = Matcher::default();
        matcher.remember_imported(
            Uuid::new_v4(),
            &input("Marie", "Tourlonnias", "new@example.org").identity,
        );
        matcher.remember(
            Uuid::new_v4(),
            &input("Marie", "Bernard", "old@example.org").identity,
        );
        assert!(matches!(
            matcher.decide(&input("Marie", "Bernard", "old@example.org")),
            Decision::Review(_)
        ));
    }

    #[test]
    fn phone_formatting_can_corroborate_a_name_correction() {
        let mut matcher = Matcher::default();
        let staff = Uuid::new_v4();
        let mut original = input("Alice", "Martin", "old@example.org");
        original.identity.phone = "06 12 34 56 78".into();
        matcher.remember_imported(staff, &original.identity);
        let mut renewal = input("Alice", "Martine", "new@example.org");
        renewal.identity.phone = "+33 6 12 34 56 78".into();
        assert!(
            matches!(matcher.decide(&renewal), Decision::Link { staff: id, .. } if id == staff)
        );
    }

    #[test]
    fn fuzzy_names_cannot_disambiguate_a_shared_family_contact() {
        let mut matcher = Matcher::default();
        matcher.remember_imported(
            Uuid::new_v4(),
            &input("Alice", "Martin", "family@example.org").identity,
        );
        matcher.remember_imported(
            Uuid::new_v4(),
            &input("Bob", "Martin", "family@example.org").identity,
        );
        assert!(matches!(
            matcher.decide(&input("Alicia", "Martin", "family@example.org")),
            Decision::Review(_)
        ));
    }

    #[test]
    fn a_second_membership_in_the_same_season_is_never_imported_automatically() {
        let mut matcher = Matcher::default();
        let staff = Uuid::new_v4();
        let mut renewal = input("Alice", "Martin", "alice@example.org");
        matcher.remember(staff, &renewal.identity);
        matcher.remember_payment(staff, 2027);
        assert_eq!(matcher.decide(&renewal), Decision::Review(DUPLICATE_REASON));
        renewal.season = Some(2028);
        assert!(
            matches!(matcher.decide(&renewal), Decision::Link { staff: id, .. } if id == staff)
        );
    }

    #[test]
    fn unrelated_historical_and_current_names_require_review() {
        let mut matcher = Matcher::default();
        let child = Uuid::new_v4();
        matcher.remember(
            child,
            &input("Anne Celine", "Thiery", "shared@example.org").identity,
        );
        matcher.remember_imported(
            child,
            &input("Titouan", "Lakama", "shared@example.org").identity,
        );
        assert!(matches!(
            matcher.decide(&input("Anne celine", "Thiery", "shared@example.org")),
            Decision::Review(_)
        ));
        assert!(matches!(
            matcher.decide(&input("Titouan", "Lakama", "shared@example.org")),
            Decision::Review(_)
        ));
    }

    #[test]
    fn nonpositive_or_missing_payments_require_review() {
        let matcher = Matcher::default();
        let mut record = input("Alice", "Martin", "alice@example.org");
        for amount in [None, Some(0), Some(-20)] {
            record.amount = amount;
            assert!(matches!(matcher.decide(&record), Decision::Review(_)));
        }
        assert!(matches!(
            matcher.decide(&input("1234", "Martin", "alice@example.org")),
            Decision::Review(_)
        ));
    }
}
