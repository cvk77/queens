//! A small, dependency-free, deterministic PRNG (SplitMix64).
//!
//! Vendored rather than taken from `rand`, because saved games and shared
//! puzzles store only a [`PuzzleSeed`](crate::PuzzleSeed) and regenerate the
//! board from it. The generator's output has to stay byte-identical across
//! versions; a dependency bump that changed the RNG stream or the shuffle
//! algorithm would silently invalidate every save file.

/// Deterministic pseudo-random number generator.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

impl Rng {
    /// Creates a generator from a seed. Every seed yields a distinct stream.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a uniformly distributed value in `0..n`.
    ///
    /// Uses Lemire's multiply-and-shift with rejection, so the result is free
    /// of the modulo bias a plain `% n` would introduce.
    ///
    /// # Panics
    /// Panics if `n == 0`.
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "Rng::below requires a non-empty range");
        let n = n as u64;
        // Values below this threshold would come from an incomplete final
        // bucket of the 2^64 space and are rejected to keep the draw uniform.
        let threshold = n.wrapping_neg() % n;
        loop {
            let product = u128::from(self.next_u64()) * u128::from(n);
            if (product as u64) >= threshold {
                return (product >> 64) as usize;
            }
        }
    }

    /// Returns a uniformly distributed value in `low..=high`.
    pub fn range_inclusive(&mut self, low: usize, high: usize) -> usize {
        debug_assert!(low <= high);
        low + self.below(high - low + 1)
    }

    /// Returns a uniformly distributed `f32` in `0.0..1.0`.
    pub fn unit_f32(&mut self) -> f32 {
        // 24 bits is the full mantissa precision of an f32.
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Shuffles `slice` in place (Fisher-Yates).
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            slice.swap(i, self.below(i + 1));
        }
    }

    /// Picks an index in `0..weights.len()` with probability proportional to
    /// its weight, ignoring non-positive weights. Returns `None` if every
    /// weight is non-positive.
    pub fn weighted_index(&mut self, weights: &[f32]) -> Option<usize> {
        let total: f32 = weights.iter().filter(|w| **w > 0.0).sum();
        if total <= 0.0 {
            return None;
        }
        let mut pick = self.unit_f32() * total;
        for (i, &w) in weights.iter().enumerate() {
            if w <= 0.0 {
                continue;
            }
            pick -= w;
            if pick <= 0.0 {
                return Some(i);
            }
        }
        // Only reachable through float rounding at the very top of the range.
        weights.iter().rposition(|w| *w > 0.0)
    }
}

/// One past the largest freshly minted seed.
///
/// Any `u64` is a valid seed, but the ones the game invents are kept to eight
/// digits so a player can read one off the screen and type it back in. That
/// still leaves a hundred million puzzles per size and difficulty.
pub const MAX_FRESH_SEED: u64 = 100_000_000;

/// Derives a seed from the system clock, for "give me a new puzzle" requests.
///
/// The seed itself is then stored, so play remains fully reproducible; only the
/// choice of seed is non-deterministic.
pub fn entropy_seed() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    use std::time::{SystemTime, UNIX_EPOCH};
    #[cfg(target_arch = "wasm32")]
    use web_time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    // Mix the clock with a process-local counter so seeds requested within the
    // same clock tick still differ.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    Rng::new(nanos ^ n.wrapping_mul(GOLDEN_GAMMA)).next_u64() % MAX_FRESH_SEED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_yields_same_stream() {
        let a: Vec<u64> = (0..64).map(|_| Rng::new(12345).next_u64()).collect();
        let b: Vec<u64> = (0..64).map(|_| Rng::new(12345).next_u64()).collect();
        assert_eq!(a, b);

        let mut x = Rng::new(7);
        let mut y = Rng::new(7);
        for _ in 0..1000 {
            assert_eq!(x.next_u64(), y.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut x = Rng::new(1);
        let mut y = Rng::new(2);
        let differences = (0..100).filter(|_| x.next_u64() != y.next_u64()).count();
        assert_eq!(differences, 100);
    }

    #[test]
    fn below_stays_in_range_and_covers_it() {
        let mut rng = Rng::new(99);
        let mut seen = [0u32; 7];
        for _ in 0..10_000 {
            let v = rng.below(7);
            assert!(v < 7);
            seen[v] += 1;
        }
        // With 10k draws over 7 buckets, a bucket below 1000 would indicate a
        // badly skewed distribution.
        assert!(seen.iter().all(|c| *c > 1000), "skewed: {seen:?}");
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut rng = Rng::new(4);
        let mut v: Vec<usize> = (0..50).collect();
        rng.shuffle(&mut v);
        assert_ne!(v, (0..50).collect::<Vec<_>>(), "shuffle did nothing");
        v.sort_unstable();
        assert_eq!(v, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn weighted_index_respects_zero_weights() {
        let mut rng = Rng::new(5);
        for _ in 0..500 {
            let i = rng.weighted_index(&[0.0, 3.0, 0.0, 1.0]).unwrap();
            assert!(i == 1 || i == 3);
        }
        assert_eq!(rng.weighted_index(&[0.0, 0.0]), None);
    }
}
