//! A map that keeps only the most recent entries.
//!
//! Scan sessions and disk analyses are large: an analysis of a home directory holds one node
//! per directory, which was tens of thousands on the machine this was measured on. They were
//! kept for the life of the process, so a session spent scanning a few times grew steadily.
//! The UI only ever refers to the scan it is showing, so old ones can go.

use std::collections::HashMap;

/// Keyed storage that forgets the oldest entry once it is full.
pub struct Recent<T> {
    capacity: usize,
    /// Insertion order, oldest first.
    order: Vec<String>,
    items: HashMap<String, T>,
}

impl<T> Recent<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "a capacity of zero would drop every entry");
        Self {
            capacity,
            order: Vec::new(),
            items: HashMap::new(),
        }
    }

    /// Adds an entry, evicting the oldest if that takes it over capacity.
    pub fn insert(&mut self, key: String, value: T) {
        if self.items.insert(key.clone(), value).is_some() {
            // Replacing an existing entry: it keeps its original position, because it is the
            // same scan being filled in, not a new one.
            return;
        }
        self.order.push(key);
        while self.order.len() > self.capacity {
            let oldest = self.order.remove(0);
            self.items.remove(&oldest);
        }
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        self.items.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut T> {
        self.items.get_mut(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<T> {
        self.order.retain(|k| k != key);
        self.items.remove(key)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[cfg(test)]
    pub fn keys_oldest_first(&self) -> &[String] {
        &self.order
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Recent<u32> {
        let mut recent = Recent::new(3);
        for i in 1..=3 {
            recent.insert(format!("s{i}"), i);
        }
        recent
    }

    #[test]
    fn keeps_what_fits() {
        let recent = store();
        assert_eq!(recent.len(), 3);
        assert_eq!(recent.get("s1"), Some(&1));
        assert_eq!(recent.get("s3"), Some(&3));
    }

    #[test]
    fn forgets_the_oldest_when_full() {
        let mut recent = store();
        recent.insert("s4".into(), 4);

        assert_eq!(recent.len(), 3);
        assert_eq!(
            recent.get("s1"),
            None,
            "the oldest should have been dropped"
        );
        assert_eq!(recent.get("s4"), Some(&4));
        assert_eq!(recent.keys_oldest_first(), ["s2", "s3", "s4"]);
    }

    #[test]
    fn replacing_an_entry_does_not_make_it_newer() {
        // A scan is inserted when it starts and replaced when it finishes. That must not push
        // the ones queued behind it out.
        let mut recent = store();
        recent.insert("s1".into(), 100);

        assert_eq!(recent.len(), 3);
        assert_eq!(recent.get("s1"), Some(&100));
        assert_eq!(recent.keys_oldest_first(), ["s1", "s2", "s3"]);

        recent.insert("s4".into(), 4);
        assert_eq!(recent.get("s1"), None, "s1 was still the oldest");
        assert_eq!(recent.get("s2"), Some(&2));
    }

    #[test]
    fn removing_frees_a_slot() {
        let mut recent = store();
        assert_eq!(recent.remove("s2"), Some(2));
        assert_eq!(recent.len(), 2);

        recent.insert("s4".into(), 4);
        assert_eq!(recent.len(), 3);
        // Nothing was evicted, because there was room.
        assert_eq!(recent.get("s1"), Some(&1));
        assert_eq!(recent.remove("s2"), None);
    }

    #[test]
    fn mutating_in_place_works() {
        let mut recent = store();
        *recent.get_mut("s2").unwrap() = 22;
        assert_eq!(recent.get("s2"), Some(&22));
        assert!(recent.get_mut("missing").is_none());
    }
}
