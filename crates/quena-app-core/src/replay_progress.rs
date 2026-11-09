//! Per-batch replay accounting, independent of other captured traffic.

use parking_lot::Mutex;
use serde::Serialize;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayBatchInfo {
    pub id: u64,
    pub planned: usize,
    pub submitted: usize,
    pub running: usize,
    /// Finished requests, including failures.
    pub completed: usize,
    /// Network/aborted requests and HTTP status >= 400.
    pub failed: usize,
    pub peak: usize,
    pub submission_done: bool,
    pub stopped: bool,
    pub elapsed_ms: u64,
    pub requests_per_second: f64,
    pub mean_ms: f64,
    pub min_ms: Option<f64>,
    pub max_ms: Option<f64>,
}

#[derive(Default)]
struct Counts {
    submitted: usize,
    running: usize,
    completed: usize,
    failed: usize,
    peak: usize,
    submission_done: bool,
    stopped: bool,
    elapsed: Option<Duration>,
    total_ms: f64,
    min_ms: Option<f64>,
    max_ms: Option<f64>,
}

pub(crate) struct ReplayBatch {
    pub id: u64,
    planned: usize,
    started: Instant,
    counts: Mutex<Counts>,
}

impl ReplayBatch {
    pub fn new(planned: usize) -> Arc<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Arc::new(Self { id: NEXT.fetch_add(1, Ordering::Relaxed), planned, started: Instant::now(), counts: Mutex::new(Counts::default()) })
    }

    pub fn stopped(&self) -> bool { self.counts.lock().stopped }

    pub fn stop(&self) {
        let mut c = self.counts.lock();
        if !c.submission_done { c.stopped = true; }
    }

    pub fn finished(&self) -> bool { self.counts.lock().elapsed.is_some() }

    pub fn submit(self: &Arc<Self>) -> ReplayTicket {
        let mut c = self.counts.lock();
        c.submitted += 1;
        c.running += 1;
        c.peak = c.peak.max(c.running);
        ReplayTicket { batch: Some(self.clone()), started: Instant::now() }
    }

    pub fn finish_submission(&self) {
        let mut c = self.counts.lock();
        c.submission_done = true;
        if c.running == 0 { c.elapsed = Some(self.started.elapsed()); }
    }

    fn complete(&self, duration: Duration, failed: bool) {
        let mut c = self.counts.lock();
        c.running -= 1;
        c.completed += 1;
        c.failed += usize::from(failed);
        let ms = duration.as_secs_f64() * 1000.0;
        c.total_ms += ms;
        c.min_ms = Some(c.min_ms.map_or(ms, |m| m.min(ms)));
        c.max_ms = Some(c.max_ms.map_or(ms, |m| m.max(ms)));
        if c.submission_done && c.running == 0 { c.elapsed = Some(self.started.elapsed()); }
    }

    pub fn info(&self) -> ReplayBatchInfo {
        let c = self.counts.lock();
        let elapsed = c.elapsed.unwrap_or_else(|| self.started.elapsed());
        ReplayBatchInfo {
            id: self.id, planned: self.planned, submitted: c.submitted, running: c.running,
            completed: c.completed, failed: c.failed, peak: c.peak, submission_done: c.submission_done, stopped: c.stopped,
            elapsed_ms: elapsed.as_millis().min(u64::MAX as u128) as u64,
            requests_per_second: if elapsed.is_zero() { 0.0 } else { c.completed as f64 / elapsed.as_secs_f64() },
            mean_ms: if c.completed == 0 { 0.0 } else { c.total_ms / c.completed as f64 }, min_ms: c.min_ms, max_ms: c.max_ms,
        }
    }
}

/// A panicked or cancelled task still finishes its accounting as a failure.
pub(crate) struct ReplayTicket {
    batch: Option<Arc<ReplayBatch>>,
    started: Instant,
}

impl ReplayTicket {
    pub fn finish(mut self, failed: bool) {
        if let Some(b) = self.batch.take() { b.complete(self.started.elapsed(), failed); }
    }
}

impl Drop for ReplayTicket {
    fn drop(&mut self) {
        if let Some(b) = self.batch.take() { b.complete(self.started.elapsed(), true); }
    }
}

impl crate::AppCore {
    pub fn replay_batches(&self) -> Vec<ReplayBatchInfo> {
        self.replay_batches.lock().iter().rev().map(|b| b.info()).collect()
    }

    pub fn replay_stop_batch(&self, id: u64) -> bool {
        let batches = self.replay_batches.lock();
        let Some(batch) = batches.iter().find(|b| b.id == id) else { return false };
        batch.stop();
        true
    }

    pub(crate) fn new_replay_batch(&self, planned: usize) -> Arc<ReplayBatch> {
        let batch = ReplayBatch::new(planned);
        let mut batches = self.replay_batches.lock();
        // Keep recent finished batches, and never discard one still running.
        while batches.len() >= 20 {
            let Some(i) = batches.iter().position(|b| b.finished()) else { break };
            batches.remove(i);
        }
        batches.push(batch.clone());
        batch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_finishes_accounting_and_freezes_the_batch() {
        let batch = ReplayBatch::new(3);
        let finished = batch.submit();
        let cancelled = batch.submit();
        batch.stop();
        batch.finish_submission();
        finished.finish(false);
        assert_eq!((batch.info().running, batch.info().completed), (1, 1));
        drop(cancelled);
        let b = batch.info();
        assert_eq!((b.planned, b.submitted, b.completed, b.failed, b.running, b.peak), (3, 2, 2, 1, 0, 2));
        assert!(b.stopped && b.submission_done && batch.finished());
        assert_eq!(batch.info(), b);
    }
}
