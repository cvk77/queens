//! Board geometry, the generated puzzle, and the player's marks.

use crate::rating::Rating;
use crate::seed::PuzzleSeed;
use serde::{Deserialize, Serialize};

/// Smallest board that still makes an interesting puzzle.
pub const MIN_SIZE: u8 = 5;
/// Largest supported board. Bounded by the `u16` bitmasks used in the solvers.
pub const MAX_SIZE: u8 = 12;

/// A cell position, row-major from the top-left.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Coord {
    /// Row index, counted from the top.
    pub row: u8,
    /// Column index, counted from the left.
    pub col: u8,
}

impl Coord {
    /// A cell at the given row and column.
    pub const fn new(row: u8, col: u8) -> Self {
        Self { row, col }
    }

    /// True when the two cells touch, including diagonally. Queens may never be
    /// placed on touching cells.
    pub fn touches(self, other: Self) -> bool {
        if self == other {
            return false;
        }
        self.row.abs_diff(other.row) <= 1 && self.col.abs_diff(other.col) <= 1
    }

    /// The up-to-8 neighbouring cells that lie on a `size` x `size` board.
    pub fn neighbours(self, size: u8) -> impl Iterator<Item = Coord> {
        const OFFSETS: [(i16, i16); 8] = [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
        ];
        offset_cells(self, size, &OFFSETS)
    }

    /// The up-to-4 edge-sharing neighbours, used for region growth and
    /// contiguity checks.
    pub fn orthogonal_neighbours(self, size: u8) -> impl Iterator<Item = Coord> {
        const OFFSETS: [(i16, i16); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        offset_cells(self, size, &OFFSETS)
    }
}

/// Applies each offset to `cell`, dropping the results that fall off the board.
fn offset_cells(
    cell: Coord,
    size: u8,
    offsets: &'static [(i16, i16)],
) -> impl Iterator<Item = Coord> {
    let row = i16::from(cell.row);
    let col = i16::from(cell.col);
    let limit = i16::from(size);
    offsets.iter().filter_map(move |&(dr, dc)| {
        let (r, c) = (row + dr, col + dc);
        (r >= 0 && r < limit && c >= 0 && c < limit).then(|| Coord::new(r as u8, c as u8))
    })
}

/// One edge of a cell. The board renderer thickens the edges that separate
/// different regions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// The edge shared with the cell above.
    Top,
    /// The edge shared with the cell to the right.
    Right,
    /// The edge shared with the cell below.
    Bottom,
    /// The edge shared with the cell to the left.
    Left,
}

/// Every side, clockwise from top.
pub const ALL_SIDES: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

impl Side {
    /// The cell on the far side of this edge, or `None` at the board rim.
    pub fn neighbour(self, cell: Coord, size: u8) -> Option<Coord> {
        let (dr, dc) = match self {
            Side::Top => (-1, 0),
            Side::Right => (0, 1),
            Side::Bottom => (1, 0),
            Side::Left => (0, -1),
        };
        let r = i16::from(cell.row) + dr;
        let c = i16::from(cell.col) + dc;
        let limit = i16::from(size);
        (r >= 0 && r < limit && c >= 0 && c < limit).then(|| Coord::new(r as u8, c as u8))
    }
}

/// What the player has put in a cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Mark {
    /// Untouched.
    #[default]
    Empty,
    /// Player note meaning no queen can go here.
    Cross,
    /// A placed queen.
    Queen,
}

impl Mark {
    /// The next mark in the left-click cycle.
    pub fn cycled(self) -> Self {
        match self {
            Mark::Empty => Mark::Cross,
            Mark::Cross => Mark::Queen,
            Mark::Queen => Mark::Empty,
        }
    }
}

/// A generated puzzle: the region layout, its unique solution, and its rating.
///
/// Build these through [`crate::generate`]; the invariants below are
/// established there and relied on everywhere else.
///
/// # Invariants
/// - `regions.len() == size * size` and every value is in `0..size`.
/// - Each region is orthogonally contiguous and holds exactly one solution queen.
/// - `solution` is a permutation: `solution[row]` is that row's queen column.
/// - The region layout admits exactly one valid solution.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Puzzle {
    size: u8,
    regions: Vec<u8>,
    solution: Vec<u8>,
    rating: Rating,
    seed: PuzzleSeed,
}

impl Puzzle {
    /// Assembles a puzzle from already-validated parts.
    pub(crate) fn new(
        size: u8,
        regions: Vec<u8>,
        solution: Vec<u8>,
        rating: Rating,
        seed: PuzzleSeed,
    ) -> Self {
        debug_assert_eq!(regions.len(), usize::from(size) * usize::from(size));
        debug_assert_eq!(solution.len(), usize::from(size));
        Self {
            size,
            regions,
            solution,
            rating,
            seed,
        }
    }

    /// Width of the board, which is also its height and its queen count.
    pub fn size(&self) -> u8 {
        self.size
    }

    /// Number of cells on the board.
    pub fn cell_count(&self) -> usize {
        usize::from(self.size) * usize::from(self.size)
    }

    /// Region id per cell, row-major.
    pub fn regions(&self) -> &[u8] {
        &self.regions
    }

    /// Column of the solution queen in each row.
    pub fn solution(&self) -> &[u8] {
        &self.solution
    }

    /// What the solver found this puzzle to require.
    pub fn rating(&self) -> &Rating {
        &self.rating
    }

    /// The seed this puzzle regenerates from.
    pub fn seed(&self) -> PuzzleSeed {
        self.seed
    }

