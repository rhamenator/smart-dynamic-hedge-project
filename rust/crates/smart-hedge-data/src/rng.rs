//! A small, seedable, dependency-free PRNG for `SyntheticProvider`.
//!
//! Deliberately **not** a port of CPython's Mersenne Twister. Nothing in
//! this system needs the Rust and Python synthetic providers to produce
//! bit-identical price paths for the same seed — `SyntheticProvider` is a
//! zero-cost research fixture (SDH-HLR-130), not a cross-language
//! reproducibility contract. What actually matters, and is tested, is:
//! the same `(symbol, time bucket)` always produces the same snapshot
//! within one implementation (SDH-LLR-122), and the output is a
//! plausible bar series (positive prices, bounded jumps). A hand-rolled
//! xorshift-family generator is more than adequate for that and keeps
//! this crate dependency-free.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Avoid the all-zero state, which a xorshift generator can never
        // leave.
        Rng(if seed == 0 { 0x9E3779B97F4A7C15 } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform float in `[0, 1)`.
    pub fn random(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn uniform(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.random()
    }

    /// A standard-normal-based deviate via Box-Muller. `u1` is floored
    /// away from exactly `0.0` so `ln(u1)` never produces `-Infinity`.
    pub fn gauss(&mut self, mu: f64, sigma: f64) -> f64 {
        let u1 = self.random().max(f64::MIN_POSITIVE);
        let u2 = self.random();
        let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        mu + sigma * z0
    }

    pub fn lognormvariate(&mut self, mu: f64, sigma: f64) -> f64 {
        self.gauss(mu, sigma).exp()
    }

    pub fn sign(&mut self) -> f64 {
        if self.random() < 0.5 {
            -1.0
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_produces_the_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..20 {
            assert_eq!(a.random(), b.random());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.random(), b.random());
    }

    #[test]
    fn random_stays_in_zero_one_range() {
        let mut rng = Rng::new(7);
        for _ in 0..10_000 {
            let x = rng.random();
            assert!((0.0..1.0).contains(&x), "{x} out of range");
        }
    }

    #[test]
    fn zero_seed_does_not_produce_a_stuck_zero_stream() {
        let mut rng = Rng::new(0);
        let first = rng.random();
        let second = rng.random();
        assert_ne!(first, second);
    }

    #[test]
    fn uniform_stays_within_bounds() {
        let mut rng = Rng::new(11);
        for _ in 0..1000 {
            let x = rng.uniform(0.002, 0.008);
            assert!((0.002..=0.008).contains(&x));
        }
    }

    #[test]
    fn gauss_output_is_always_finite() {
        let mut rng = Rng::new(99);
        for _ in 0..10_000 {
            assert!(rng.gauss(0.0, 0.01).is_finite());
        }
    }
}
