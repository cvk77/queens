//! The rules of Queens, applied to a player's board.
//!
//! Exactly one queen per row, per column and per region, and no two queens may
//! touch — not even diagonally. Note that unlike classic N-Queens, full
//! diagonals are unrestricted; only adjacency matters.

use crate::board::{BoardState, Coord, Mark, Puzzle};

/// Why two queens conflict.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViolationKind {
    /// The queens share a row.
    Row,
    /// The queens share a column.
    Column,
    /// The queens share a region.
    Region,
    /// The queens touch, orthogonally or diagonally.
    Touching,
}

impl ViolationKind {
    /// Message shown when the board highlights this conflict.
    pub const fn message(self) -> &'static str {
        match self {
            ViolationKind::Row => "Two queens share a row",
            ViolationKind::Column => "Two queens share a column",
            ViolationKind::Region => "Two queens share a colour",
            ViolationKind::Touching => "Two queens are touching",
        }
    }
}

/// A pair of queens that break a rule. `a` always precedes `b` in row-major
/// order, so each conflict is reported once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Violation {
    /// Which rule the pair breaks.
    pub kind: ViolationKind,
    /// The earlier of the two cells in row-major order.
    pub a: Coord,
    /// The later of the two cells in row-major order.
    pub b: Coord,
}

/// Every rule broken by the queens currently on the board.
///
/// Crosses are ignored: they are the player's own notes and carry no meaning
/// for the rules.
pub fn violations(puzzle: &Puzzle, state: &BoardState) -> Vec<Violation> {
    let queens: Vec<Coord> = state.queens().collect();
    let mut found = Vec::new();

    for (i, &a) in queens.iter().enumerate() {
        for &b in &queens[i + 1..] {
            let kind = if a.row == b.row {
                Some(ViolationKind::Row)
            } else if a.col == b.col {
                Some(ViolationKind::Column)
            } else if puzzle.region_at(a) == puzzle.region_at(b) {
                Some(ViolationKind::Region)
            } else if a.touches(b) {
                Some(ViolationKind::Touching)
            } else {
                None
            };
            if let Some(kind) = kind {
                found.push(Violation { kind, a, b });
            }
        }
    }
    found
}

/// The distinct cells involved in any violation, for highlighting. Row-major
/// order, no duplicates.
pub fn conflicting_cells(puzzle: &Puzzle, state: &BoardState) -> Vec<Coord> {
    let mut cells: Vec<Coord> = violations(puzzle, state)
        .into_iter()
        .flat_map(|v| [v.a, v.b])
        .collect();
    cells.sort_unstable();
    cells.dedup();
    cells
}

/// True when the board is a complete, legal solution: one queen per row and no
/// rule broken.
///
/// A full board with no violations necessarily satisfies every constraint —
/// `size` non-conflicting queens must occupy distinct rows, columns and
/// regions — so this is the only win condition needed.
pub fn is_solved(puzzle: &Puzzle, state: &BoardState) -> bool {
    state.queen_count() == usize::from(puzzle.size()) && violations(puzzle, state).is_empty()
}

/// True when a queen standing on `queen` forbids `cell`: same row, same
/// column, same region, or touching.
pub fn rules_out(puzzle: &Puzzle, queen: Coord, cell: Coord) -> bool {
    cell != queen
        && (cell.row == queen.row
            || cell.col == queen.col
            || puzzle.region_at(cell) == puzzle.region_at(queen)
            || cell.touches(queen))
}

/// True when placing a queen at `cell` would break no rule against the queens
/// already down. Drives the optional auto-cross assist.
pub fn is_placement_legal(puzzle: &Puzzle, state: &BoardState, cell: Coord) -> bool {
    state
        .queens()
        .all(|queen| queen != cell && !rules_out(puzzle, queen, cell))
}

/// The cells a queen at `cell` rules out: its row, its column, its region and
/// its neighbours. Used by the auto-cross assist and by the logical solver.
pub fn eliminated_by(puzzle: &Puzzle, cell: Coord) -> Vec<Coord> {
    puzzle
        .cells()
        .filter(|&other| rules_out(puzzle, cell, other))
        .collect()
}

