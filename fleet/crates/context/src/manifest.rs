use crate::types::{ContextManifest, EvidenceRef, Span};

/// Build manifest digest from mandatory+evidence content (deterministic, not crypto).
fn simple_digest(mandatory: &[Span], evidence: &[EvidenceRef]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    for s in mandatory {
        s.source_digest.hash(&mut h);
        s.start.hash(&mut h);
        s.end.hash(&mut h);
    }
    for e in evidence {
        e.source_digest.hash(&mut h);
        e.tokens.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// Assemble a ContextManifest from packed components.
pub fn assemble_manifest(
    mandatory: Vec<Span>,
    evidence: Vec<EvidenceRef>,
    omitted: Vec<String>,
) -> ContextManifest {
    let total = (mandatory.len() + evidence.len()) as u64;
    let digest = simple_digest(&mandatory, &evidence);
    ContextManifest {
        digest,
        mandatory,
        evidence,
        omitted,
        checked: total,
        total,
    }
}
