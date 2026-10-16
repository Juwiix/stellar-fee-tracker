//! TTL-based in-memory cache for fee_stats-style lookups.
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct TtlCache<V: Clone> {
    ttl: Duration,
    entries: HashMap<String, (V, Instant)>,
}

impl<V: Clone> TtlCache<V> {
    pub fn new(ttl: Duration) -> Self {
        Self { ttl, entries: HashMap::new() }
    }

    /// Returns the value for `key` if it is present and has not expired.
    ///
    /// This does not modify the cache; expired entries are removed by
    /// [`TtlCache::put`] and [`TtlCache::purge_expired`].
    pub fn get(&self, key: &str) -> Option<V> {
        self.entries
            .get(key)
            .filter(|(_, at)| at.elapsed() < self.ttl)
            .map(|(v, _)| v.clone())
    }

    /// Stores `value` under `key`, first evicting every expired entry so keys
    /// that are never read again do not accumulate.
    pub fn put(&mut self, key: String, value: V) {
        self.purge_expired();
        self.entries.insert(key, (value, Instant::now()));
    }

    /// Removes all expired entries and returns how many were removed.
    pub fn purge_expired(&mut self) -> usize {
        let ttl = self.ttl;
        let before = self.entries.len();
        self.entries.retain(|_, (_, at)| at.elapsed() < ttl);
        before - self.entries.len()
    }

    /// Number of entries currently stored, including any that have expired but
    /// have not been purged yet.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    const TTL: Duration = Duration::from_millis(20);
    const PAST_TTL: Duration = Duration::from_millis(60);

    #[test]
    fn get_returns_fresh_value_and_hides_expired_one() {
        let mut cache = TtlCache::new(TTL);
        cache.put("a".to_string(), 1);
        assert_eq!(cache.get("a"), Some(1));

        sleep(PAST_TTL);
        assert_eq!(cache.get("a"), None);
    }

    #[test]
    fn put_evicts_expired_entries_that_are_never_read() {
        let mut cache = TtlCache::new(TTL);
        cache.put("stale-1".to_string(), 1);
        cache.put("stale-2".to_string(), 2);
        assert_eq!(cache.len(), 2);

        sleep(PAST_TTL);
        cache.put("fresh".to_string(), 3);

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("fresh"), Some(3));
    }

    #[test]
    fn put_keeps_entries_that_have_not_expired() {
        let mut cache = TtlCache::new(Duration::from_secs(60));
        cache.put("a".to_string(), 1);
        cache.put("b".to_string(), 2);

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get("a"), Some(1));
        assert_eq!(cache.get("b"), Some(2));
    }

    #[test]
    fn purge_expired_removes_only_expired_entries_and_reports_the_count() {
        let mut cache = TtlCache::new(TTL);
        cache.put("old".to_string(), 1);
        sleep(PAST_TTL);
        cache.entries.insert("new".to_string(), (2, Instant::now()));

        assert_eq!(cache.purge_expired(), 1);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("new"), Some(2));
        assert_eq!(cache.purge_expired(), 0);
    }

    #[test]
    fn overwriting_a_key_replaces_its_value_and_timestamp() {
        let mut cache = TtlCache::new(Duration::from_secs(60));
        cache.put("k".to_string(), 1);
        cache.put("k".to_string(), 2);

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("k"), Some(2));
    }
}