/// Applies `state` onto a fresh board where every cell ruled out by a placed
/// queen carries a cross. Returns the number of crosses added.
pub fn auto_cross(puzzle: &Puzzle, state: &mut BoardState) -> usize {
    let queens: Vec<Coord> = state.queens().collect();
    let mut added = 0;
    for queen in queens {
        for cell in eliminated_by(puzzle, queen) {
            if state.get(cell) == Mark::Empty {
                state.set(cell, Mark::Cross);
                added += 1;
            }
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::puzzle_from_ascii;

    /// A 5x5 board whose regions are five horizontal stripes; its solution is
    /// therefore one queen per stripe.
    fn striped() -> Puzzle {
        puzzle_from_ascii(&["AAAAA", "BBBBB", "CCCCC", "DDDDD", "EEEEE"])
    }

    fn with_queens(puzzle: &Puzzle, cells: &[(u8, u8)]) -> BoardState {
        let mut state = BoardState::new(puzzle.size());
        for &(r, c) in cells {
            state.set(Coord::new(r, c), Mark::Queen);
        }
        state
    }

    #[test]
    fn detects_a_shared_row() {
        let p = striped();
        let s = with_queens(&p, &[(0, 0), (0, 3)]);
        let v = violations(&p, &s);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::Row);
    }

    #[test]
    fn detects_a_shared_column() {
        let p = striped();
        let s = with_queens(&p, &[(0, 2), (3, 2)]);
        assert_eq!(violations(&p, &s)[0].kind, ViolationKind::Column);
    }

    #[test]
    fn detects_a_shared_region() {
        // Region A is the 2x2 block in the corner, so two queens can share it
        // while sitting in different rows and columns.
        let p = puzzle_from_ascii(&["AABBB", "AABBB", "CCCDD", "CCEDD", "CCEED"]);
        let s = with_queens(&p, &[(0, 0), (1, 1)]);
        let v = violations(&p, &s);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::Region);
    }

    #[test]
    fn detects_diagonal_touching_but_allows_distant_diagonals() {
        let p = striped();
        let touching = with_queens(&p, &[(0, 0), (1, 1)]);
        assert_eq!(violations(&p, &touching)[0].kind, ViolationKind::Touching);

        // Same diagonal, two rows apart: legal in Queens, unlike N-Queens.
        let distant = with_queens(&p, &[(0, 0), (2, 2)]);
        assert!(violations(&p, &distant).is_empty());
    }

    #[test]
    fn a_full_legal_board_is_solved() {
        let p = striped();
        // One queen per stripe, all columns distinct, none touching.
        let s = with_queens(&p, &[(0, 0), (1, 2), (2, 4), (3, 1), (4, 3)]);
        assert!(violations(&p, &s).is_empty());
        assert!(is_solved(&p, &s));
    }

    #[test]
    fn an_incomplete_board_is_not_solved() {
        let p = striped();
        let s = with_queens(&p, &[(0, 0), (1, 2)]);
        assert!(!is_solved(&p, &s));
    }

    #[test]
    fn crosses_never_count_as_violations() {
        let p = striped();
        let mut s = with_queens(&p, &[(0, 0)]);
        for col in 0..5 {
            if s.get(Coord::new(1, col)) == Mark::Empty {
                s.set(Coord::new(1, col), Mark::Cross);
            }
        }
        assert!(violations(&p, &s).is_empty());
    }

    #[test]
    fn eliminated_by_covers_row_column_region_and_neighbours() {
        let p = striped();
        let ruled_out = eliminated_by(&p, Coord::new(2, 2));
        assert!(ruled_out.contains(&Coord::new(2, 4)), "same row");
        assert!(ruled_out.contains(&Coord::new(0, 2)), "same column");
        assert!(
            ruled_out.contains(&Coord::new(2, 0)),
            "same region (stripe)"
        );
        assert!(ruled_out.contains(&Coord::new(1, 1)), "touching");
        assert!(!ruled_out.contains(&Coord::new(2, 2)), "not itself");
        assert!(!ruled_out.contains(&Coord::new(4, 0)), "unrelated cell");
    }

    #[test]
    fn auto_cross_marks_exactly_the_eliminated_cells() {
        let p = striped();
        let mut s = with_queens(&p, &[(0, 0)]);
        let added = auto_cross(&p, &mut s);
        assert_eq!(added, eliminated_by(&p, Coord::new(0, 0)).len());
        assert_eq!(s.get(Coord::new(0, 3)), Mark::Cross);
        assert_eq!(s.get(Coord::new(0, 0)), Mark::Queen);
        assert_eq!(s.get(Coord::new(3, 3)), Mark::Empty);
    }

    #[test]
    fn is_placement_legal_agrees_with_violations() {
        let p = striped();
        let s = with_queens(&p, &[(0, 0)]);
        assert!(!is_placement_legal(&p, &s, Coord::new(1, 1)), "touching");
        assert!(!is_placement_legal(&p, &s, Coord::new(3, 0)), "same column");
        assert!(is_placement_legal(&p, &s, Coord::new(1, 2)));
    }
}
