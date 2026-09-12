use crate::{CapabilityReport, CatalogError};

pub fn qualify(fresh_trials: u32, _evidence: &[CapabilityReport]) -> Result<(), CatalogError> {
    if fresh_trials == 0 {
        return Err(CatalogError::InsufficientTrials { got: 0 });
    }
    Ok(())
}
