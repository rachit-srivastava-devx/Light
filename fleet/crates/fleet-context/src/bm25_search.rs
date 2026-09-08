//! `TantivyIndex::search` -- split out of `bm25.rs` to keep both files under the 80-line gate.
//! Returns up to `k` `(SymbolId, bm25_score)` pairs, descending by score, ties broken by
//! `SymbolId` ordering (tantivy's own tie order is not guaranteed stable).

use crate::bm25::TantivyIndex;
use crate::error::ContextError;
use crate::types::SymbolId;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::TantivyDocument;

impl TantivyIndex {
    pub fn search(&self, query: &str, k: u32) -> Result<Vec<(SymbolId, f32)>, ContextError> {
        let searcher = self.reader.searcher();
        let parser = QueryParser::for_index(&self.index, vec![self.text_field]);
        let parsed = parser
            .parse_query(query)
            .map_err(|e| ContextError::Bm25Index(e.to_string()))?;
        let top = searcher
            .search(&parsed, &TopDocs::with_limit(k as usize).order_by_score())
            .map_err(|e| ContextError::Bm25Index(e.to_string()))?;
        let mut results = Vec::with_capacity(top.len());
        for (score, address) in top {
            let stored = searcher
                .doc::<TantivyDocument>(address)
                .map_err(|e| ContextError::Bm25Index(e.to_string()))?;
            let id_text = stored
                .get_first(self.id_field)
                .and_then(|v| v.as_str())
                .ok_or_else(|| ContextError::Bm25Index("missing id field".into()))?;
            results.push((SymbolId(id_text.to_string()), score));
        }
        results.sort_by(|(a_id, a_score), (b_id, b_score)| {
            b_score
                .partial_cmp(a_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a_id.cmp(b_id))
        });
        Ok(results)
    }
}
