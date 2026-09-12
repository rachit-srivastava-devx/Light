//! Hand-rolled `RandomSource` + Gamma/Normal sampling — no `rand` dependency. Marsaglia-Tsang
//! Gamma (shape >= 1, always true here since callers add the `+1` Jeffreys prior to a `u64`) and
//! Box-Muller Normal, both closed-form given only uniform draws. O(1) expected work per draw.
//! See BLUEPRINT.md §3.G, §7.

/// The injected uniform-random source, `(0.0, 1.0)` exclusive both ends. This crate never reads
/// a thread-local or ambient RNG.
pub trait RandomSource {
    fn uniform(&mut self) -> f64;
}

/// One standard-normal sample via Box-Muller.
pub fn normal_sample(rng: &mut dyn RandomSource) -> f64 {
    let u1 = rng.uniform();
    let u2 = rng.uniform();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// One `Gamma(shape, 1)` sample via Marsaglia-Tsang, for `shape >= 1.0`.
pub fn gamma_sample(shape: f64, rng: &mut dyn RandomSource) -> f64 {
    let d = shape - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();
    loop {
        let (x, mut v) = loop {
            let x = normal_sample(rng);
            let v = 1.0 + c * x;
            if v > 0.0 {
                break (x, v);
            }
        };
        v = v * v * v;
        let u = rng.uniform();
        if u < 1.0 - 0.0331 * x.powi(4) || u.ln() < 0.5 * x * x + d * (1.0 - v + v.ln()) {
            return d * v;
        }
    }
}
