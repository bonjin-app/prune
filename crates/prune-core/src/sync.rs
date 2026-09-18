//! Locks that survive a panic.
//!
//! Rust poisons a `Mutex` when a thread panics while holding it, and every later `lock().unwrap()`
//! panics in turn. For a long-running application that means one bug in one scan leaves every
//! later action broken until the user quits and reopens — a far worse outcome than the original
//! panic.
//!
//! Recovering is only safe when the protected data has no invariant a half-finished write could
//! break. Everything Prune guards this way is a cache of results: scan sessions, analyses, the
//! list of applications. Nothing about removal depends on them, because
//! [`SafetyPolicy`](crate::safety::SafetyPolicy) re-validates every path at the moment of
//! deletion rather than trusting anything cached. So the right response to poisoning is to take
//! the data as it stands and carry on.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// `lock`, `read` and `write` that step over a poisoned lock instead of panicking.
pub trait LockExt<T> {
    /// The guard, whether or not a previous holder panicked.
    fn lock_recover(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("recovering a lock poisoned by an earlier panic");
            poisoned.into_inner()
        })
    }
}

/// The same for a reader-writer lock.
pub trait RwLockExt<T> {
    fn read_recover(&self) -> RwLockReadGuard<'_, T>;
    fn write_recover(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwLockExt<T> for RwLock<T> {
    fn read_recover(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|poisoned| {
            tracing::warn!("recovering a lock poisoned by an earlier panic");
            poisoned.into_inner()
        })
    }

    fn write_recover(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|poisoned| {
            tracing::warn!("recovering a lock poisoned by an earlier panic");
            poisoned.into_inner()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_panic_while_holding_a_mutex_does_not_break_every_later_use() {
        let value = Arc::new(Mutex::new(vec![1, 2, 3]));

        let poisoner = Arc::clone(&value);
        let panicked = std::thread::spawn(move || {
            let mut guard = poisoner.lock().unwrap();
            guard.push(4);
            panic!("something went wrong mid-write");
        })
        .join();
        assert!(panicked.is_err(), "the thread should have panicked");
        assert!(value.lock().is_err(), "which poisons the lock");

        // The application carries on with the data as it stands.
        let guard = value.lock_recover();
        assert_eq!(*guard, vec![1, 2, 3, 4]);
    }

    #[test]
    fn a_healthy_mutex_behaves_normally() {
        let value = Mutex::new(7);
        assert_eq!(*value.lock_recover(), 7);
        *value.lock_recover() = 9;
        assert_eq!(*value.lock_recover(), 9);
    }

    #[test]
    fn a_panic_does_not_break_a_reader_writer_lock_either() {
        let value = Arc::new(RwLock::new(String::from("before")));

        let poisoner = Arc::clone(&value);
        let _ = std::thread::spawn(move || {
            let mut guard = poisoner.write().unwrap();
            guard.push_str(" and after");
            panic!("interrupted");
        })
        .join();
        assert!(value.read().is_err());

        assert_eq!(&*value.read_recover(), "before and after");
        value.write_recover().push('!');
        assert_eq!(&*value.read_recover(), "before and after!");
    }
}
