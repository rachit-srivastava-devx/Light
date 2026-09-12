pub mod auth;
pub mod github;
pub mod gmail;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NativeEvent {
    pub source: String,
    pub delivery_id: String,
    pub object_version: String,
    pub actor: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct PollPage {
    pub events: Vec<NativeEvent>,
    pub next_cursor: Option<String>,
    pub retry_after_seconds: Option<u64>,
}

pub struct SecretRef(pub String);

#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    #[error("cursor expired")]
    CursorExpired,
    #[error("auth required")]
    AuthRequired,
    #[error("unknown completion")]
    UnknownCompletion,
    #[error("provider error: {0}")]
    Provider(String),
}

pub trait ProviderClient {
    fn poll(&mut self, cursor: Option<&str>) -> Result<PollPage, ConnectorError>;
}

pub trait CredentialPort {
    fn token(&self, provider: &str) -> Result<SecretRef, ConnectorError>;
}

pub fn pull_once<C: ProviderClient>(
    client: &mut C,
    cursor: Option<&str>,
) -> Result<PollPage, ConnectorError> {
    let page = client.poll(cursor)?;
    if page.events.iter().any(|e| e.delivery_id.is_empty()) {
        return Err(ConnectorError::UnknownCompletion);
    }
    Ok(page)
}
