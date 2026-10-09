//! Shared-address recipients for notifications and newsletters, never login links.

use std::collections::BTreeMap;

use crate::auto_import::capitalize_words;

#[derive(Debug, PartialEq, Eq)]
pub struct EmailRecipient {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
}

impl EmailRecipient {
    pub fn name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
            .trim()
            .to_string()
    }
}

/// Send once per normalized address. Group first names under each shared surname.
/// Keep login delivery separate: its name and token must belong to one person.
pub fn group_recipients<'a>(
    people: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>,
) -> Vec<EmailRecipient> {
    let mut addresses: BTreeMap<String, Vec<(String, Vec<String>)>> = BTreeMap::new();
    for (email, first_name, last_name) in people {
        let email = email.trim().to_lowercase();
        if email.is_empty() {
            continue;
        }
        let families = addresses.entry(email).or_default();
        let first_name = capitalize_words(first_name);
        let last_name = capitalize_words(last_name);
        if first_name.is_empty() && last_name.is_empty() {
            continue;
        }
        if let Some((_, first_names)) = families.iter_mut().find(|(name, _)| *name == last_name) {
            if !first_names.contains(&first_name) {
                first_names.push(first_name);
            }
        } else {
            families.push((last_name, vec![first_name]));
        }
    }
    addresses
        .into_iter()
        .map(|(email, mut families)| {
            let (first_name, last_name) = if families.len() == 1 {
                let (last_name, first_names) = families.pop().unwrap();
                (first_names.join(" et "), last_name)
            } else {
                let name = families
                    .into_iter()
                    .map(|(last_name, first_names)| {
                        format!("{} {}", first_names.join(" et "), last_name)
                            .trim()
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(", et ");
                // Different surnames cannot be represented by one LNAME. Put
                // the full household name in FNAME and clear the old LNAME.
                (name, String::new())
            };
            EmailRecipient {
                email,
                first_name,
                last_name,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_email_combines_families_without_overwriting_names() {
        let recipients = group_recipients([
            (" Family@Example.org ", "Firstname1", "Lastname1"),
            ("family@example.org", "Firstname2", "Lastname1"),
            ("FAMILY@example.org", "Firstname3", "Lastname2"),
            ("family@example.org", "Firstname1", "Lastname1"),
        ]);
        assert_eq!(recipients.len(), 1);
        assert_eq!(recipients[0].email, "family@example.org");
        assert_eq!(
            recipients[0].name(),
            "Firstname1 et Firstname2 Lastname1, et Firstname3 Lastname2"
        );
        assert_eq!(recipients[0].last_name, "");
    }

    #[test]
    fn same_surname_preserves_merge_fields_and_unicode_canonicalization() {
        let recipients = group_recipients([
            ("family@example.org", "ANNE", "LE\u{301}VY"),
            ("family@example.org", "PAUL", "LÉVY"),
            ("other@example.org", "Alice", "Martin"),
            ("", "Ignored", "Person"),
        ]);
        assert_eq!(recipients.len(), 2);
        assert_eq!(recipients[0].first_name, "Anne et Paul");
        assert_eq!(recipients[0].last_name, "Lévy");
        assert_eq!(recipients[1].name(), "Alice Martin");
    }

    #[test]
    fn unknown_addresses_still_receive_one_notification() {
        let recipients = group_recipients([
            ("unknown@example.org", "", ""),
            ("UNKNOWN@example.org", "", ""),
        ]);
        assert_eq!(recipients.len(), 1);
        assert_eq!(recipients[0].name(), "");
    }
}
