//! End-to-end `retrieve_context` over a small fixture repo, a real `TantivyIndex::open(Ram)`,
//! and `NoVectorIndex` (BM25-only, per this pass's scope adjudication -- no embedder ships).

use fleet_context::{
    build_repo_map, retrieve_context, IndexDoc, IndexLocation, Language, NoVectorIndex,
    RetrievalQuery, SourceFile, TantivyIndex, TokenModel,
};
use fleet_types::Tokens;
use std::collections::HashMap;

#[test]
fn retrieve_pipeline_end_to_end() {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/known-callers.rs"
    );
    let source = std::fs::read_to_string(fixture_path)
        .unwrap_or_else(|e| panic!("required fixture missing at {fixture_path}: {e}"));
    let files = [SourceFile {
        path: "known-callers.rs".into(),
        language: Language::Rust,
        source: source.clone(),
    }];
    let repo_map = build_repo_map(&files).unwrap();

    let mut texts: HashMap<String, String> = HashMap::new();
    for symbol in &repo_map.symbols {
        texts.insert(
            symbol.id.as_str().to_string(),
            format!("fn {} calls callee", symbol.name),
        );
    }

    let mut bm25 = TantivyIndex::open(IndexLocation::Ram).unwrap();
    let docs: Vec<IndexDoc<'_>> = repo_map
        .symbols
        .iter()
        .map(|s| IndexDoc {
            id: s.id.clone(),
            text: texts[s.id.as_str()].as_str(),
        })
        .collect();
    bm25.index(&docs).unwrap();

    let vectors = NoVectorIndex;
    let chunk_text = |id: &fleet_context::SymbolId| texts.get(id.as_str()).cloned();

    let query = RetrievalQuery {
        task_text: "callee",
        budget: Tokens::new(1000),
        top_k_bm25: 10,
        top_k_vector: 0,
        rrf_k: 60.0,
        token_model: TokenModel::Cl100kBase,
    };

    let slice = retrieve_context(&query, &repo_map, &bm25, &vectors, None, &chunk_text, None)
        .expect("pipeline succeeds");

    assert!(slice.tokens_used.get() <= slice.tokens_budget.get());
    assert!(!slice.chunks.is_empty());
}
