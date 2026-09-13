use crate::{Commit, Event, Key, Record, Revision, Store, StoreError};
use rusqlite::Connection;

pub struct SqlStore {
    conn: Connection,
}

impl SqlStore {
    pub fn from_conn(conn: Connection) -> Self {
        Self { conn }
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
