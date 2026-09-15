/// Canonical connector boundary. Provider adapters own signature/scope checks;
/// ingest validates this bounded, provider-neutral envelope and the body digest.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectorEnvelope {
    pub source: String,
    pub delivery_id: String,
    pub object_version: String,
    pub schema_version: u64,
    pub actor: String,
    pub payload_ref: String,
    pub payload_digest: String,
    pub auth_metadata_ref: String,
}
