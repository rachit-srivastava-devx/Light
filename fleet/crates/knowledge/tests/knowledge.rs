use knowledge::{sources, InMemoryStore, KnowledgeError, KnowledgeItem, Kind, KnowledgeStore, Scope, SystemClock};

fn make_item(
    id: &str,
    kind: Kind,
    scope: &str,
    evidence_count: u32,
    expires_at: Option<&str>,
) -> KnowledgeItem {
    KnowledgeItem {
        id: id.to_string(),
        kind,
        text_ref: format!("ref/{}", id),
        scope: Scope::new(scope),
        source_digest: format!("sha256:{}", id),
        evidence_count,
        expires_at: expires_at.map(|s| s.to_string()),
        revision: 1,
    }
}

mod tests {
    use super::*;

    #[test]
    fn out_of_scope_item_excluded() {
        let store = InMemoryStore::new();
        store
            .put_candidate(make_item("a1", Kind::Lesson, "repo/a", 1, None))
            .unwrap();
        store
            .put_candidate(make_item("b1", Kind::Lesson, "repo/b", 1, None))
            .unwrap();

        let scope_a = Scope::new("repo/a");
        let manifest = sources(&store, "", &scope_a, 10, &SystemClock).unwrap();

        assert_eq!(manifest.items.len(), 1);
        assert_eq!(manifest.checked, 1);
        assert_eq!(manifest.total, 1);
        assert_eq!(manifest.items[0].scope, scope_a);
    }

    #[test]
    fn expired_standard_refused() {
        let store = InMemoryStore::new();
        store
            .put_candidate(make_item(
                "std1",
                Kind::Standard,
                "global",
                1,
                Some("2020-01-01T00:00:00Z"),
            ))
            .unwrap();

        let scope = Scope::new("global");
        let manifest = sources(&store, "", &scope, 10, &SystemClock).unwrap();

        assert!(
            !manifest.items.iter().any(|i| i.id == "std1"),
            "expired standard must not appear in manifest"
        );
    }

    #[test]
    fn zero_evidence_candidate_not_promoted() {
        let store = InMemoryStore::new();
        let item = make_item("lesson1", Kind::Lesson, "repo/x", 0, None);
        let result = store.put_candidate(item);
        assert!(
            matches!(result, Err(KnowledgeError::InsufficientEvidence)),
            "zero evidence must return InsufficientEvidence"
        );

        let scope = Scope::new("repo/x");
        let manifest = sources(&store, "", &scope, 10, &SystemClock).unwrap();
        assert_eq!(manifest.items.len(), 0);
    }
}
