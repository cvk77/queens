//! The Queens puzzle, with no engine attached.
//!
//! # The game
//! An `n` x `n` board is divided into `n` contiguous coloured regions. Place
//! `n` queens so that there is exactly one in every row, every column and every
//! region, and so that no two queens touch — not even diagonally. Unlike
//! classic N-Queens, full diagonals are unrestricted; only adjacency matters.
//!
//! # What lives here
//! - [`board`] — geometry, the puzzle, and the player's marks.
//! - [`rules`] — the four constraints, violation reporting, win detection.
//! - [`solver`] — exhaustive search, used to prove a puzzle has one solution.
//! - [`logic`] — a human-style deductive solver; rates puzzles and gives hints.
//! - [`generator`] — puzzle generation, deterministic from a [`PuzzleSeed`].
//!
//! Everything is deterministic given a seed, and this crate deliberately has no
//! dependency on Bevy, so the generator can be exercised headlessly.
//!
//! ```
//! use queens_core::{Difficulty, PuzzleSeed, generate, rules, BoardState};
//!
//! let puzzle = generate(PuzzleSeed::new(8, Difficulty::Medium, 1234));
//! assert_eq!(puzzle.size(), 8);
//!
//! // The stored solution really does solve it.
//! let mut board = BoardState::new(puzzle.size());
//! for cell in puzzle.solution_cells() {
//!     board.set(cell, queens_core::Mark::Queen);
//! }
//! assert!(rules::is_solved(&puzzle, &board));
//! ```

pub mod board;
pub mod generator;
pub mod logic;
pub mod rating;
pub mod rng;
pub mod rules;
pub mod seed;
pub mod solver;

#[cfg(test)]
pub(crate) mod test_support;

pub use board::{ALL_SIDES, BoardState, Coord, MAX_SIZE, MIN_SIZE, Mark, Puzzle, Side};
pub use generator::generate;
pub use logic::{Hint, HintKind};
pub use rating::{ALL_DIFFICULTIES, Difficulty, Rating, RuleId};
pub use rng::Rng;
pub use seed::PuzzleSeed;
