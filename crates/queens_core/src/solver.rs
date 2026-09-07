//! Exhaustive solver. Answers "how many solutions does this layout have?",
//! which is what makes a generated puzzle fair.
//!
//! Because exactly one queen goes in each row, a solution is a permutation of
//! columns and the touching rule collapses to a constraint between consecutive
//! rows only: queens two or more rows apart can never touch. That turns the
//! search into a fast bitmask walk down the rows.

use crate::board::MAX_SIZE;

/// Counts solutions, stopping once `cap` have been found.
///
/// Pass `cap = 2` to ask the only question the generator cares about: is the
/// solution unique?
pub fn count_solutions(size: u8, regions: &[u8], cap: usize) -> usize {
    Search::new(size, regions, cap).run().0
}

/// True when the layout has exactly one solution.
pub fn has_unique_solution(size: u8, regions: &[u8]) -> bool {
    count_solutions(size, regions, 2) == 1
}

/// The first solution found, as a column per row, or `None` if there is none.
pub fn solve_first(size: u8, regions: &[u8]) -> Option<Vec<u8>> {
    Search::new(size, regions, 1).run().1
}

/// Finds a solution other than `known`, if the layout has one.
///
/// This is what lets the generator repair an ambiguous layout: the rival
/// solution names exactly which cells need to change.
pub fn find_alternative(size: u8, regions: &[u8], known: &[u8]) -> Option<Vec<u8>> {
    let mut search = Search::new(size, regions, usize::MAX);
    search.reject = Some(known.to_vec());
    search.cap = 1;
    search.run().1
}

struct Search<'a> {
    size: usize,
    regions: &'a [u8],
    cap: usize,
    found: usize,
    first: Option<Vec<u8>>,
    current: Vec<u8>,
    /// A solution to skip over, so [`find_alternative`] can look past the one
    /// the caller already has.
    reject: Option<Vec<u8>>,
    /// Bit `r` set when region `i` has at least one cell in row `r`. Lets the
    /// search abandon a branch as soon as some region can no longer be reached.
    region_rows: [u16; MAX_SIZE as usize],
}

impl<'a> Search<'a> {
    fn new(size: u8, regions: &'a [u8], cap: usize) -> Self {
        assert!(
            size <= MAX_SIZE,
            "board size {size} exceeds the {MAX_SIZE} the bitmask solver supports"
        );
        let size = usize::from(size);
        assert_eq!(
            regions.len(),
            size * size,
            "region map has the wrong length"
        );

        let mut region_rows = [0u16; MAX_SIZE as usize];
        for row in 0..size {
            for col in 0..size {
                region_rows[usize::from(regions[row * size + col])] |= 1 << row;
            }
        }

        Self {
            size,
            regions,
            cap,
            found: 0,
            first: None,
            current: vec![0; size],
            reject: None,
            region_rows,
        }
    }

    fn run(mut self) -> (usize, Option<Vec<u8>>) {
        if self.cap > 0 {
            self.descend(0, 0, 0, None);
        }
        (self.found, self.first)
    }

