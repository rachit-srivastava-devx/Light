use verify::{DenominatorResult, GATES};

fn parser_for(id: &str) -> fn(&str, &str) -> DenominatorResult {
    GATES
        .iter()
        .find(|g| g.id == id)
        .expect("gate registered")
        .parse_denominator
}

#[path = "parser_core_tests.rs"]
mod core;
#[path = "parser_edge_tests.rs"]
mod edge;
