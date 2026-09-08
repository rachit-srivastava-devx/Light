//! `GmailAdapter` -- polls IMAP via `UID SEARCH ... UID <last_uid>:*` against a persisted UID
//! cursor. Never `IDLE` (which requires holding a connection open, i.e. a resident daemon).

use crate::adapter::Adapter;
use crate::clock::Clock;
use crate::envelope::EventEnvelope;
use crate::event_kind::EventKind;
use crate::source_kind::SourceKind;
use crate::ids::EventId;
use mailparse::MailHeaderMap;
use serde_json::json;

/// A failure polling the mailbox over IMAP.
#[derive(Debug, thiserror::Error)]
pub enum GmailAdapterError {
    #[error("imap error: {0}")]
    Imap(#[from] imap::Error),
    #[error("tls setup failed: {0}")]
    Tls(#[from] native_tls::Error),
    #[error("message parse error: {0}")]
    Parse(#[from] mailparse::MailParseError),
}

/// Polls `host:993` (IMAPS, TLS via the `imap` crate's own connection setup) for messages with a
/// UID greater than the last one this adapter has seen.
pub struct GmailAdapter {
    host: String,
    username: String,
    password: String,
    last_uid: u32,
}

impl GmailAdapter {
    pub fn new(host: String, username: String, password: String, last_uid: u32) -> Self {
        Self { host, username, password, last_uid }
    }
}

impl Adapter for GmailAdapter {
    type Error = GmailAdapterError;

    fn source(&self) -> SourceKind {
        SourceKind::Gmail
    }

    fn pull(&mut self, clock: &dyn Clock) -> Result<Vec<EventEnvelope>, Self::Error> {
        let tls = native_tls::TlsConnector::builder().build()?;
        let client = imap::connect((self.host.as_str(), 993), &self.host, &tls)?;
        let mut session = client
            .login(&self.username, &self.password)
            .map_err(|(e, _)| e)?;
        session.select("INBOX")?;
        let query = format!("UID {}:*", self.last_uid + 1);
        let uids = session.uid_search(query)?;
        let mut envelopes = Vec::new();
        for uid in uids {
            if uid <= self.last_uid {
                continue;
            }
            let fetched = session.uid_fetch(uid.to_string(), "RFC822")?;
            let Some(msg) = fetched.iter().next().and_then(|m| m.body()) else { continue };
            let parsed = mailparse::parse_mail(msg)?;
            let subject = parsed.headers.get_first_value("Subject").unwrap_or_default();
            let from = parsed.headers.get_first_value("From").unwrap_or_default();
            let body = parsed.get_body().unwrap_or_default();
            let payload = json!({ "subject": subject, "from": from, "body": body });
            let id = EventId::derive(SourceKind::Gmail, EventKind::GmailMessageReceived, &uid.to_string());
            envelopes.push(EventEnvelope::new(
                id,
                SourceKind::Gmail,
                EventKind::GmailMessageReceived,
                clock.now_rfc3339(),
                payload,
            ));
            self.last_uid = uid;
        }
        session.logout().ok();
        Ok(envelopes)
    }
}
