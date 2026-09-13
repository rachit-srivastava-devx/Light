use super::ORDER;

#[test]
fn order_ids_are_pairwise_distinct() {
    for (i, a) in ORDER.iter().enumerate() {
        for b in &ORDER[i + 1..] {
            assert_ne!(a.id, b.id, "duplicate candidate id {}", a.id);
        }
    }
}
