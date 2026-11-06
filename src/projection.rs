//! Batch projection from the ledger into an analytics store.
//!
//! The job reads events after its checkpoint, writes them to analytics, then
//! advances the checkpoint. Those are two separate systems, so a crash can
//! land between the two writes. When the job restarts it writes the same batch
//! again. Analytics may hold duplicate physical rows, and usage is the number
//! of distinct event ids, so the total does not change.

use std::{
    collections::HashSet,
    fmt,
    sync::{Arc, Mutex},
};

use crate::ledger::EventLedger;

/// One analytics row. The same event can be written more than once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageRow {
    pub seq: u64,
    pub project: String,
    pub event_id: String,
    pub operation: String,
    pub received_at: u64,
}

/// Half-open time range `[from, to)` in Unix seconds. `None` means unbounded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Window {
    pub from: Option<u64>,
    pub to: Option<u64>,
}

impl Window {
    pub fn contains(&self, at: u64) -> bool {
        self.from.is_none_or(|from| at >= from) && self.to.is_none_or(|to| at < to)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct StoreError(pub String);

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The analytics side (ClickHouse in the cloud design).
pub trait Analytics: Send + Sync {
    fn insert(&self, rows: &[UsageRow]) -> Result<(), StoreError>;
    /// Distinct event ids for a project whose receipt time is in the window.
    fn distinct_events(&self, project: &str, window: Window) -> u64;
    /// Physical rows for a project, duplicates included. Used to show replays.
    fn physical_rows(&self, project: &str) -> u64;
}

/// Where the job stopped (stored next to the ledger in the cloud design).
pub trait Checkpoint: Send + Sync {
    fn position(&self) -> u64;
    fn advance(&self, to: u64) -> Result<(), StoreError>;
    fn reset(&self);
}

#[derive(Clone, Default)]
pub struct MemoryAnalytics(Arc<Mutex<Vec<UsageRow>>>);

impl Analytics for MemoryAnalytics {
    fn insert(&self, rows: &[UsageRow]) -> Result<(), StoreError> {
        self.0
            .lock()
            .expect("analytics lock")
            .extend_from_slice(rows);
        Ok(())
    }

    fn distinct_events(&self, project: &str, window: Window) -> u64 {
        let rows = self.0.lock().expect("analytics lock");
        rows.iter()
            .filter(|row| row.project == project && window.contains(row.received_at))
            .map(|row| row.event_id.as_str())
            .collect::<HashSet<_>>()
            .len() as u64
    }

    fn physical_rows(&self, project: &str) -> u64 {
        let rows = self.0.lock().expect("analytics lock");
        rows.iter().filter(|row| row.project == project).count() as u64
    }
}

#[derive(Clone, Default)]
pub struct MemoryCheckpoint(Arc<Mutex<u64>>);

impl Checkpoint for MemoryCheckpoint {
    fn position(&self) -> u64 {
        *self.0.lock().expect("checkpoint lock")
    }

    fn advance(&self, to: u64) -> Result<(), StoreError> {
        let mut position = self.0.lock().expect("checkpoint lock");
        if to < *position {
            return Err(StoreError(format!(
                "checkpoint cannot move back from {} to {to}",
                *position
            )));
        }
        *position = to;
        Ok(())
    }

    fn reset(&self) {
        *self.0.lock().expect("checkpoint lock") = 0;
    }
}

/// A deliberate fault, used by tests and by the demo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailPoint {
    Never,
    /// Stop once after the analytics write and before the checkpoint.
    AfterAnalyticsWriteOnce,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProjectionError {
    /// The injected crash: analytics has the batch, the checkpoint does not.
    InjectedCrash {
        batch_end: u64,
    },
    Store(StoreError),
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InjectedCrash { batch_end } => {
                write!(
                    f,
                    "injected crash after writing up to seq {batch_end}, before the checkpoint"
                )
            }
            Self::Store(error) => write!(f, "store error: {error}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing new in the ledger.
    Idle,
    Projected {
        from: u64,
        to: u64,
        rows: usize,
    },
}

/// How far analytics is behind the ledger.
#[derive(Debug, PartialEq, Eq)]
pub struct Freshness {
    pub ledger_head: u64,
    pub projected_through: u64,
    pub pending_events: u64,
    /// Age in seconds of the oldest event not yet projected.
    pub oldest_pending_age_secs: Option<u64>,
}

pub struct Projector {
    ledger: Arc<dyn EventLedger>,
    analytics: Arc<dyn Analytics>,
    checkpoint: Arc<dyn Checkpoint>,
    batch_size: usize,
    fail: Mutex<FailPoint>,
    // One step at a time, so two runs never write the same batch in parallel.
    running: Mutex<()>,
}

impl Projector {
    pub fn new(
        ledger: Arc<dyn EventLedger>,
        analytics: Arc<dyn Analytics>,
        checkpoint: Arc<dyn Checkpoint>,
        batch_size: usize,
    ) -> Self {
        Self {
            ledger,
            analytics,
            checkpoint,
            batch_size: batch_size.max(1),
            fail: Mutex::new(FailPoint::Never),
            running: Mutex::new(()),
        }
    }

    pub fn inject(&self, fail: FailPoint) {
        *self.fail.lock().expect("fail lock") = fail;
    }

    /// Projects one batch.
    pub fn step(&self) -> Result<Step, ProjectionError> {
        let _running = self.running.lock().expect("projector lock");
        let from = self.checkpoint.position();
        let batch = self.ledger.read_after(from, self.batch_size);
        let Some(last) = batch.last() else {
            return Ok(Step::Idle);
        };
        let to = last.seq;
        let rows: Vec<UsageRow> = batch
            .into_iter()
            .map(|event| UsageRow {
                seq: event.seq,
                project: event.project,
                event_id: event.event_id,
                operation: event.operation,
                received_at: event.received_at,
            })
            .collect();
        self.analytics
            .insert(&rows)
            .map_err(ProjectionError::Store)?;

        let mut fail = self.fail.lock().expect("fail lock");
        if *fail == FailPoint::AfterAnalyticsWriteOnce {
            *fail = FailPoint::Never;
            return Err(ProjectionError::InjectedCrash { batch_end: to });
        }
        drop(fail);

        self.checkpoint
            .advance(to)
            .map_err(ProjectionError::Store)?;
        Ok(Step::Projected {
            from,
            to,
            rows: rows.len(),
        })
    }

    /// Runs steps until the ledger has nothing new. Returns the batches written.
    pub fn catch_up(&self) -> Result<usize, ProjectionError> {
        let mut batches = 0;
        while let Step::Projected { .. } = self.step()? {
            batches += 1;
        }
        Ok(batches)
    }

    /// Projects the whole ledger again from the start.
    pub fn replay(&self) -> Result<usize, ProjectionError> {
        self.checkpoint.reset();
        self.catch_up()
    }

    pub fn freshness(&self, now: u64) -> Freshness {
        let ledger_head = self.ledger.head();
        let projected_through = self.checkpoint.position();
        Freshness {
            ledger_head,
            projected_through,
            pending_events: ledger_head.saturating_sub(projected_through),
            oldest_pending_age_secs: self
                .ledger
                .read_after(projected_through, 1)
                .first()
                .map(|event| now.saturating_sub(event.received_at)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::MemoryLedger;

    struct Fixture {
        ledger: MemoryLedger,
        analytics: MemoryAnalytics,
        checkpoint: MemoryCheckpoint,
        projector: Projector,
    }

    fn fixture(batch_size: usize) -> Fixture {
        let ledger = MemoryLedger::default();
        let analytics = MemoryAnalytics::default();
        let checkpoint = MemoryCheckpoint::default();
        let projector = Projector::new(
            Arc::new(ledger.clone()),
            Arc::new(analytics.clone()),
            Arc::new(checkpoint.clone()),
            batch_size,
        );
        Fixture {
            ledger,
            analytics,
            checkpoint,
            projector,
        }
    }

    fn record(ledger: &MemoryLedger, project: &str, count: usize, received_at: u64) {
        for i in 0..count {
            ledger.append(
                project,
                &format!("{project}-{received_at}-{i}"),
                "GET /orders",
                received_at,
            );
        }
    }

    const ALL: Window = Window {
        from: None,
        to: None,
    };

    #[test]
    fn projects_in_batches_and_advances_the_checkpoint() {
        let f = fixture(4);
        record(&f.ledger, "alpha", 10, 100);
        assert_eq!(
            f.projector.step(),
            Ok(Step::Projected {
                from: 0,
                to: 4,
                rows: 4
            })
        );
        assert_eq!(f.checkpoint.position(), 4);
        assert_eq!(f.projector.catch_up(), Ok(2));
        assert_eq!(f.projector.step(), Ok(Step::Idle));
        assert_eq!(f.analytics.distinct_events("alpha", ALL), 10);
        assert_eq!(f.analytics.physical_rows("alpha"), 10);
    }

    #[test]
    fn a_crash_between_the_analytics_write_and_the_checkpoint_does_not_change_the_total() {
        let f = fixture(4);
        record(&f.ledger, "alpha", 6, 100);
        f.projector.inject(FailPoint::AfterAnalyticsWriteOnce);

        assert_eq!(
            f.projector.step(),
            Err(ProjectionError::InjectedCrash { batch_end: 4 })
        );
        // Analytics has the batch, the checkpoint does not.
        assert_eq!(f.checkpoint.position(), 0);
        assert_eq!(f.analytics.physical_rows("alpha"), 4);

        // Restart: the same batch is written again, then the job catches up.
        assert_eq!(f.projector.catch_up(), Ok(2));
        assert_eq!(f.analytics.physical_rows("alpha"), 10);
        assert_eq!(
            f.analytics.distinct_events("alpha", ALL),
            f.ledger.count("alpha")
        );
        assert_eq!(f.analytics.distinct_events("alpha", ALL), 6);
    }

    #[test]
    fn a_full_replay_rebuilds_the_same_totals() {
        let f = fixture(3);
        record(&f.ledger, "alpha", 5, 100);
        record(&f.ledger, "beta", 2, 100);
        f.projector.catch_up().unwrap();
        let before = (
            f.analytics.distinct_events("alpha", ALL),
            f.analytics.distinct_events("beta", ALL),
        );

        assert_eq!(f.projector.replay(), Ok(3));
        assert_eq!(f.checkpoint.position(), f.ledger.head());
        assert_eq!(
            (
                f.analytics.distinct_events("alpha", ALL),
                f.analytics.distinct_events("beta", ALL)
            ),
            before
        );
        assert_eq!(f.analytics.physical_rows("alpha"), 10);
    }

    #[test]
    fn usage_by_period_uses_the_receipt_time_and_stays_per_project() {
        let f = fixture(10);
        record(&f.ledger, "alpha", 3, 100);
        record(&f.ledger, "alpha", 2, 200);
        record(&f.ledger, "beta", 4, 150);
        f.projector.catch_up().unwrap();

        let window = |from, to| Window { from, to };
        assert_eq!(
            f.analytics
                .distinct_events("alpha", window(Some(100), Some(200))),
            3
        );
        assert_eq!(
            f.analytics
                .distinct_events("alpha", window(Some(200), None)),
            2
        );
        assert_eq!(
            f.analytics
                .distinct_events("alpha", window(None, Some(100))),
            0
        );
        assert_eq!(
            f.analytics
                .distinct_events("beta", window(Some(100), Some(200))),
            4
        );
    }

    #[test]
    fn freshness_reports_pending_events_and_the_age_of_the_oldest() {
        let f = fixture(2);
        assert_eq!(
            f.projector.freshness(500),
            Freshness {
                ledger_head: 0,
                projected_through: 0,
                pending_events: 0,
                oldest_pending_age_secs: None
            }
        );
        record(&f.ledger, "alpha", 2, 100);
        record(&f.ledger, "alpha", 1, 400);
        f.projector.step().unwrap();
        assert_eq!(
            f.projector.freshness(500),
            Freshness {
                ledger_head: 3,
                projected_through: 2,
                pending_events: 1,
                oldest_pending_age_secs: Some(100)
            }
        );
    }

    #[test]
    fn retries_into_the_ledger_never_reach_analytics_twice_without_a_crash() {
        let f = fixture(10);
        for _ in 0..5 {
            f.ledger.append("alpha", "same", "GET /orders", 100);
        }
        f.projector.catch_up().unwrap();
        assert_eq!(f.analytics.physical_rows("alpha"), 1);
    }

    #[test]
    fn the_checkpoint_never_moves_back() {
        let checkpoint = MemoryCheckpoint::default();
        checkpoint.advance(5).unwrap();
        assert!(checkpoint.advance(3).is_err());
        assert_eq!(checkpoint.position(), 5);
    }
}