    /// Row-major index of a cell.
    pub fn index(&self, cell: Coord) -> usize {
        usize::from(cell.row) * usize::from(self.size) + usize::from(cell.col)
    }

    /// Region owning a cell.
    pub fn region_at(&self, cell: Coord) -> u8 {
        self.regions[self.index(cell)]
    }

    /// Every cell, row-major.
    pub fn cells(&self) -> impl Iterator<Item = Coord> {
        cells_of(self.size)
    }

    /// The solution's queen positions.
    pub fn solution_cells(&self) -> impl Iterator<Item = Coord> {
        self.solution
            .iter()
            .enumerate()
            .map(|(row, &col)| Coord::new(row as u8, col))
    }

    /// True when `cell` holds a solution queen.
    pub fn is_solution_cell(&self, cell: Coord) -> bool {
        self.solution[usize::from(cell.row)] == cell.col
    }

    /// True when this edge separates two different regions, or is the board
    /// rim. The renderer draws these thick.
    pub fn is_region_boundary(&self, cell: Coord, side: Side) -> bool {
        match side.neighbour(cell, self.size) {
            Some(other) => self.region_at(other) != self.region_at(cell),
            None => true,
        }
    }
}

/// Every cell of a `size` x `size` board, row-major.
pub fn cells_of(size: u8) -> impl Iterator<Item = Coord> {
    (0..size).flat_map(move |row| (0..size).map(move |col| Coord::new(row, col)))
}

/// The player's in-progress marks. Kept separate from [`Puzzle`] so it can be
/// snapshotted cheaply for undo/redo.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BoardState {
    size: u8,
    marks: Vec<Mark>,
}

impl BoardState {
    /// An empty board.
    pub fn new(size: u8) -> Self {
        Self {
            size,
            marks: vec![Mark::Empty; usize::from(size) * usize::from(size)],
        }
    }

    /// Rebuilds a board from stored marks, rejecting a mismatched length.
    /// Used when loading a save file that may predate a change here.
    pub fn from_marks(size: u8, marks: Vec<Mark>) -> Option<Self> {
        (marks.len() == usize::from(size) * usize::from(size)).then_some(Self { size, marks })
    }

    /// Width of the board these marks belong to.
    pub fn size(&self) -> u8 {
        self.size
    }

    /// Every mark, row-major from the top-left.
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    fn index(&self, cell: Coord) -> usize {
        usize::from(cell.row) * usize::from(self.size) + usize::from(cell.col)
    }

    /// The mark on one cell.
    pub fn get(&self, cell: Coord) -> Mark {
        self.marks[self.index(cell)]
    }

    /// Replaces the mark on one cell.
    pub fn set(&mut self, cell: Coord, mark: Mark) {
        let i = self.index(cell);
        self.marks[i] = mark;
    }

    /// Every cell holding a queen.
    pub fn queens(&self) -> impl Iterator<Item = Coord> {
        let size = usize::from(self.size);
        self.marks
            .iter()
            .enumerate()
            .filter(|(_, mark)| **mark == Mark::Queen)
            .map(move |(i, _)| Coord::new((i / size) as u8, (i % size) as u8))
    }

    /// How many queens are on the board.
    pub fn queen_count(&self) -> usize {
        self.marks.iter().filter(|m| **m == Mark::Queen).count()
    }

    /// True when nothing has been marked yet.
    pub fn is_empty(&self) -> bool {
        self.marks.iter().all(|m| *m == Mark::Empty)
    }

    /// Clears every mark.
    pub fn clear(&mut self) {
        self.marks.fill(Mark::Empty);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_covers_diagonals_but_not_self() {
        let c = Coord::new(3, 3);
        assert!(c.touches(Coord::new(2, 2)));
        assert!(c.touches(Coord::new(4, 3)));
        assert!(!c.touches(c));
        assert!(!c.touches(Coord::new(3, 5)));
        assert!(!c.touches(Coord::new(1, 3)));
    }

    #[test]
    fn neighbours_are_clipped_at_the_rim() {
        assert_eq!(Coord::new(0, 0).neighbours(8).count(), 3);
        assert_eq!(Coord::new(0, 3).neighbours(8).count(), 5);
        assert_eq!(Coord::new(3, 3).neighbours(8).count(), 8);
        assert_eq!(Coord::new(0, 0).orthogonal_neighbours(8).count(), 2);
        assert_eq!(Coord::new(3, 3).orthogonal_neighbours(8).count(), 4);
    }

    #[test]
    fn mark_cycle_returns_to_empty() {
        let mut m = Mark::Empty;
        m = m.cycled();
        assert_eq!(m, Mark::Cross);
        m = m.cycled();
        assert_eq!(m, Mark::Queen);
        m = m.cycled();
        assert_eq!(m, Mark::Empty);
    }

    #[test]
    fn board_state_tracks_queens() {
        let mut b = BoardState::new(6);
        assert!(b.is_empty());
        b.set(Coord::new(1, 2), Mark::Queen);
        b.set(Coord::new(4, 5), Mark::Queen);
        b.set(Coord::new(0, 0), Mark::Cross);
        assert_eq!(b.queen_count(), 2);
        assert_eq!(
            b.queens().collect::<Vec<_>>(),
            vec![Coord::new(1, 2), Coord::new(4, 5)]
        );
        b.clear();
        assert!(b.is_empty());
    }

    #[test]
    fn from_marks_rejects_a_length_mismatch() {
        assert!(BoardState::from_marks(4, vec![Mark::Empty; 16]).is_some());
        assert!(BoardState::from_marks(4, vec![Mark::Empty; 15]).is_none());
    }
}
