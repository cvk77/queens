//! The live game: the puzzle, the player's marks, and everything derived from
//! them.

use bevy::prelude::*;
use queens_core::{BoardState, Coord, Hint, Mark, Puzzle, RegionNames, logic, rules};

/// How many board snapshots the undo stack keeps. A snapshot is one byte per
/// cell (144 at most), so this is generous and still trivial.
const MAX_HISTORY: usize = 256;

/// A puzzle in play.
///
/// Undo history is stored as whole-board snapshots rather than a move log:
/// at this size the memory is irrelevant, and it makes undo immune to the
/// replay bugs that a log invites — especially with auto-cross turning one
/// click into many changes.
#[derive(Resource)]
pub struct Session {
    pub puzzle: Puzzle,
    pub board: BoardState,
    /// Seconds of active play.
    pub elapsed: f32,
    /// Cells currently breaking a rule, for highlighting.
    pub conflicts: Vec<Coord>,
    /// The last hint and the cells it points at, cleared as soon as it is
    /// acted on.
    pub hint: Option<Hint>,
    /// Which crosses the auto-cross assist put down, rather than the player.
    /// One flag per cell, row-major.
    auto_crossed: Vec<bool>,
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
}

/// A board and the provenance of its crosses: everything undo has to restore.
#[derive(Clone)]
struct Snapshot {
    board: BoardState,
    auto_crossed: Vec<bool>,
}

impl Session {
    /// Starts a puzzle, optionally restoring a saved position.
    pub fn new(puzzle: Puzzle, restore: Option<Restore>) -> Self {
        let size = puzzle.size();
        let cells = usize::from(size) * usize::from(size);
        let (board, elapsed, auto_crossed) = match restore {
            // A save from a different build may not match the board any more;
            // an empty board is a better outcome than a crash.
            Some(restore) => {
                let board = BoardState::from_marks(size, restore.marks)
                    .unwrap_or_else(|| BoardState::new(size));
                // Provenance from an older save may be missing or the wrong
                // length; treating those crosses as the player's own is the
                // safe way to be wrong.
                let mut flags = restore.auto_crossed;
                flags.resize(cells, false);
                (board, restore.elapsed, flags)
            }
            None => (BoardState::new(size), 0.0, vec![false; cells]),
        };

        let mut session = Self {
            puzzle,
            board,
            elapsed,
            conflicts: Vec::new(),
            hint: None,
            auto_crossed,
            past: Vec::new(),
            future: Vec::new(),
        };
        session.refresh();
        session
    }

    /// The provenance flags, for storing alongside the marks.
    pub fn auto_crossed(&self) -> &[bool] {
        &self.auto_crossed
    }

    /// True when the board is a complete, legal solution.
    pub fn is_solved(&self) -> bool {
        rules::is_solved(&self.puzzle, &self.board)
    }

