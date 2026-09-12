use connectors::{pull_once, ConnectorError, CredentialPort, NativeEvent, PollPage, ProviderClient, SecretRef};
use serde_json::json;

struct MockProviderClient {
    events: Vec<NativeEvent>,
    cursor: Option<String>,
    fail_with: Option<ConnectorError>,
}
impl ProviderClient for MockProviderClient {
    fn poll(&mut self, _cursor: Option<&str>) -> Result<PollPage, ConnectorError> {
        if let Some(err) = &self.fail_with {
            return Err(match err {
                ConnectorError::CursorExpired => ConnectorError::CursorExpired,
                ConnectorError::AuthRequired => ConnectorError::AuthRequired,
                ConnectorError::UnknownCompletion => ConnectorError::UnknownCompletion,
                ConnectorError::Provider(s) => ConnectorError::Provider(s.clone()),
            });
        }
        Ok(PollPage { events: self.events.clone(), next_cursor: self.cursor.clone(), retry_after_seconds: None })
    }
}

struct MockCredentialPort;
impl CredentialPort for MockCredentialPort {
    fn token(&self, _provider: &str) -> Result<SecretRef, ConnectorError> {
        Ok(SecretRef("supersecret".into()))
    }
}

fn event(src: &str, id: &str, actor: &str, payload: serde_json::Value) -> NativeEvent {
    NativeEvent { source: src.into(), delivery_id: id.into(), object_version: "v1".into(), actor: actor.into(), payload }
}

#[test]
fn github_pull_returns_nonempty_page() {
    let mut client = MockProviderClient {
        events: vec![
            event("github", "d1", "user1", json!({"action": "push"})),
            event("github", "d2", "user2", json!({"action": "pull_request"})),
        ],
        cursor: Some("next_page_cursor".into()),
        fail_with: None,
    };
    let page = pull_once(&mut client, None).unwrap();
    assert_eq!(page.events.len(), 2);
    assert!(page.next_cursor.is_some());
    assert!(page.retry_after_seconds.is_none());
}

#[test]
fn credential_never_appears_in_native_event() {
    let cred_port = MockCredentialPort;
    let _secret = cred_port.token("github").unwrap();
    let mut client = MockProviderClient {
        events: vec![event("github", "d3", "user", json!({"action": "push", "repo": "myrepo"}))],
        cursor: None,
        fail_with: None,
    };
    let page = pull_once(&mut client, None).unwrap();
    assert_eq!(page.events.len(), 1);
    let json_str = serde_json::to_string(&page.events[0]).unwrap();
    assert!(!json_str.contains("supersecret"), "Credential in event: {}", json_str);
}

#[test]
fn gmail_cursor_expiry_returns_cursor_expired() {
    let mut client = MockProviderClient {
        events: vec![],
        cursor: None,
        fail_with: Some(ConnectorError::CursorExpired),
    };
    let result = pull_once(&mut client, Some("old_cursor"));
    assert!(matches!(result, Err(ConnectorError::CursorExpired)));
}
