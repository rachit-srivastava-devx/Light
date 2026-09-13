use connectors::{
    pull_once, ConnectorError, CredentialPort, NativeEvent, PollPage, ProviderClient, SecretRef,
};
use serde_json::json;

struct FailClient(ConnectorError);
impl ProviderClient for FailClient {
    fn poll(&mut self, _: Option<&str>) -> Result<PollPage, ConnectorError> {
        Err(match &self.0 {
            ConnectorError::UnknownCompletion => ConnectorError::UnknownCompletion,
            ConnectorError::AuthRequired => ConnectorError::AuthRequired,
            ConnectorError::Provider(s) => ConnectorError::Provider(s.clone()),
            ConnectorError::CursorExpired => ConnectorError::CursorExpired,
        })
    }
}

struct EmptyClient;
impl ProviderClient for EmptyClient {
    fn poll(&mut self, _: Option<&str>) -> Result<PollPage, ConnectorError> {
        Ok(PollPage {
            events: vec![],
            next_cursor: None,
            retry_after_seconds: None,
        })
    }
}

struct NoCredentialPort;
impl CredentialPort for NoCredentialPort {
    fn token(&self, _: &str) -> Result<SecretRef, ConnectorError> {
        Err(ConnectorError::AuthRequired)
    }
}

struct BadDeliveryClient;
impl ProviderClient for BadDeliveryClient {
    fn poll(&mut self, _: Option<&str>) -> Result<PollPage, ConnectorError> {
        let ev = NativeEvent {
            source: "gh".into(),
            delivery_id: String::new(),
            object_version: "v1".into(),
            actor: "u1".into(),
            payload: json!({}),
        };
        Ok(PollPage {
            events: vec![ev],
            next_cursor: None,
            retry_after_seconds: None,
        })
    }
}

#[test]
fn unknown_completion_propagates() {
    let result = pull_once(&mut FailClient(ConnectorError::UnknownCompletion), None);
    assert!(matches!(result, Err(ConnectorError::UnknownCompletion)));
}

#[test]
fn empty_page_is_accepted() {
    let page = pull_once(&mut EmptyClient, None).expect("empty page must succeed");
    assert!(page.events.is_empty());
    assert!(page.next_cursor.is_none());
}

#[test]
fn auth_required_when_token_missing() {
    let result = NoCredentialPort.token("github");
    assert!(matches!(result, Err(ConnectorError::AuthRequired)));
}

#[test]
fn provider_error_propagates() {
    let result = pull_once(
        &mut FailClient(ConnectorError::Provider("500".into())),
        None,
    );
    assert!(matches!(result, Err(ConnectorError::Provider(_))));
}

#[test]
fn empty_delivery_id_returns_unknown_completion() {
    let result = pull_once(&mut BadDeliveryClient, None);
    assert!(matches!(result, Err(ConnectorError::UnknownCompletion)));
}
#[test]
fn in_memory_port_returns_inserted_token() {
    use connectors::auth::InMemoryCredentialPort;
    let mut port = InMemoryCredentialPort::new();
    port.insert("github", "tok123");
    assert!(port.token("github").is_ok());
}
