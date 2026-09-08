//! `pick_arm` — Thompson sampling over models (Beta-Bernoulli bandit). See BLUEPRINT.md §3.G.

use super::arm::{ArmStats, NoArms};
use super::random::{gamma_sample, RandomSource};

/// One `Beta(alpha, beta)` sample, as the ratio of two independent Gamma draws.
fn beta_sample(alpha: f64, beta: f64, rng: &mut dyn RandomSource) -> f64 {
    let x = gamma_sample(alpha, rng);
    let y = gamma_sample(beta, rng);
    x / (x + y)
}

/// Thompson sampling: draw one `Beta(successes + 1, failures + 1)` sample per arm (Jeffreys-style
/// `+1` prior), return the `id` of the arm with the highest draw. Ties break toward the earlier
/// arm in `arms`. Never panics; the only failure mode is `NoArms`.
pub fn pick_arm(arms: &[ArmStats], rng: &mut dyn RandomSource) -> Result<&'static str, NoArms> {
    if arms.is_empty() {
        return Err(NoArms);
    }
    let mut best_id = arms[0].id;
    let mut best_sample = f64::NEG_INFINITY;
    for arm in arms {
        let sample = beta_sample((arm.successes + 1) as f64, (arm.failures + 1) as f64, rng);
        if sample > best_sample {
            best_sample = sample;
            best_id = arm.id;
        }
    }
    Ok(best_id)
}
