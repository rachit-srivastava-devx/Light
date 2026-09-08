//! Output/input data shapes: `RuntimeState` (caller-supplied world snapshot), `Stage`,
//! `Decision`, `Refusal`. Data only -- no logic. Re-homed from
//! `fleet/keel/fleet/src/route.rs:81-116`.

use std::collections::{BTreeMap, BTreeSet};

/// Everything `decide` needs to know about the outside world, computed and owned by the caller.
/// This crate never mutates or re-derives any field -- it only filters `ORDER` against them.
#[derive(Clone, Debug)]
pub struct RuntimeState {
    /// Adapter families ("claude", "codex", "freelane", ...) confirmed installed AND
    /// non-interactively usable AND passing the builder/verifier capability contract for this
    /// call. Computed by the caller; this crate does not know how.
    pub capable: BTreeSet<&'static str>,
    /// Remaining token budget per adapter family. `None` = "no measured window" (treated as not
    /// usable, never as unlimited or as zero). Absent key = not measured at all.
    pub remaining: BTreeMap<String, Option<u64>>,
    /// Adapter families currently in a cooldown window (measured failure backoff).
    pub cooldown: BTreeSet<String>,
    /// Token budget this task is estimated to need; a candidate needs `remaining >= required_tokens`.
    pub required_tokens: u64,
    /// The committed preference order for the deterministic tie-breaker (stage 6). In production
    /// this is always `ORDER.iter().map(|c| c.id).collect()`; tests may shrink or reorder it to
    /// exercise the tie-breaker directly.
    pub preference: Vec<&'static str>,
}

/// One stage of the 6-stage pipeline, after filtering: how many candidates survived, out of the
/// total that started, and their ids -- the auditable trail.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Stage {
    pub number: usize,
    pub name: &'static str,
    pub checked: usize,
    pub total: usize,
    pub candidates: Vec<&'static str>,
}

/// Why routing refused to select anyone. Always names the stage that first emptied out, so the
/// operator's fix is specific ("stage 4: no lane has quota") not generic ("routing failed").
#[derive(Clone, Debug, serde::Serialize)]
pub struct Refusal {
    pub stage: usize,
    pub stage_name: &'static str,
    pub reason: String,
    pub fix: String,
}

/// The result of `decide`. `refusal.is_none()` iff a candidate was selected; the two are mutually
/// exclusive and jointly exhaustive -- never both `None`/`None` and never both `Some`/`Some`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Decision {
    pub role: &'static str,
    pub stages: Vec<Stage>,
    pub selected_adapter: Option<&'static str>,
    pub requested_model: Option<&'static str>,
    pub resolved_model: Option<&'static str>,
    pub decided_at_stage: Option<usize>,
    pub refusal: Option<Refusal>,
}
