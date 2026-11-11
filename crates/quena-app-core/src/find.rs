//! Find Sessions: one initial search, optionally followed by incremental highlighting.

use crate::AppCore;
use crate::dto::spec_of;
use anyhow::{Result, anyhow};
use parking_lot::Mutex;
use quena_body::Variant;
use quena_body::decode::{Progress, variant_applies};
use quena_jobs::{JobId, JobStatus, Priority};
use quena_model::{MarkColor, SessionId};
use quena_store::Capture;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, Weak};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindOptions {
    pub text: String,
    #[serde(default)]
    pub match_case: bool,
    #[serde(default)]
    pub regex: bool,
    /// "all" | "requests" | "responses" | "urls"
    #[serde(default = "all")]
    pub scope: String,
    /// "all" | "headers" | "bodies"
    #[serde(default = "all")]
    pub examine: String,
    /// Restrict to these sessions (empty = all visible).
    #[serde(default)]
    pub ids: Vec<SessionId>,
    #[serde(default)]
    pub decode: bool,
    #[serde(default = "max_body")]
    pub max_body_mb: u64,
    pub mark: Option<MarkColor>,
    /// Keep highlighting later traffic (UI only, with a colour and no fixed id list).
    #[serde(default)]
    pub continuous: bool,
}

/// At most this much of a body is decoded (in memory) for a decoded search.
const DECODE_SEARCH_LIMIT: u64 = 64 << 20;

/// Forwards only cancellation; body progress would clobber the per-session progress.
struct CancelOnly<'a>(&'a quena_jobs::JobCtx);
impl Progress for CancelOnly<'_> {
    fn cancelled(&self) -> bool {
        self.0.cancelled()
    }
    fn progress(&self, _: u64, _: u64) {}
}

