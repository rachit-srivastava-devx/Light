use crate::{
    normalize_authenticated, AuthenticatedIncomingEvent, DurableInbox, InboxDecision, IngestError,
    SourceRegistration,
};

/// Normalize and durably accept one authorized event. Refusals are recorded first.
pub fn ingest(
    inbox: &mut impl DurableInbox,
    input: AuthenticatedIncomingEvent,
    reg: &SourceRegistration,
) -> Result<InboxDecision, IngestError> {
    let source = input.source.clone();
    let delivery_id = input.delivery_id.clone();
    match normalize_authenticated(input, reg) {
        Ok(event) => inbox.accept(event),
        Err(error) => {
            inbox.refuse(&source, &delivery_id, &error)?;
            Err(error)
        }
    }
}
