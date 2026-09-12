use crate::{ConnectorError, CredentialPort, SecretRef};
use std::collections::HashMap;

pub struct InMemoryCredentialPort {
    tokens: HashMap<String, String>,
}

impl InMemoryCredentialPort {
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
        }
    }

    pub fn insert(&mut self, provider: &str, token: &str) {
        self.tokens.insert(provider.to_string(), token.to_string());
    }
}

impl Default for InMemoryCredentialPort {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialPort for InMemoryCredentialPort {
    fn token(&self, provider: &str) -> Result<SecretRef, ConnectorError> {
        self.tokens
            .get(provider)
            .map(|t| SecretRef(t.clone()))
            .ok_or(ConnectorError::AuthRequired)
    }
}
