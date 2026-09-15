use super::acceptance::acceptance_digest_for_specs;
use super::tree::tree_digest_for_repo;
use crate::impl_;
use crate::types::{GateSpec, ReviewedCandidate, VerifyError};
use std::path::Path;

pub(super) fn canonical_gates(specs: &[impl_::GateSpec]) -> Vec<GateSpec> {
    specs
        .iter()
        .map(|spec| GateSpec {
            id: spec.id.to_string(),
            command: "registered".into(),
            args: Vec::new(),
        })
        .collect()
}

pub fn candidate_from_registry(tree_digest: String, _: String) -> ReviewedCandidate {
    candidate_from_specs(tree_digest, String::new(), impl_::GATES)
}

pub fn candidate_from_specs(
    tree_digest: String,
    _: String,
    specs: &[impl_::GateSpec],
) -> ReviewedCandidate {
    ReviewedCandidate {
        tree_digest,
        acceptance_digest: unresolved_acceptance_digest(specs),
        gates: canonical_gates(specs),
        coverage_floor: None,
        output_digest: None,
    }
}

pub fn candidate_from_resolved_specs(
    tree_digest: String,
    gates: &impl_::GatesRoot,
    specs: &[impl_::GateSpec],
) -> Result<ReviewedCandidate, VerifyError> {
    Ok(ReviewedCandidate {
        tree_digest,
        acceptance_digest: acceptance_digest_for_specs(gates, specs)?,
        gates: canonical_gates(specs),
        coverage_floor: None,
        output_digest: None,
    })
}

pub fn candidate_for_repo(repo: &Path) -> ReviewedCandidate {
    let tree = tree_digest_for_repo(repo);
    match impl_::GatesRoot::materialize() {
        Ok(gates) => candidate_from_resolved_specs(tree.clone(), &gates, impl_::GATES)
            .unwrap_or_else(|_| candidate_from_specs(tree.clone(), String::new(), impl_::GATES)),
        Err(_) => candidate_from_specs(tree, String::new(), impl_::GATES),
    }
}

pub fn candidate_for_repo_with_specs(
    repo: &Path,
    gates: &impl_::GatesRoot,
    specs: &[impl_::GateSpec],
) -> Result<ReviewedCandidate, VerifyError> {
    candidate_from_resolved_specs(tree_digest_for_repo(repo), gates, specs)
}

fn unresolved_acceptance_digest(specs: &[impl_::GateSpec]) -> String {
    let mut hasher = blake3::Hasher::new();
    super::acceptance::spec_hash::field(&mut hasher, b"fleet.verify.acceptance.unresolved.v1");
    super::acceptance::spec_hash::field(&mut hasher, &(specs.len() as u64).to_le_bytes());
    for spec in specs {
        super::acceptance::spec_hash::append_unresolved_spec(&mut hasher, spec);
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}
