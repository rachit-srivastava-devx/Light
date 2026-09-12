use crate::budget::pack;
use crate::manifest::assemble_manifest;
use crate::types::{
    CompileInput, ContextError, ContextManifest, Retriever, RetrievalQuery, TokenCounter,
};

/// Compile a bounded, digest-bound context manifest from inputs and ports.
///
/// Preconditions: `reserve < budget`; mandatory spans have source digests.
/// Postcondition: `checked == total > 0`; mandatory is retained; no panic.
pub fn compile(
    input: &CompileInput,
    r: &dyn Retriever,
    counter: &dyn TokenCounter,
) -> Result<ContextManifest, ContextError> {
    // staleness guard: base_digest must match the index snapshot
    let index_digest = r.index_digest();
    if !input.base_digest.is_empty() && index_digest != input.base_digest {
        return Err(ContextError::Stale {
            expected: input.base_digest.clone(),
            actual: index_digest.to_string(),
        });
    }
    // mandatory must not be empty
    if input.mandatory.is_empty() {
        return Err(ContextError::ZeroCoverage);
    }
    let q = RetrievalQuery {
        task_digest: input.task_digest.clone(),
        base_digest: input.base_digest.clone(),
        budget: input.budget,
    };
    let candidates = r.retrieve(&q)?;
    let (evidence, omitted) =
        pack(&input.mandatory, &candidates, input.budget, input.reserve, counter)?;
    let manifest = assemble_manifest(input.mandatory.clone(), evidence, omitted);
    if manifest.total == 0 {
        return Err(ContextError::ZeroCoverage);
    }
    Ok(manifest)
}
