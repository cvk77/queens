//! Helpers for building boards by hand in unit tests.

use crate::board::{BoardState, Coord, Mark, Puzzle};
use crate::rating::{ALL_RULES, Difficulty, Rating, RuleId};
use crate::seed::PuzzleSeed;
use crate::solver;

/// Parses a square block of letters into `(size, region ids)`.
///
/// Each distinct letter becomes a region, numbered in order of first
/// appearance, so `["AAB", "AAB", "CCB"]` yields three regions.
pub fn ascii_regions(rows: &[&str]) -> (u8, Vec<u8>) {
    let size = rows.len();
    assert!(size > 0, "an ascii board needs at least one row");
    let mut letters: Vec<char> = Vec::new();
    let mut regions = Vec::with_capacity(size * size);

    for row in rows {
        let chars: Vec<char> = row.chars().collect();
        assert_eq!(chars.len(), size, "row {row:?} is not {size} cells wide");
        for ch in chars {
            let id = match letters.iter().position(|c| *c == ch) {
                Some(i) => i,
                None => {
                    letters.push(ch);
                    letters.len() - 1
                }
            };
            regions.push(id as u8);
        }
    }
    (size as u8, regions)
}

/// Builds a [`Puzzle`] from an ascii region map, taking the first solution the
/// exhaustive solver finds.
///
/// The rating is a placeholder — tests that care about difficulty go through
/// the generator instead.
///
/// # Panics
/// Panics if the layout has no solution.
pub fn puzzle_from_ascii(rows: &[&str]) -> Puzzle {
    let (size, regions) = ascii_regions(rows);
    let solution = solver::solve_first(size, &regions)
        .unwrap_or_else(|| panic!("ascii board {rows:?} has no solution"));
    Puzzle::new(
        size,
        regions,
        solution,
        Rating {
            difficulty: Difficulty::Easy,
            hardest_rule: RuleId::Single,
            rule_counts: [0; ALL_RULES.len()],
            steps: 0,
        },
        PuzzleSeed::new(size, Difficulty::Easy, 0),
    )
}

/// A board with queens on the given cells and nothing else marked.
pub fn board_with_queens(puzzle: &Puzzle, cells: &[Coord]) -> BoardState {
    let mut state = BoardState::new(puzzle.size());
    for &cell in cells {
        state.set(cell, Mark::Queen);
    }
    state
}
