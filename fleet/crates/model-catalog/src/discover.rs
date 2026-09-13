use crate::{Candidate, CatalogError};

#[derive(Clone, Debug)]
pub struct DiscoveryConfig {
    pub explicit_paths: Vec<std::path::PathBuf>,
}

pub fn discover(config: &DiscoveryConfig) -> Result<Vec<Candidate>, CatalogError> {
    let mut candidates = Vec::new();
    for path in &config.explicit_paths {
        if std::fs::metadata(path).is_ok() {
            let digest = format!("digest-{}", path.display());
            candidates.push(Candidate {
                path: path.clone(),
                version: "unknown".into(),
                digest,
            });
        }
    }
    Ok(candidates)
}
