use super::event::PipelineError;
use ingest::DurableInbox;
use std::path::Path;
use types::TaskId;

pub(super) fn ingest_task(
    state_dir: &Path,
    task: &TaskId,
) -> Result<serde_json::Value, PipelineError> {
    let path = state_dir.join("ingest.sqlite");
    let conn = store::open(&path).map_err(|e| PipelineError::Event(e.to_string()))?;
    store::migrate(&conn).map_err(|e| PipelineError::Event(e.to_string()))?;
    let store = store::SqlStore::from_conn(conn);
    let revision = store
        .current_revision()
        .map_err(|e| PipelineError::Event(e.to_string()))?;
    let mut inbox = ingest::SqlDurableInbox::new(store, revision);
    let input = ingest::AuthenticatedIncomingEvent {
        source: "user_cli".into(),
        delivery_id: task.as_str().into(),
        object_version: task.as_str().into(),
        object_version_position: None,
        payload: serde_json::json!({ "task": task.as_str() }),
        attachments: vec![],
        auth: ingest::AuthEvidence {
            external_actor: "local-cli".into(),
            auth_metadata_ref: "local-process".into(),
        },
        cursor: None,
        cursor_position: None,
    };
    let decision = ingest::ingest(
        &mut inbox,
        input,
        &ingest::SourceRegistration {
            namespace: "user_cli".into(),
            schema_version: 1,
        },
    )
    .map_err(|e| PipelineError::Event(e.to_string()))?;
    match decision {
        ingest::InboxDecision::Accepted { .. } => {
            let event_ref = inbox.committed_event_ref().ok_or_else(|| {
                PipelineError::Event("accepted ingest has no committed reference".into())
            })?;
            let control_ref = event_ref.control_ref();
            let mut body = serde_json::to_value(control_ref).map_err(|error| {
                PipelineError::Event(format!("could not serialize control reference: {error}"))
            })?;
            body["task_id"] = serde_json::Value::String(task.as_str().into());
            Ok(body)
        }
        ingest::InboxDecision::Duplicate { event_id } => Ok(serde_json::json!({
            "task_id": task.as_str(),
            "event_id": event_id,
            "deduplicated": true,
        })),
        ingest::InboxDecision::Conflict {
            prior_digest,
            new_digest,
        } => Err(PipelineError::Event(format!(
            "ingest conflict for task {}: prior={prior_digest}, new={new_digest}",
            task.as_str()
        ))),
    }
}