    /// Places a queen in `row`, given the columns and regions already spent and
    /// the previous row's column.
    fn descend(&mut self, row: usize, cols_used: u16, regions_used: u16, prev_col: Option<usize>) {
        if row == self.size {
            if self.reject.as_deref() == Some(self.current.as_slice()) {
                return;
            }
            self.found += 1;
            if self.first.is_none() {
                self.first = Some(self.current.clone());
            }
            return;
        }

        // Any region with no cell left in the rows still to come is
        // unreachable, so this branch cannot complete.
        let rows_left = !((1u16 << row) - 1);
        for region in 0..self.size {
            if regions_used & (1 << region) == 0 && self.region_rows[region] & rows_left == 0 {
                return;
            }
        }

        for col in 0..self.size {
            if cols_used & (1 << col) != 0 {
                continue;
            }
            // Adjacent columns in consecutive rows would put the queens in
            // contact diagonally.
            if prev_col.is_some_and(|prev| col.abs_diff(prev) <= 1) {
                continue;
            }
            let region = usize::from(self.regions[row * self.size + col]);
            if regions_used & (1 << region) != 0 {
                continue;
            }

            self.current[row] = col as u8;
            self.descend(
                row + 1,
                cols_used | (1 << col),
                regions_used | (1 << region),
                Some(col),
            );
            if self.found >= self.cap {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Coord, Puzzle};
    use crate::rules::{is_solved, violations};
    use crate::test_support::{ascii_regions, board_with_queens, puzzle_from_ascii};

    #[test]
    fn finds_the_single_solution_of_a_hand_built_puzzle() {
        // Regions drawn so that exactly one arrangement works.
        let rows = ["AABBB", "AABBB", "CCCDD", "CCEDD", "CCEED"];
        let (size, regions) = ascii_regions(&rows);
        let solutions = count_solutions(size, &regions, 10);
        assert!(solutions >= 1, "layout should be solvable");

        let solution = solve_first(size, &regions).expect("a solution exists");
        let puzzle = puzzle_from_ascii(&rows);
        let cells: Vec<Coord> = solution
            .iter()
            .enumerate()
            .map(|(r, &c)| Coord::new(r as u8, c))
            .collect();
        let state = board_with_queens(&puzzle, &cells);
        assert!(
            violations(&puzzle, &state).is_empty(),
            "the solver returned an illegal board: {:?}",
            violations(&puzzle, &state)
        );
        assert!(is_solved(&puzzle, &state));
    }

    #[test]
    fn every_returned_solution_obeys_the_rules() {
        // Horizontal stripes: region == row, so this has many solutions and
        // exercises the counter on a permissive layout.
        let rows = ["AAAAA", "BBBBB", "CCCCC", "DDDDD", "EEEEE"];
        let puzzle = puzzle_from_ascii(&rows);
        let (size, regions) = ascii_regions(&rows);
        assert!(count_solutions(size, &regions, 100) > 1);

        let solution = solve_first(size, &regions).unwrap();
        let cells: Vec<Coord> = solution
            .iter()
            .enumerate()
            .map(|(r, &c)| Coord::new(r as u8, c))
            .collect();
        assert!(is_solved(&puzzle, &board_with_queens(&puzzle, &cells)));
    }

    #[test]
    fn reports_zero_for_an_unsolvable_layout() {
        // Four vertical regions on a 4-wide board: the only column permutations
        // avoiding adjacency are 2-4-1-3 and 3-1-4-2, and vertical regions make
        // region == column, which those satisfy. Use a layout that instead
        // forces two queens into one region.
        let rows = ["AAAA", "AAAA", "BBCC", "BBCC"];
        let (size, regions) = ascii_regions(&rows);
        // Only three regions exist for four rows, so no solution can use one
        // queen per region.
        assert_eq!(count_solutions(size, &regions, 5), 0);
    }

    #[test]
    fn cap_stops_the_search_early() {
        let rows = ["AAAAA", "BBBBB", "CCCCC", "DDDDD", "EEEEE"];
        let (size, regions) = ascii_regions(&rows);
        let total = count_solutions(size, &regions, 1000);
        assert!(total > 2, "expected several solutions, found {total}");
        assert_eq!(count_solutions(size, &regions, 2), 2);
        assert_eq!(count_solutions(size, &regions, 1), 1);
    }

    #[test]
    fn has_unique_solution_distinguishes_the_two_cases() {
        let ambiguous = ascii_regions(&["AAAAA", "BBBBB", "CCCCC", "DDDDD", "EEEEE"]);
        assert!(!has_unique_solution(ambiguous.0, &ambiguous.1));
    }

    /// The solver is the arbiter of correctness for the whole crate, so check
    /// it against a brute-force enumeration on small boards.
    #[test]
    fn agrees_with_brute_force() {
        let rows = ["AABBB", "AABBB", "CCCDD", "CCEDD", "CCEED"];
        let (size, regions) = ascii_regions(&rows);
        let puzzle: Puzzle = puzzle_from_ascii(&rows);

        let mut brute = 0;
        let mut perm: Vec<u8> = (0..size).collect();
        permutations(&mut perm, 0, &mut |candidate: &[u8]| {
            let cells: Vec<Coord> = candidate
                .iter()
                .enumerate()
                .map(|(r, &c)| Coord::new(r as u8, c))
                .collect();
            if is_solved(&puzzle, &board_with_queens(&puzzle, &cells)) {
                brute += 1;
            }
        });

        assert_eq!(count_solutions(size, &regions, usize::MAX), brute);
    }

    /// Visits every permutation of `items`.
    fn permutations(items: &mut Vec<u8>, k: usize, visit: &mut impl FnMut(&[u8])) {
        if k == items.len() {
            visit(items);
            return;
        }
        for i in k..items.len() {
            items.swap(k, i);
            permutations(items, k + 1, visit);
            items.swap(k, i);
        }
    }
}
