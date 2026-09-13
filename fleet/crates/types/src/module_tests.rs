use super::{Module, ModuleGraph, ModuleState, TopologicalSortError};

#[test]
fn test_module_creation() {
    let module = Module::new("module-1", "First Module", "SOW text here");
    assert_eq!(module.id, "module-1");
    assert_eq!(module.name, "First Module");
    assert_eq!(module.sow_text, "SOW text here");
    assert_eq!(module.depends_on, Vec::<String>::new());
    assert!(matches!(module.state, ModuleState::Pending));
}

#[test]
fn test_module_with_dependencies() {
    let module = Module::new("module-1", "First Module", "SOW text")
        .with_dependencies(vec!["dep-1".to_string(), "dep-2".to_string()]);
    assert_eq!(module.depends_on, vec!["dep-1", "dep-2"]);
}

#[test]
fn test_module_graph_topological_sort() {
    let mut graph = ModuleGraph::new();

    // Create modules with dependencies: A -> B -> C
    graph.add_module(Module::new("C", "C", "SOW C").with_dependencies(vec!["B".to_string()]));
    graph.add_module(Module::new("B", "B", "SOW B").with_dependencies(vec!["A".to_string()]));
    graph.add_module(Module::new("A", "A", "SOW A").with_dependencies(vec![]));

    let sorted = graph.topological_sort().unwrap();
    assert_eq!(sorted, vec!["A", "B", "C"]);
}

#[test]
fn test_module_graph_batches() {
    let mut graph = ModuleGraph::new();

    // Create modules: A (independent), B depends on A, C depends on A, D depends on B and C
    graph.add_module(Module::new("A", "A", "SOW A").with_dependencies(vec![]));
    graph.add_module(Module::new("B", "B", "SOW B").with_dependencies(vec!["A".to_string()]));
    graph.add_module(Module::new("C", "C", "SOW C").with_dependencies(vec!["A".to_string()]));
    graph.add_module(
        Module::new("D", "D", "SOW D")
            .with_dependencies(vec!["B".to_string(), "C".to_string()]),
    );

    let batches = graph.batches().unwrap();
    // Batch 0: A (no deps)
    // Batch 1: B, C (depend only on A)
    // Batch 2: D (depends on B and C)
    assert_eq!(batches.len(), 3);
    assert_eq!(batches[0], vec!["A"]);
    assert!(batches[1].contains(&"B".to_string()));
    assert!(batches[1].contains(&"C".to_string()));
    assert_eq!(batches[2], vec!["D"]);
}

#[test]
fn test_module_graph_cycle_detection() {
    let mut graph = ModuleGraph::new();

    // Create a cycle: A -> B -> C -> A
    graph.add_module(Module::new("A", "A", "SOW A").with_dependencies(vec!["C".to_string()]));
    graph.add_module(Module::new("B", "B", "SOW B").with_dependencies(vec!["A".to_string()]));
    graph.add_module(Module::new("C", "C", "SOW C").with_dependencies(vec!["B".to_string()]));

    let result = graph.topological_sort();
    assert!(matches!(result, Err(TopologicalSortError::CycleDetected)));
}
