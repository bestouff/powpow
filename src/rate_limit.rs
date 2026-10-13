use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct MailLimits(Mutex<HashMap<String, (Instant, u32)>>);

impl MailLimits {
    pub fn allow(&self, key: &str, limit: u32, window: Duration) -> bool {
        self.allow_at(key, limit, window, Instant::now())
    }

    fn allow_at(&self, key: &str, limit: u32, window: Duration, now: Instant) -> bool {
        let Ok(mut buckets) = self.0.lock() else {
            return false;
        };
        buckets.retain(|_, (expires, _)| *expires > now);
        if buckets.len() >= 4096 && !buckets.contains_key(key) {
            return false;
        }
        let (_, count) = buckets.entry(key.to_string()).or_insert((now + window, 0));
        if *count >= limit {
            return false;
        }
        *count += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_independent_and_expire() {
        let limiter = MailLimits::default();
        let now = Instant::now();
        let window = Duration::from_secs(60);
        assert!(limiter.allow_at("login:a", 1, window, now));
        assert!(!limiter.allow_at("login:a", 1, window, now));
        assert!(limiter.allow_at("login:b", 1, window, now));
        assert!(limiter.allow_at("login:a", 1, window, now + window));
    }
}