    pub fn queens_placed(&self) -> usize {
        self.board.queen_count()
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Sets one cell, optionally crossing off everything the new queen rules
    /// out. Recorded as a single undo step.
    ///
    /// Taking a queen off again takes those crosses with it — see
    /// [`Session::clear_orphaned_auto_crosses`].
    pub fn set_mark(&mut self, cell: Coord, mark: Mark, auto_cross: bool) {
        let previous = self.board.get(cell);
        if previous == mark {
            return;
        }
        self.checkpoint();
        self.board.set(cell, mark);
        // The player has spoken for this cell, so it is no longer the assist's.
        self.set_auto(cell, false);

        if auto_cross && mark == Mark::Queen {
            for ruled_out in rules::eliminated_by(&self.puzzle, cell) {
                if self.board.get(ruled_out) == Mark::Empty {
                    self.board.set(ruled_out, Mark::Cross);
                    self.set_auto(ruled_out, true);
                }
            }
        }
        if previous == Mark::Queen {
            self.clear_orphaned_auto_crosses();
        }
        self.refresh();
    }

    /// Clears the board, keeping the clock running — this is a restart of the
    /// attempt, not of the puzzle.
    pub fn reset(&mut self) {
        if self.board.is_empty() {
            return;
        }
        self.checkpoint();
        self.board.clear();
        self.auto_crossed.fill(false);
        self.refresh();
    }

    pub fn undo(&mut self) {
        if let Some(previous) = self.past.pop() {
            let replaced = self.swap_in(previous);
            self.future.push(replaced);
            self.refresh();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.future.pop() {
            let replaced = self.swap_in(next);
            self.past.push(replaced);
            self.refresh();
        }
    }

    /// Asks the deductive solver what the player could work out next.
    ///
    /// `names` comes from the palette the board is drawn with, so the
    /// explanation can point at a colour rather than a region index that
    /// appears nowhere on screen.
    pub fn request_hint(&mut self, names: RegionNames<'_>) {
        self.hint = Some(logic::next_hint(&self.puzzle, &self.board, names));
    }

    /// Removes the assist's crosses that no remaining queen justifies.
    ///
    /// Placing a queen with auto-cross on marks a swathe of cells, so taking
    /// that queen off again has to take them with it — otherwise a queen
    /// placed by mistake leaves its debris behind and the board has to be
    /// tidied by hand. Crosses the player made themselves stay put, and so do
    /// any that another queen still rules out.
    fn clear_orphaned_auto_crosses(&mut self) {
        let queens: Vec<Coord> = self.board.queens().collect();
        let cells: Vec<Coord> = self.puzzle.cells().collect();
        for cell in cells {
            if self.board.get(cell) != Mark::Cross || !self.is_auto(cell) {
                continue;
            }
            let still_ruled_out = queens
                .iter()
                .any(|&queen| rules::rules_out(&self.puzzle, queen, cell));
            if !still_ruled_out {
                self.board.set(cell, Mark::Empty);
                self.set_auto(cell, false);
            }
        }
    }

    fn index(&self, cell: Coord) -> usize {
        usize::from(cell.row) * usize::from(self.puzzle.size()) + usize::from(cell.col)
    }

    fn is_auto(&self, cell: Coord) -> bool {
        self.auto_crossed[self.index(cell)]
    }

    fn set_auto(&mut self, cell: Coord, auto: bool) {
        let i = self.index(cell);
        self.auto_crossed[i] = auto;
    }

    /// Swaps a snapshot in and hands back the one it replaced.
    fn swap_in(&mut self, snapshot: Snapshot) -> Snapshot {
        Snapshot {
            board: std::mem::replace(&mut self.board, snapshot.board),
            auto_crossed: std::mem::replace(&mut self.auto_crossed, snapshot.auto_crossed),
        }
    }

    /// Records the current position so the next change can be undone.
    fn checkpoint(&mut self) {
        self.past.push(Snapshot {
            board: self.board.clone(),
            auto_crossed: self.auto_crossed.clone(),
        });
        if self.past.len() > MAX_HISTORY {
            self.past.remove(0);
        }
        // Any new move invalidates the redo branch, as in every editor.
        self.future.clear();
    }

    /// Recomputes everything derived from the board.
    fn refresh(&mut self) {
        self.conflicts = rules::conflicting_cells(&self.puzzle, &self.board);
        self.hint = None;
    }
}

/// A saved position, handed to a fresh [`Session`].
#[derive(Clone, Debug)]
pub struct Restore {
    pub marks: Vec<Mark>,
    pub elapsed: f32,
    /// Which of those crosses the assist placed. May be empty, from a save
    /// written before provenance was tracked.
    pub auto_crossed: Vec<bool>,
}

/// The puzzle the [`Generating`](crate::states::AppState::Generating) screen
/// should build next.
#[derive(Resource, Clone, Debug)]
pub struct PuzzleRequest {
    pub seed: queens_core::PuzzleSeed,
    /// The position to restore, when resuming a saved game.
    pub restore: Option<Restore>,
}

impl PuzzleRequest {
    /// A brand new puzzle.
    pub fn fresh(seed: queens_core::PuzzleSeed) -> Self {
        Self {
            seed,
            restore: None,
        }
    }

    /// Picks up where a saved game left off.
    pub fn resume(saved: &crate::persistence::InProgress) -> Self {
        Self {
            seed: saved.seed,
            restore: Some(Restore {
                marks: saved.marks.clone(),
                elapsed: saved.elapsed,
                auto_crossed: saved.auto_crossed.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use queens_core::{Difficulty, PuzzleSeed, generate};

    fn session() -> Session {
        Session::new(generate(PuzzleSeed::new(8, Difficulty::Medium, 777)), None)
    }

    /// Placing a queen with the assist on marks a swathe of cells; taking that
    /// queen off again has to take them with it, so a mis-click really is
    /// undone rather than leaving debris to tidy up by hand.
    #[test]
    fn removing_a_queen_takes_its_auto_crosses_with_it() {
        let mut session = session();
        let queen = session.puzzle.solution_cells().next().unwrap();

        session.set_mark(queen, Mark::Queen, true);
        assert!(
            session.board.marks().contains(&Mark::Cross),
            "the assist should have crossed cells off"
        );

        session.set_mark(queen, Mark::Empty, true);
        assert!(
            session.board.is_empty(),
            "the board should be back to untouched, got {:?}",
            session.board.marks()
        );
    }

    #[test]
    fn removing_a_queen_leaves_the_players_own_crosses_alone() {
        let mut session = session();
        let queen = session.puzzle.solution_cells().next().unwrap();
        // A cross the player made by their own reasoning, nowhere near the
        // queen's lines.
        let mine = session
            .puzzle
            .cells()
            .find(|&c| !rules::rules_out(&session.puzzle, queen, c) && c != queen)
            .expect("some cell is unrelated to the queen");

        session.set_mark(mine, Mark::Cross, true);
        session.set_mark(queen, Mark::Queen, true);
        session.set_mark(queen, Mark::Empty, true);

        assert_eq!(session.board.get(mine), Mark::Cross, "the player's cross");
        assert_eq!(session.board.queen_count(), 0);
    }

    #[test]
    fn a_cross_another_queen_still_rules_out_survives() {
        let mut session = session();
        let queens: Vec<Coord> = session.puzzle.solution_cells().take(2).collect();
        let (first, second) = (queens[0], queens[1]);
        // A cell both queens rule out: same column as one, same row as the
        // other is not guaranteed, so search for a genuine overlap.
        let shared = session
            .puzzle
            .cells()
            .find(|&c| {
                rules::rules_out(&session.puzzle, first, c)
                    && rules::rules_out(&session.puzzle, second, c)
            })
            .expect("the two queens overlap somewhere");

        session.set_mark(first, Mark::Queen, true);
        session.set_mark(second, Mark::Queen, true);
        assert_eq!(session.board.get(shared), Mark::Cross);

        session.set_mark(first, Mark::Empty, true);
        assert_eq!(
            session.board.get(shared),
            Mark::Cross,
            "the other queen still rules this cell out"
        );
    }

    /// The whole removal is one click, so one undo has to put it all back.
    #[test]
    fn undo_restores_a_removed_queen_and_its_crosses() {
        let mut session = session();
        let queen = session.puzzle.solution_cells().next().unwrap();

        session.set_mark(queen, Mark::Queen, true);
        let with_queen = session.board.clone();

        session.set_mark(queen, Mark::Empty, true);
        session.undo();

        assert_eq!(session.board, with_queen, "one undo restores the placement");
    }

    #[test]
    fn removal_does_nothing_special_when_the_assist_is_off() {
        let mut session = session();
        let queen = session.puzzle.solution_cells().next().unwrap();

        session.set_mark(queen, Mark::Queen, false);
        assert_eq!(session.board.queen_count(), 1);
        assert!(!session.board.marks().contains(&Mark::Cross));

        session.set_mark(queen, Mark::Empty, false);
        assert!(session.board.is_empty());
    }
}
