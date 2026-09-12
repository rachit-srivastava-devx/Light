use knowledge::{sources, Clock, InMemoryStore, KnowledgeItem, Kind, KnowledgeStore, Scope, SystemClock};

struct FixedClock(String);
impl Clock for FixedClock {
    fn now_iso(&self) -> String { self.0.clone() }
}

fn item(id: &str, scope: &str, evidence_count: u32) -> KnowledgeItem {
    KnowledgeItem {
        id: id.to_string(),
        kind: Kind::Lesson,
        text_ref: format!("ref/{}", id),
        scope: Scope::new(scope),
        source_digest: format!("sha256:{}", id),
        evidence_count,
        expires_at: None,
        revision: 1,
    }
}

#[test]
fn promoted_items_sorted_by_score_desc() {
    let store = InMemoryStore::new();
    store.put_candidate(item("low", "s", 1)).unwrap();
    store.put_candidate(item("high", "s", 5)).unwrap();
    store.put_candidate(item("mid", "s", 3)).unwrap();
    let clock = FixedClock("2099-01-01T00:00:00Z".to_string());
    let manifest = sources(&store, "", &Scope::new("s"), 10, &clock).unwrap();
    let ids: Vec<&str> = manifest.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, vec!["high", "mid", "low"]);
}

#[test]
fn zero_items_returns_empty_not_panic() {
    let store = InMemoryStore::new();
    let manifest = sources(&store, "", &Scope::new("empty"), 10, &SystemClock).unwrap();
    assert_eq!(manifest.items.len(), 0);
    assert_eq!(manifest.checked, 0);
    assert_eq!(manifest.total, 0);
}

#[test]
fn query_param_filters_results() {
    let store = InMemoryStore::new();
    store.put_candidate(item("rust-basics", "lang", 1)).unwrap();
    store.put_candidate(item("python-intro", "lang", 1)).unwrap();
    store.put_candidate(item("rust-advanced", "lang", 2)).unwrap();
    let manifest = sources(&store, "rust", &Scope::new("lang"), 10, &SystemClock).unwrap();
    assert_eq!(manifest.items.len(), 2);
    assert!(manifest.items.iter().all(|i| i.id.contains("rust")));
}
