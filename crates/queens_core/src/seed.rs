//! The complete, tiny description of a puzzle.

use std::fmt;

use crate::board::{MAX_SIZE, MIN_SIZE};
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

    /// Decodes a share code produced by [`PuzzleSeed`]'s `Display` impl:
    /// board size, then a difficulty letter, then the RNG seed, with no
    /// separators. A bare number - just a seed, with no letter to split on -
    /// deliberately fails to decode rather than guessing a size or
    /// difficulty for it.
    pub fn parse(s: &str) -> Option<Self> {
        let split = s.find(|c: char| !c.is_ascii_digit())?;
        let (size_digits, rest) = s.split_at(split);
        let mut rest = rest.chars();
        let letter = rest.next()?;
        let seed_digits = rest.as_str();

        let size: u8 = size_digits.parse().ok()?;
        if !(MIN_SIZE..=MAX_SIZE).contains(&size) {
            return None;
        }
        let difficulty = Difficulty::from_letter(letter)?;
        let seed: u64 = seed_digits.parse().ok()?;
        Some(Self::new(size, difficulty, seed))
    }
}

/// A share code: board size, then a difficulty letter, then the RNG seed,
/// e.g. `10H392854`. Short enough to type, and round-trips through
/// [`PuzzleSeed::parse`].
impl fmt::Display for PuzzleSeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.size, self.difficulty.letter(), self.seed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rating::ALL_DIFFICULTIES;

    #[test]
    fn a_share_code_round_trips_through_display_and_parse() {
        for size in [MIN_SIZE, 8, MAX_SIZE] {
            for difficulty in ALL_DIFFICULTIES {
                for seed in [0, 1, 392_854, u64::MAX] {
                    let original = PuzzleSeed::new(size, difficulty, seed);
                    assert_eq!(PuzzleSeed::parse(&original.to_string()), Some(original));
                }
            }
        }
    }

    #[test]
    fn parse_rejects_a_bare_seed_with_no_difficulty_letter() {
        assert_eq!(PuzzleSeed::parse("392854"), None);
    }

    #[test]
    fn parse_rejects_an_empty_string() {
        assert_eq!(PuzzleSeed::parse(""), None);
    }

    #[test]
    fn parse_rejects_an_unknown_difficulty_letter() {
        assert_eq!(PuzzleSeed::parse("10Z392854"), None);
    }

    #[test]
    fn parse_rejects_an_out_of_range_size() {
        assert_eq!(PuzzleSeed::parse("99H392854"), None);
    }

    #[test]
    fn parse_rejects_garbage_after_the_seed_digits() {
        assert_eq!(PuzzleSeed::parse("10H392H854"), None);
    }

    #[test]
    fn parse_rejects_a_letter_with_no_seed_digits() {
        assert_eq!(PuzzleSeed::parse("10H"), None);
    }
}
