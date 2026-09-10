//! Write side of `sow`'s memory wiring: dedup-writes (`fleet_memory::dedup::write`) a piece of
//! text so a later, similar `sow` call recalls it via `memory_adapter::RealMemory::
//! recall_similar`. Split out of `memory_adapter.rs` to hold that file under the ≤80-line rule.
//!
//! Two entry points over one `record`, because WHAT may be remembered is a policy decision, not
//! a storage one: `record_accepted_sow` (an accepted SOW is a real prior decision a later,
//! similar SOW should be measured against) and `record_sow_refusal` (the pipeline `Teach`
//! trailer's derived lesson). A REFUSED `sow` submission is deliberately neither: remembering a
//! draft that was rejected made the corrected resubmission collide with it
//! ("This looks like a prior decision -- which one governs?"), so a caller could never get out
//! of the hole their own rejected draft dug.

use super::clock::now;
use super::embed::{embed_text, stable_id};
use super::ports::InMemoryPorts;
use super::store::{MemoryStoreError, SowMemoryStore};
use fleet_memory::{write, DedupThreshold, Importance, MemoryKind, NewMemory, RetrieveError, WriteDecision};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum SowMemoryError {
    #[error(transparent)]
    Store(#[from] MemoryStoreError),
    #[error(transparent)]
    Retrieve(#[from] RetrieveError),
}

/// An ACCEPTED SOW: the only submission `fleet sow` itself is allowed to remember.
pub fn record_accepted_sow(state_dir: &Path, text: &str) -> Result<(), SowMemoryError> {
    record(state_dir, text)
}

/// A lesson derived from a real pipeline stage failure (`pipeline::teach_stage`), not a `sow`
/// submission -- kept on the same store so one recall surfaces both kinds.
pub fn record_sow_refusal(state_dir: &Path, text: &str) -> Result<(), SowMemoryError> {
    record(state_dir, text)
}

fn record(state_dir: &Path, text: &str) -> Result<(), SowMemoryError> {
    let store = SowMemoryStore::new(state_dir);
    let mut items = store.load()?;
    let candidate = NewMemory {
        kind: MemoryKind::Semantic,
        text: text.to_string(),
        embedding: embed_text(text),
        importance: Importance::new(0.8).expect("0.8 is within [0.0, 1.0]"),
    };
    let tau = DedupThreshold::new(0.9).expect("0.9 is finite");
    let ts = now();
    let decision = {
        let ports = InMemoryPorts { items: &items };
        write(stable_id(text), candidate, ts, tau, &ports)?
    };
    match decision {
        WriteDecision::Insert(item) => items.push(item),
        WriteDecision::Merge { into, observed_importance, observation_count, .. } => {
            if let Some(existing) = items.iter_mut().find(|it| it.id == into) {
                let merged = existing.importance.get().max(observed_importance.get());
                existing.importance = Importance::new(merged).unwrap_or(existing.importance);
                existing.confirmed_count += observation_count;
                existing.last_confirmed_at = ts;
            }
        }
    }
    Ok(store.save(&items)?)
}
