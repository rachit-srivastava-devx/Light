#[path = "production_spec_hash.rs"]
pub(super) mod spec_hash;

use crate::impl_;
use crate::VerifyError;

pub fn acceptance_digest_for_specs(
    gates: &impl_::GatesRoot,
    specs: &[impl_::GateSpec],
) -> Result<String, VerifyError> {
    let mut hasher = blake3::Hasher::new();
    spec_hash::field(&mut hasher, b"fleet.verify.acceptance.v2");
    // The scripts can consume policy/corpus/config files that are not named by an individual
    // GateSpec. Bind the complete resolved root so changing any executable input changes the
    // candidate identity, including an override root that is itself a Git checkout.
    spec_hash::field(&mut hasher, b"resolved-assets");
    spec_hash::field(
        &mut hasher,
        super::tree::content_digest(gates.path()).as_bytes(),
    );
    spec_hash::field(&mut hasher, &(specs.len() as u64).to_le_bytes());
    for spec in specs {
        spec_hash::append_spec(&mut hasher, gates, spec)?;
    }
    Ok(format!("blake3:{}", hasher.finalize().to_hex()))
}
