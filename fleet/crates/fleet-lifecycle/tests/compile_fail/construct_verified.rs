use fleet_lifecycle::{Task, TaskId, Verified};
use std::marker::PhantomData;

fn main() {
    let _forged = Task::<Verified> {
        id: TaskId::new("forged").unwrap(),
        retry_depth: 0,
        _s: PhantomData,
    };
}
