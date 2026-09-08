//! `TantivyIndex` -- BM25 full-text index (§3 C of the blueprint). `IndexLocation::Ram` never
//! touches the filesystem; `Path` is caller-chosen, never an ambient tempdir. `search` lives in
//! `bm25_search` to keep this file under the 80-line gate.

use crate::error::ContextError;
use crate::types::SymbolId;
use std::path::Path;
use tantivy::schema::{Field, Schema, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexReader, IndexWriter};

pub struct IndexDoc<'a> {
    pub id: SymbolId,
    pub text: &'a str,
}

pub enum IndexLocation<'a> {
    Ram,
    Path(&'a Path),
}

pub struct TantivyIndex {
    pub(crate) index: Index,
    pub(crate) reader: IndexReader,
    pub(crate) id_field: Field,
    pub(crate) text_field: Field,
}

fn build_schema() -> (Schema, Field, Field) {
    let mut builder = Schema::builder();
    let id_field = builder.add_text_field("id", STRING | STORED);
    let text_field = builder.add_text_field("text", TEXT);
    (builder.build(), id_field, text_field)
}

impl TantivyIndex {
    pub fn open(location: IndexLocation<'_>) -> Result<Self, ContextError> {
        let (schema, id_field, text_field) = build_schema();
        let index = match location {
            IndexLocation::Ram => Index::create_in_ram(schema),
            IndexLocation::Path(path) => Index::create_in_dir(path, schema)
                .map_err(|e| ContextError::Bm25Index(e.to_string()))?,
        };
        let reader = index
            .reader()
            .map_err(|e| ContextError::Bm25Index(e.to_string()))?;
        Ok(Self {
            index,
            reader,
            id_field,
            text_field,
        })
    }

    pub fn index(&mut self, docs: &[IndexDoc<'_>]) -> Result<(), ContextError> {
        let mut writer: IndexWriter = self
            .index
            .writer(15_000_000)
            .map_err(|e| ContextError::Bm25Index(e.to_string()))?;
        writer
            .delete_all_documents()
            .map_err(|e| ContextError::Bm25Index(e.to_string()))?;
        for entry in docs {
            writer
                .add_document(doc!(
                    self.id_field => entry.id.as_str(),
                    self.text_field => entry.text,
                ))
                .map_err(|e| ContextError::Bm25Index(e.to_string()))?;
        }
        writer
            .commit()
            .map_err(|e| ContextError::Bm25Index(e.to_string()))?;
        self.reader
            .reload()
            .map_err(|e| ContextError::Bm25Index(e.to_string()))?;
        Ok(())
    }
}
