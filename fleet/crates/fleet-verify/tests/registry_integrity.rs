use fleet_verify::GATES;

#[test]
fn gate_ids_are_pairwise_distinct() {
    for (i, a) in GATES.iter().enumerate() {
        for b in &GATES[i + 1..] {
            assert_ne!(a.id, b.id, "duplicate gate id in committed table");
        }
    }
}

#[test]
fn every_gate_has_a_non_empty_command() {
    for gate in GATES {
        assert!(!gate.command.is_empty(), "gate {} has an empty command", gate.id);
    }
}

#[test]
fn registry_is_non_empty() {
    assert!(!GATES.is_empty());
}
