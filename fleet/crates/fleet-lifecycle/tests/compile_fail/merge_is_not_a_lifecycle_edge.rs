// There is deliberately no `Merged` state and no `merge` edge: an agent that opens a PR
// must never self-approve or self-merge (author != integrator, SDLC gate model).
// If someone adds one, this test breaks and they must justify it in review.
use fleet_lifecycle::{Proposed, Task};

fn main() {
    let task: Task<Proposed> = unreachable!();
    let _ = task.merge();
}
