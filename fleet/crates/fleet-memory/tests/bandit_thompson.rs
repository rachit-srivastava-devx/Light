//! `pick_arm()` determinism + exploration cases. See BLUEPRINT.md §9.

use fleet_memory::{pick_arm, ArmStats, RandomSource};

struct SeqRng {
    values: Vec<f64>,
    idx: usize,
}
impl RandomSource for SeqRng {
    fn uniform(&mut self) -> f64 {
        let v = self.values[self.idx % self.values.len()];
        self.idx += 1;
        v
    }
}

struct SplitMix64(u64);
impl RandomSource for SplitMix64 {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        // map to (0.0, 1.0) exclusive both ends
        ((z >> 11) as f64 / (1u64 << 53) as f64).clamp(1e-12, 1.0 - 1e-12)
    }
}

#[test]
fn pick_arm_rejects_empty_arms() {
    let mut rng = SplitMix64(1);
    assert!(pick_arm(&[], &mut rng).is_err());
}

#[test]
fn pick_arm_is_deterministic_for_a_fixed_random_source() {
    let arms = [ArmStats { id: "a", successes: 3, failures: 1 }, ArmStats { id: "b", successes: 1, failures: 3 }];
    let seq = vec![0.1, 0.5, 0.7, 0.2, 0.9, 0.3, 0.4, 0.6];
    let mut rng1 = SeqRng { values: seq.clone(), idx: 0 };
    let mut rng2 = SeqRng { values: seq, idx: 0 };
    let first = pick_arm(&arms, &mut rng1).unwrap();
    let second = pick_arm(&arms, &mut rng2).unwrap();
    assert_eq!(first, second);
}

#[test]
fn pick_arm_favors_the_higher_success_rate_arm_over_many_trials() {
    let arms = [ArmStats { id: "good", successes: 90, failures: 10 }, ArmStats { id: "bad", successes: 10, failures: 90 }];
    let mut count_good = 0;
    let mut count_bad = 0;
    for seed in 1..=2000u64 {
        let mut rng = SplitMix64(seed);
        match pick_arm(&arms, &mut rng).unwrap() {
            "good" => count_good += 1,
            "bad" => count_bad += 1,
            other => panic!("unexpected arm {other}"),
        }
    }
    assert!(count_good > count_bad, "good={count_good} bad={count_bad}");
}
