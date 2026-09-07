//! The complete, tiny description of a puzzle.

use crate::rating::Difficulty;
use crate::rng::entropy_seed;
use serde::{Deserialize, Serialize};

/// Everything needed to reproduce a puzzle exactly.
///
/// Generation is deterministic, so this is all a save file stores; the board
/// itself is rebuilt on load. Keep the generator's behaviour stable or old
/// saves will resolve to different puzzles.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct PuzzleSeed {
    /// Board edge length.
    pub size: u8,
    /// The band that was *requested*. The generated puzzle may fall short of
    /// it; see [`Puzzle::rating`](crate::Puzzle::rating) for what was actually
    /// produced.
    pub difficulty: Difficulty,
    /// Seed for the generator's RNG.
    pub seed: u64,
}

impl PuzzleSeed {
    pub fn new(size: u8, difficulty: Difficulty, seed: u64) -> Self {
        Self {
            size,
            difficulty,
            seed,
        }
    }

    /// A fresh puzzle request with a clock-derived seed.
    pub fn random(size: u8, difficulty: Difficulty) -> Self {
        Self::new(size, difficulty, entropy_seed())
    }
}
