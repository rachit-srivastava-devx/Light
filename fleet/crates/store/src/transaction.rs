use crate::{Commit, Event, IngestRecord, Key, Record, Revision, Store, StoreError};
use rusqlite::{Connection, OptionalExtension};

pub struct SqlStore {
    conn: Connection,
}

impl SqlStore {
    pub fn from_conn(conn: Connection) -> Self {
        Self { conn }
    }

    pub fn current_revision(&self) -> Result<Revision, StoreError> {
        self.conn
            .query_row("SELECT COALESCE(MAX(revision),0) FROM events", [], |r| {
                r.get::<_, i64>(0)
            })
            .map(|value| value as Revision)
            .map_err(db)
    }

    pub fn append_ingest(
        &mut self,
        expected: Revision,
        record: IngestRecord,
    ) -> Result<Commit, StoreError> {
        crate::ensure_ingest_schema(&self.conn)?;
        let tx = self.conn.transaction().map_err(db)?;
        let current: u64 = tx
            .query_row("SELECT COALESCE(MAX(revision),0) FROM events", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(db)? as u64;
        let prior: Option<String> = tx
            .query_row(
                "SELECT payload_digest FROM ingest_events WHERE source=?1 AND delivery_id=?2",
                [&record.source, &record.delivery_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db)?;
        if let Some(digest) = prior {
            return if digest == record.payload_digest {
                Err(StoreError::DuplicateEvent {
                    id: record.event_id,
                })
            } else {
                Err(StoreError::IngestConflict {
                    prior_digest: digest,
                    new_digest: record.payload_digest,
                })
            };
        }
        if current != expected {
            return Err(StoreError::StaleRevision { current });
        }
        let revision = current + 1;
        let stale_object_version = match record.object_version_position {
            Some(position) => {
                let newest: Option<i64> = tx
                    .query_row(
                        "SELECT MAX(object_version_position) FROM ingest_events WHERE source=?1",
                        [&record.source],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(db)?
                    .flatten();
                newest.is_some_and(|value| (position as i64) < value)
            }
            None => false,
        };
        tx.execute(
            "INSERT INTO events(id,revision,payload) VALUES(?1,?2,?3)",
            rusqlite::params![&record.event_id, revision as i64, &record.payload],
        )
        .map_err(db)?;
        tx.execute("INSERT INTO ingest_events(event_id,source,schema_version,payload_ref,delivery_id,object_version,object_version_position,external_actor,auth_metadata_ref,payload,payload_digest,injection_taint,stale_object_version) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)", rusqlite::params![&record.event_id, &record.source, record.schema_version, &record.payload_ref, &record.delivery_id, &record.object_version, record.object_version_position.map(|value| value as i64), &record.external_actor, &record.auth_metadata_ref, &record.payload, &record.payload_digest, record.injection_taint as i64, stale_object_version as i64]).map_err(db)?;
        tx.execute(
            "INSERT INTO ingest_redactions(event_id,categories,field_count) VALUES(?1,?2,?3)",
            rusqlite::params![
                &record.event_id,
                &record.redaction_categories,
                record.redaction_field_count as i64
            ],
        )
        .map_err(db)?;
        for attachment in record.attachments {
            tx.execute("INSERT INTO ingest_attachments(event_id,uri,digest,mime_type,size_bytes) VALUES(?1,?2,?3,?4,?5)", rusqlite::params![&record.event_id, &attachment.uri, &attachment.digest, &attachment.mime_type, attachment.size_bytes as i64]).map_err(db)?;
        }
        let cursor_advanced = match (record.cursor, record.cursor_position) {
            (Some(cursor), Some(position)) => {
                let prior: Option<i64> = tx
                    .query_row(
                        "SELECT cursor_position FROM ingest_cursors WHERE source=?1",
                        [&record.source],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(db)?;
                if prior.is_none_or(|value| position as i64 >= value) {
                    tx.execute("INSERT INTO ingest_cursors(source,cursor,cursor_position,revision) VALUES(?1,?2,?3,?4) ON CONFLICT(source) DO UPDATE SET cursor=excluded.cursor,cursor_position=excluded.cursor_position,revision=excluded.revision", rusqlite::params![&record.source, cursor, position as i64, revision as i64]).map_err(db)?;
                    true
                } else {
                    false
                }
            }
            (None, None) => false,
            _ => {
                return Err(StoreError::Io(
                    "cursor and cursor position must be paired".into(),
                ))
            }
        };
        let receipt_id = format!("rcpt-{}-{}", record.event_id, revision);
        tx.execute(
            "INSERT INTO receipts(id,event_id) VALUES(?1,?2)",
            [&receipt_id, &record.event_id],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(Commit {
            revision,
            event_id: record.event_id,
            receipt_id,
            cursor_advanced,
            stale_object_version,
        })
    }

    pub fn record_ingest_refusal(
        &self,
        source: &str,
        delivery_id: &str,
        error: &str,
    ) -> Result<(), StoreError> {
        crate::ensure_ingest_schema(&self.conn)?;
        self.conn
            .execute(
                "INSERT INTO ingest_refusals(source,delivery_id,error) VALUES(?1,?2,?3)",
                [source, delivery_id, error],
            )
            .map(|_| ())
            .map_err(db)
    }

    pub fn committed_ingest_cursor(&self, source: &str) -> Result<Option<String>, StoreError> {
        crate::ensure_ingest_schema(&self.conn)?;
        self.conn
            .query_row(
                "SELECT cursor FROM ingest_cursors WHERE source=?1",
                [source],
                |r| r.get(0),
            )
            .optional()
            .map_err(db)
    }
}

fn db(e: rusqlite::Error) -> StoreError {
    StoreError::Io(e.to_string())
}

impl Store for SqlStore {
    fn append_event(&mut self, expected: Revision, event: Event) -> Result<Commit, StoreError> {
        let tx = self.conn.transaction().map_err(db)?;
        // Revision = count of committed events; 0 means empty stream.
        let current = tx
            .query_row("SELECT COALESCE(MAX(revision),0) FROM events", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(db)? as u64;
        if current != expected {
            return Err(StoreError::StaleRevision { current });
        }
        let dup: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM events WHERE id=?1",
                [&event.id],
                |r| r.get(0),
            )
            .map_err(db)?;
        if dup > 0 {
            return Err(StoreError::DuplicateEvent { id: event.id });
        }
        let new_rev = current + 1;
        tx.execute(
            "INSERT INTO events(id,revision,payload) VALUES(?1,?2,?3)",
            rusqlite::params![&event.id, new_rev as i64, &event.payload],
        )
        .map_err(db)?;
        let receipt_id = format!("rcpt-{}-{}", event.id, new_rev);
        tx.execute(
            "INSERT INTO receipts(id,event_id) VALUES(?1,?2)",
            rusqlite::params![&receipt_id, &event.id],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(Commit {
            revision: new_rev,
            event_id: event.id,
            receipt_id,
            cursor_advanced: false,
            stale_object_version: false,
        })
    }

    fn load(&self, key: Key) -> Result<Option<Record>, StoreError> {
        match self.conn.query_row(
            "SELECT id,revision,payload FROM events WHERE id=?1",
            [&key],
            |row| {
                Ok(Record {
                    key: row.get(0)?,
                    revision: row.get::<_, i64>(1)? as u64,
                    payload: row.get(2)?,
                })
            },
        ) {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(db(e)),
        }
    }

    fn cas(&mut self, key: Key, expected: Revision, next: Record) -> Result<Revision, StoreError> {
        let tx = self.conn.transaction().map_err(db)?;
        let current: u64 =
            match tx.query_row("SELECT revision FROM nodes WHERE key=?1", [&key], |r| {
                r.get::<_, i64>(0)
            }) {
                Ok(v) => v as u64,
                Err(rusqlite::Error::QueryReturnedNoRows) => 0,
                Err(e) => return Err(db(e)),
            };
        if current != expected {
            return Err(StoreError::StaleRevision { current });
        }
        let new_rev = current + 1;
        tx.execute(
            "INSERT INTO nodes(key,revision,payload) VALUES(?1,?2,?3) \
             ON CONFLICT(key) DO UPDATE SET revision=excluded.revision,payload=excluded.payload",
            rusqlite::params![&key, new_rev as i64, &next.payload],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(new_rev)
    }
}
