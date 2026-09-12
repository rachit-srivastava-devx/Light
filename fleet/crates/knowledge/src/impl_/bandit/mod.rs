//! Beta-Bernoulli Thompson-sampling bandit over model arms. See BLUEPRINT.md §3.G, §7.

mod arm;
mod random;
mod thompson;

pub use arm::{ArmStats, NoArms};
pub use random::RandomSource;
pub use thompson::pick_arm;
