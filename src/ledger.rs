//! Source of truth for usage events.
//!
//! Every accepted event gets a global, increasing sequence number. The
//! projection job reads the ledger in sequence order and records how far it
//! got, so the ledger is what a replay starts from.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// An event as the ledger stored it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredEvent {
    pub seq: u64,
    pub project: String,
    pub event_id: String,
    pub operation: String,
    /// Unix time in seconds when the API accepted the event.
    pub received_at: u64,
}

/// What happened to an append.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Append {
    /// First time this id was seen in this project.
    Accepted(StoredEvent),
    /// Same id and same content: a retry. Nothing changes.
    Duplicate,
    /// Same id with different content: a client bug. The first record wins.
    Conflict,
}

/// Storage for the event ledger. The in-memory implementation is the
/// reference; durable implementations must pass the same tests.
pub trait EventLedger: Send + Sync {
    /// Checks the id and inserts in one atomic step.
    fn append(&self, project: &str, event_id: &str, operation: &str, received_at: u64) -> Append;
    /// Events with `seq > after`, in sequence order, at most `limit`.
    fn read_after(&self, after: u64, limit: usize) -> Vec<StoredEvent>;
    /// Number of distinct events recorded for a project.
    fn count(&self, project: &str) -> u64;
    /// Highest sequence number assigned so far (0 when empty).
    fn head(&self) -> u64;
}

#[derive(Default)]
struct Inner {
    events: Vec<StoredEvent>,
    by_id: HashMap<(String, String), usize>,
    per_project: HashMap<String, u64>,
}

/// Ledger kept in memory behind one lock. Not durable: a restart starts empty.
#[derive(Clone, Default)]
pub struct MemoryLedger(Arc<Mutex<Inner>>);

impl EventLedger for MemoryLedger {
    fn append(&self, project: &str, event_id: &str, operation: &str, received_at: u64) -> Append {
        let mut inner = self.0.lock().expect("ledger lock");
        let key = (project.to_owned(), event_id.to_owned());
        if let Some(&index) = inner.by_id.get(&key) {
            return if inner.events[index].operation == operation {
                Append::Duplicate
            } else {
                Append::Conflict
            };
        }
        let event = StoredEvent {
            seq: inner.events.len() as u64 + 1,
            project: key.0.clone(),
            event_id: key.1.clone(),
            operation: operation.to_owned(),
            received_at,
        };
        let index = inner.events.len();
        inner.events.push(event.clone());
        inner.by_id.insert(key, index);
        *inner.per_project.entry(project.to_owned()).or_default() += 1;
        Append::Accepted(event)
    }

    fn read_after(&self, after: u64, limit: usize) -> Vec<StoredEvent> {
        let inner = self.0.lock().expect("ledger lock");
        // Sequence numbers start at 1 and match the position in the vector.
        let start = usize::try_from(after)
            .unwrap_or(usize::MAX)
            .min(inner.events.len());
        inner.events[start..].iter().take(limit).cloned().collect()
    }

    fn count(&self, project: &str) -> u64 {
        let inner = self.0.lock().expect("ledger lock");
        inner.per_project.get(project).copied().unwrap_or(0)
    }

    fn head(&self) -> u64 {
        self.0.lock().expect("ledger lock").events.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn retries_count_once_and_conflicts_keep_the_first_record() {
        let ledger = MemoryLedger::default();
        assert!(matches!(
            ledger.append("alpha", "e-1", "GET /orders", 10),
            Append::Accepted(_)
        ));
        assert_eq!(
            ledger.append("alpha", "e-1", "GET /orders", 11),
            Append::Duplicate
        );
        assert_eq!(
            ledger.append("alpha", "e-1", "POST /orders", 12),
            Append::Conflict
        );
        assert_eq!(ledger.count("alpha"), 1);
        assert_eq!(ledger.read_after(0, 10)[0].operation, "GET /orders");
    }

    #[test]
    fn the_same_id_is_independent_per_project() {
        let ledger = MemoryLedger::default();
        ledger.append("alpha", "e-1", "GET /orders", 10);
        assert!(matches!(
            ledger.append("beta", "e-1", "POST /orders", 10),
            Append::Accepted(_)
        ));
        assert_eq!((ledger.count("alpha"), ledger.count("beta")), (1, 1));
    }

    #[test]
    fn sequence_numbers_are_dense_and_reads_resume_after_a_position() {
        let ledger = MemoryLedger::default();
        for i in 0..5 {
            ledger.append("alpha", &format!("e-{i}"), "GET /orders", 10);
        }
        assert_eq!(ledger.head(), 5);
        let seqs: Vec<u64> = ledger.read_after(2, 2).iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![3, 4]);
        assert!(ledger.read_after(5, 10).is_empty());
        assert!(ledger.read_after(u64::MAX, 10).is_empty());
    }

    #[test]
    fn concurrent_retries_accept_exactly_one() {
        let ledger = MemoryLedger::default();
        let accepted: usize = (0..16)
            .map(|_| {
                let ledger = ledger.clone();
                thread::spawn(move || {
                    matches!(
                        ledger.append("alpha", "same", "GET /orders", 10),
                        Append::Accepted(_)
                    )
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum();
        assert_eq!(accepted, 1);
        assert_eq!(ledger.head(), 1);
    }
}