fn all() -> String {
    "all".into()
}
fn max_body() -> u64 {
    64
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindResult {
    pub ids: Vec<SessionId>,
    pub examined: usize,
    pub total: usize,
    pub done: bool,
}

struct FindMatcher {
    options: FindOptions,
    regex: Option<regex::Regex>,
    folded: String,
}

impl FindMatcher {
    fn new(options: FindOptions) -> Result<Arc<Self>> {
        if options.text.is_empty() {
            return Err(anyhow!("nothing to find"));
        }
        let regex = if options.regex {
            Some(regex::RegexBuilder::new(&options.text).case_insensitive(!options.match_case).build().map_err(|e| anyhow!("regex: {e}"))?)
        } else {
            None
        };
        let folded = options.text.to_lowercase();
        Ok(Arc::new(Self { options, regex, folded }))
    }

    fn matches(&self, cap: &Capture, id: SessionId, ctx: &quena_jobs::JobCtx) -> bool {
        let o = &self.options;
        let re = &self.regex;
        let text_hit = |text: &str| match re {
            Some(re) => re.is_match(text),
            None if o.match_case => text.contains(&o.text),
            None => text.to_lowercase().contains(&self.folded),
        };
        let req = o.scope != "responses" && o.scope != "urls";
        let resp = o.scope != "requests" && o.scope != "urls";
        let headers = o.examine != "bodies";
        let bodies = o.examine != "headers";
        let max = o.max_body_mb.saturating_mul(1 << 20);
        let Some(s) = cap.index.get(id) else { return false };
        let mut hit = text_hit(&s.full_url());
        if !hit && o.scope != "urls" {
            if let Some(d) = cap.detail(id) {
                if headers {
                    if req && d.request.headers.iter().any(|(k, v)| text_hit(&format!("{k}: {v}"))) {
                        hit = true;
                    }
                    if !hit && resp {
                        if let Some(r) = &d.response {
                            hit = r.headers.iter().any(|(k, v)| text_hit(&format!("{k}: {v}"))) || text_hit(&r.reason);
                        }
                    }
                }
                if !hit && bodies {
                    if let Some((rb, sb)) = cap.bodies_of(id) {
                        let parts: Vec<(quena_body::Body, quena_model::Headers)> = [
                            req.then(|| (rb, d.request.headers.clone())),
                            resp.then(|| (sb, d.response.as_ref().map(|r| r.headers.clone()).unwrap_or_default())),
                        ]
                        .into_iter()
                        .flatten()
                        .collect();
                        for (body, h) in parts {
                            if body.is_empty() || body.len() > max {
                                continue;
                            }
                            let spec = spec_of(&h);
                            // Bodies are searched as text in their charset ("Grüße" finds
                            // the windows-1252 bytes; UTF-16 is decoded first).
                            let enc = quena_body::text::detect_body(&body, &spec).encoding;
                            let as_text = |data: &[u8]| quena_body::charset::decode(data, enc).0.into_owned();
                            // Decoded search works on a bounded in-memory decode instead of
                            // deriving (and caching) every matching body; an existing complete
                            // cache entry is searched directly.
                            let decoded = match (o.decode && variant_applies(&spec, Variant::Decoded), spec.content_encoding.as_deref()) {
                                (true, Some(ce)) => match cap.bodies.derived(body.id(), Variant::Decoded).filter(|b| b.is_complete()) {
                                    Some(cached) => Some(cached.read_range(0, DECODE_SEARCH_LIMIT.min(max) as usize).unwrap_or_default()),
                                    None => quena_body::decode::decode_prefix(&body, ce, DECODE_SEARCH_LIMIT.min(max) as usize, &CancelOnly(ctx)).ok(),
                                },
                                _ => None,
                            };
                            let found = match (&decoded, re) {
                                (Some(data), _) => text_hit(&as_text(data)),
                                (None, Some(_)) => text_hit(&as_text(&body.read_range(0, max as usize).unwrap_or_default())),
                                (None, None) if quena_body::text::needs_transcoding(enc) => text_hit(&as_text(&body.read_range(0, max as usize).unwrap_or_default())),
                                (None, None) => {
                                    let mut f = false;
                                    let _ = quena_body::search::search_text(&body, &o.text, enc, !o.match_case, 0, &CancelOnly(ctx), |_| {
                                        f = true;
                                        false
                                    });
                                    f
                                }
                            };
                            if found {
                                hit = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
        hit
    }
}

#[derive(Default)]
pub(crate) struct LiveFindState {
    generation: u64,
    active: Option<LiveFind>,
}

struct LiveFind {
    capture: Weak<Capture>,
    numbering: u64,
    matcher: Arc<FindMatcher>,
    changes: Arc<quena_index::SessionChanges>,
    matched: HashSet<SessionId>,
    worker: Option<JobId>,
}

impl LiveFind {
    fn new(cap: &Arc<Capture>, numbering: u64, matcher: Arc<FindMatcher>) -> Self {
        Self { capture: Arc::downgrade(cap), numbering, matcher, changes: cap.index.watch_changes(), matched: HashSet::new(), worker: None }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveFindStatus {
    pub text: String,
    pub matches: usize,
}

impl AppCore {
    /// A new UI search replaces the previous initial search and continuous highlighter.
    pub fn find_sessions(self: &Arc<Self>, options: FindOptions) -> Result<JobId> {
        // Validate before replacing the current search; an invalid regex must not stop it.
        let matcher = FindMatcher::new(options)?;
        let cap = self.capture();
        let numbering = cap.numbering();
        let mut state = self.live_find.lock();
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        self.jobs.cancel_prefix("find:");
        self.jobs.cancel_prefix("find-live:");
        state.active = (matcher.options.continuous && matcher.options.mark.is_some() && matcher.options.ids.is_empty())
            .then(|| LiveFind::new(&cap, numbering, matcher.clone()));
        // The subscription is installed before the snapshot: requests and late responses
        // arriving while the initial search runs are queued, not lost between the two.
        let job = self.submit_find(cap, numbering, matcher, "find:", Some(generation));
        if let Some(active) = &mut state.active {
            active.worker = Some(job);
        }
        Ok(job)
    }

    /// MCP searches remain one-shot and never replace the user's continuous search.
    pub fn find_sessions_for(self: &Arc<Self>, options: FindOptions, client: &str) -> Result<JobId> {
        let matcher = FindMatcher::new(options)?;
        let cap = self.capture();
        let numbering = cap.numbering();
        let prefix = format!("find-{client}:");
        self.jobs.cancel_prefix(&prefix);
        Ok(self.submit_find(cap, numbering, matcher, &prefix, None))
    }

    fn submit_find(self: &Arc<Self>, cap: Arc<Capture>, numbering: u64, matcher: Arc<FindMatcher>, prefix: &str, generation: Option<u64>) -> JobId {
        let ids = if matcher.options.ids.is_empty() { cap.index.find(|_| true) } else { matcher.options.ids.clone() };
        let result = Arc::new(Mutex::new(FindResult { total: ids.len(), ..Default::default() }));
        let progress = result.clone();
        let core = Arc::downgrade(self);
        let job = self.jobs.submit(format!("{prefix}{}", quena_model::now_us()), format!("Finding \"{}\"", matcher.options.text), Priority::Interactive, true, move |ctx| {
            let Some(core) = core.upgrade() else { return Ok(()) };
            for (n, id) in ids.iter().enumerate() {
                if ctx.cancelled() || !core.find_is_current(&cap, numbering, generation) {
                    break;
                }
                ctx.progress(n as u64, ids.len() as u64);
                let hit = matcher.matches(&cap, *id, ctx) && !ctx.cancelled()
                    && core.publish_find_match(&cap, numbering, *id, matcher.options.mark, generation);
                let mut r = progress.lock();
                r.examined = n + 1;
                if hit {
                    r.ids.push(*id);
                }
            }
            progress.lock().done = true;
            Ok(())
        });
        self.finds.lock().insert(job, result);
        job
    }

    fn find_is_current(&self, cap: &Arc<Capture>, numbering: u64, generation: Option<u64>) -> bool {
        cap.numbering() == numbering && Arc::ptr_eq(cap, &self.capture())
            && generation.is_none_or(|g| self.live_find.lock().generation == g)
    }

    fn publish_find_match(&self, cap: &Arc<Capture>, numbering: u64, id: SessionId, mark: Option<MarkColor>, generation: Option<u64>) -> bool {
        // Serialize publication with Stop/replacement; a slow body search cannot apply an
        // old colour after a newer search or a completed Stop command.
        let mut state = self.live_find.lock();
        if generation.is_some_and(|g| state.generation != g) || cap.numbering() != numbering || !Arc::ptr_eq(cap, &self.capture()) {
            return false;
        }
        let Some(summary) = cap.index.get(id) else { return false };
        if let Some(mark) = mark {
            if summary.color != Some(mark) && !cap.update_summary_numbered(numbering, id, |s| s.color = Some(mark)) {
                return false;
            }
        }
        if generation.is_some() {
            if let Some(active) = &mut state.active {
                active.matched.insert(id);
            }
        }
        true
    }

    pub fn stop_find(&self) {
        let mut state = self.live_find.lock();
        state.generation = state.generation.wrapping_add(1);
        state.active = None;
        self.jobs.cancel_prefix("find:");
        self.jobs.cancel_prefix("find-live:");
    }

    pub fn live_find_status(&self) -> Option<LiveFindStatus> {
        let state = self.live_find.lock();
        state.active.as_ref().map(|a| LiveFindStatus { text: a.matcher.options.text.clone(), matches: a.matched.len() })
    }

    /// Coalesce changes on the UI ticker; body work stays on a background worker. At most
    /// one batch is in flight, and already matched sessions never trigger another body scan.
    pub(crate) fn live_find_tick(self: &Arc<Self>) {
        let cap = self.capture();
        let mut state = self.live_find.lock();
        let Some(active) = &state.active else { return };
        if active.numbering != cap.numbering() || !active.capture.upgrade().is_some_and(|c| Arc::ptr_eq(&c, &cap)) {
            let matcher = active.matcher.clone();
            state.generation = state.generation.wrapping_add(1);
            self.jobs.cancel_prefix("find:");
            self.jobs.cancel_prefix("find-live:");
            let active = LiveFind::new(&cap, cap.numbering(), matcher);
            active.changes.extend(cap.index.find(|_| true));
            state.active = Some(active);
        }
        let generation = state.generation;
        let active = state.active.as_mut().unwrap();
        if active.worker.and_then(|id| self.jobs.get(id)).is_some_and(|j| matches!(j.status(), JobStatus::Queued | JobStatus::Running)) {
            return;
        }
        let ids = active.changes.take(256);
        if ids.is_empty() {
            return;
        }
        let matcher = active.matcher.clone();
        let numbering = active.numbering;
        let core = Arc::downgrade(self);
        let job = self.jobs.submit(format!("find-live:{generation}"), "Matching new sessions", Priority::Background, false, move |ctx| {
            let Some(core) = core.upgrade() else { return Ok(()) };
            for id in ids {
                if ctx.cancelled() || !core.find_is_current(&cap, numbering, Some(generation)) {
                    break;
                }
                {
                    let mut state = core.live_find.lock();
                    if state.generation != generation {
                        break;
                    }
                    let Some(active) = &mut state.active else { break };
                    if cap.index.get(id).is_none() {
                        active.matched.remove(&id);
                        continue;
                    }
                    if active.matched.contains(&id) {
                        continue;
                    }
                }
                if cap.index.matches_filter(id) && matcher.matches(&cap, id, ctx) && !ctx.cancelled() {
                    core.publish_find_match(&cap, numbering, id, matcher.options.mark, Some(generation));
                }
            }
            Ok(())
        });
        active.worker = Some(job);
    }

    pub fn find_result(&self, job: JobId) -> Option<FindResult> {
        let progress = self.finds.lock().get(&job)?.clone();
        let result = progress.lock().clone();
        let done = result.done || self.jobs.get(job).is_some_and(|j| matches!(j.status(), JobStatus::Cancelled | JobStatus::Failed));
        Some(FindResult { done, ..result })
    }

    /// The result, removed (a client that reads it once).
    pub fn take_find_result(&self, job: JobId) -> Option<FindResult> {
        self.finds.lock().remove(&job).map(|r| r.lock().clone())
    }
}
