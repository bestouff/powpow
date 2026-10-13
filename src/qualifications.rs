use chrono::{Datelike, Months, NaiveDate, Utc};

pub fn valid_obtained_date(date: NaiveDate) -> bool {
    date.year() >= 1900 && date <= Utc::now().date_naive()
}

pub fn expiry(date: NaiveDate, years: i16) -> Option<NaiveDate> {
    let years = u32::try_from(years)
        .ok()
        .filter(|years| (1..=100).contains(years))?;
    date.checked_add_months(Months::new(years * 12))
}

pub fn expired(date: NaiveDate, years: Option<i16>) -> bool {
    !valid_obtained_date(date)
        || years.is_some_and(|years| {
            expiry(date, years).is_none_or(|expiry| expiry < Utc::now().date_naive())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extreme_dates_and_invalid_durations_are_rejected_without_panics() {
        for date in [NaiveDate::MAX, NaiveDate::MIN] {
            assert!(!valid_obtained_date(date));
            assert!(expired(date, Some(10)));
        }
        let leap_day = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        assert_eq!(expiry(leap_day, 1), NaiveDate::from_ymd_opt(2025, 2, 28));
        for years in [i16::MIN, -1, 0, 101, i16::MAX] {
            assert!(expiry(leap_day, years).is_none());
        }
    }
}
