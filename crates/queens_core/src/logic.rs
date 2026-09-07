//! A human-style deductive solver.
//!
//! This is where difficulty comes from. Rather than measuring board size or
//! search-tree depth, a puzzle is solved the way a person would, by applying
//! named deduction rules from the most obvious to the most demanding, and rated
//! by the hardest rule it required. The same engine backs the in-game hint
//! button, so a hint is always a step the player could have taken themselves.
//!
//! The board is held as a candidate bitmask per row, which keeps every rule a
//! handful of `u16` operations.

use crate::board::{BoardState, Coord, MAX_SIZE, Mark, Puzzle};
use crate::rating::{ALL_RULES, Rating, RuleId};

const MAX: usize = MAX_SIZE as usize;

/// A set of cells: one column bitmask per row.
type CellSet = [u16; MAX];

const EMPTY_SET: CellSet = [0; MAX];

/// One of the three families of "exactly one queen goes here" constraints.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Rows,
    Columns,
    Regions,
}

/// Every family, in a fixed order so deductions are reproducible.
pub const ALL_FAMILIES: [Family; 3] = [Family::Rows, Family::Columns, Family::Regions];

impl Family {
    const fn singular(self) -> &'static str {
        match self {
            Family::Rows => "row",
            Family::Columns => "column",
            Family::Regions => "region",
        }
    }
}

/// A single constraint: one row, one column or one region.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Unit {
    pub family: Family,
    pub index: u8,
}

impl Unit {
    pub const fn row(index: u8) -> Self {
        Self {
            family: Family::Rows,
            index,
        }
    }
    pub const fn column(index: u8) -> Self {
        Self {
            family: Family::Columns,
            index,
        }
    }
    pub const fn region(index: u8) -> Self {
        Self {
            family: Family::Regions,
            index,
        }
    }

    /// Human-readable, 1-based: "row 3", "region 5".
    pub fn label(self) -> String {
        format!("{} {}", self.family.singular(), self.index + 1)
    }
}

/// What a deduction concluded.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Action {
    /// A queen belongs here.
    Place(Coord),
    /// These cells cannot hold a queen.
    Eliminate(Vec<Coord>),
}

/// One step of a logical solve.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    pub rule: RuleId,
    pub action: Action,
    /// The unit the deduction reasoned from.
    pub subject: Option<Unit>,
    /// The unit the conclusion lands in, for rules that relate two units.
    pub object: Option<Unit>,
}

impl Step {
    /// An explanation phrased for the player.
    pub fn explain(&self) -> String {
        match (&self.action, self.subject, self.object) {
            (Action::Place(cell), Some(unit), _) => format!(
                "{} has only one cell left for its queen: r{}c{}.",
                capitalise(&unit.label()),
                cell.row + 1,
                cell.col + 1
            ),
            (Action::Eliminate(_), Some(subject), Some(object)) => match self.rule {
                RuleId::LineConfinement | RuleId::RegionConfinement => format!(
                    "Every remaining cell of {} lies in {}, so that queen is spoken for and the rest of {} is clear.",
                    subject.label(),
                    object.label(),
                    object.label()
                ),
                RuleId::SetElimination => format!(
                    "A locked set starting at {} uses up {} entirely.",
                    subject.label(),
                    object.label()
                ),
                _ => format!(
                    "{} rules out cells in {}.",
                    capitalise(&subject.label()),
                    object.label()
                ),
            },
            (Action::Eliminate(_), Some(subject), None) => match self.rule {
                RuleId::CommonElimination => format!(
                    "Wherever the queen of {} goes, these cells are ruled out.",
                    subject.label()
                ),
                _ => format!("{} rules these cells out.", capitalise(&subject.label())),
            },
            _ => self.rule.name().to_string(),
        }
    }
}

fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// The outcome of a full logical solve.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Solve {
    /// True when the rules alone finished the board.
    pub solved: bool,
    /// True when the deductions ran into an impossible position, which for a
    /// well-formed puzzle can only happen from a bad starting board.
    pub contradiction: bool,
    /// The deductions made, in order.
    pub steps: Vec<Step>,
    /// How many times each rule fired, indexed by [`RuleId::index`].
    pub rule_counts: [u16; ALL_RULES.len()],
}

impl Solve {
    /// The hardest rule that fired, or `None` if no deduction was needed.
    pub fn hardest_rule(&self) -> Option<RuleId> {
        ALL_RULES
            .into_iter()
            .rev()
            .find(|r| self.rule_counts[r.index()] > 0)
    }

    /// Turns a completed solve into a difficulty rating. `None` when the rules
    /// could not finish the board, which disqualifies the layout.
    pub fn rating(&self) -> Option<Rating> {
        if !self.solved || self.contradiction {
            return None;
        }
        let hardest = self.hardest_rule().unwrap_or(RuleId::Single);
        Some(Rating {
            difficulty: hardest.difficulty(),
            hardest_rule: hardest,
            rule_counts: self.rule_counts,
            steps: self.steps.len().min(u16::MAX as usize) as u16,
        })
    }
}

/// Solves a region layout from an empty board using the deduction rules.
pub fn solve_layout(size: u8, regions: &[u8]) -> Solve {
    let mut grid = Grid::new(size, regions);
    run(&mut grid, true)
}

/// Rates a layout by the hardest deduction it requires. `None` when the rules
/// cannot finish it.
pub fn rate_layout(size: u8, regions: &[u8]) -> Option<Rating> {
    solve_layout(size, regions).rating()
}

/// Rates an already-built puzzle. Mostly useful in tests.
pub fn rate(puzzle: &Puzzle) -> Option<Rating> {
    rate_layout(puzzle.size(), puzzle.regions())
}

/// What the hint button should say.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HintKind {
    /// A queen on the board is not part of the solution. Nothing else can be
    /// deduced until it goes.
    IncorrectQueen(Coord),
    /// A cross sits on a cell the solution needs. Every deduction from here
    /// would build on a false premise, so this has to go first.
    IncorrectCross(Coord),
    /// A queen belongs on this cell.
    Place(Coord),
    /// This cell can be crossed off.
    Eliminate(Coord),
    /// The board is already solved.
    Complete,
    /// Everything deducible has been deduced and the board is still unfinished,
    /// which for a generated puzzle should not happen.
    Stuck,
}

/// A single suggestion for the player.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Hint {
    pub kind: HintKind,
    /// The rule behind the suggestion, if it came from one.
    pub rule: Option<RuleId>,
    /// A sentence to show the player.
    pub message: String,
}

impl Hint {
    /// The cell the UI should draw attention to.
    pub fn cell(&self) -> Option<Coord> {
        match self.kind {
            HintKind::IncorrectQueen(c)
            | HintKind::IncorrectCross(c)
            | HintKind::Place(c)
            | HintKind::Eliminate(c) => Some(c),
            HintKind::Complete | HintKind::Stuck => None,
        }
    }

    /// True when the hint is pointing out a mistake rather than a next step.
    pub fn is_correction(&self) -> bool {
        matches!(
            self.kind,
            HintKind::IncorrectQueen(_) | HintKind::IncorrectCross(_)
        )
    }
}

/// Suggests the next step from the player's current board.
///
/// Mistakes come first: while a queen or a cross contradicts the solution,
/// every deduction past it would rest on a false premise, so the hint names the
/// mistake instead.
///
/// Otherwise the solver starts from *the player's actual position* — their
/// queens placed and their crosses taken as eliminations — so a hint carries on
/// from the work already done rather than repeating the opening move. Crosses
/// are only safe to trust because the step above has just proved that none of
/// them sits on a solution cell; that also guarantees every constraint keeps at
/// least one candidate, so seeding the grid this way cannot manufacture a
/// contradiction.
pub fn next_hint(puzzle: &Puzzle, state: &BoardState) -> Hint {
    if let Some(wrong) = state.queens().find(|&c| !puzzle.is_solution_cell(c)) {
        return Hint {
            kind: HintKind::IncorrectQueen(wrong),
            rule: None,
            message: format!(
                "The queen at r{}c{} is not part of the solution.",
                wrong.row + 1,
                wrong.col + 1
            ),
        };
    }

    let crosses =
        || crate::board::cells_of(puzzle.size()).filter(|&cell| state.get(cell) == Mark::Cross);
    if let Some(wrong) = crosses().find(|&cell| puzzle.is_solution_cell(cell)) {
        return Hint {
            kind: HintKind::IncorrectCross(wrong),
            rule: None,
            message: format!(
                "The cross at r{}c{} is on a cell the solution needs.",
                wrong.row + 1,
                wrong.col + 1
            ),
        };
    }

    if state.queen_count() == usize::from(puzzle.size()) {
        return Hint {
            kind: HintKind::Complete,
            rule: None,
            message: "The board is solved.".to_string(),
        };
    }

    let mut grid = Grid::new(puzzle.size(), puzzle.regions());
    for queen in state.queens() {
        grid.place(queen);
    }
    // Take the player's crosses as read, so the rules pick up where they left
    // off instead of re-deriving what is already on the board.
    let crossed: Vec<Coord> = crosses().collect();
    grid.eliminate(&crossed);
    debug_assert!(
        !grid.contradiction,
        "crosses clear of the solution cannot contradict"
    );

    // Prefer the most basic observation: an empty cell that a queen already on
    // the board rules out.
    for cell in crate::board::cells_of(puzzle.size()) {
        if state.get(cell) != Mark::Empty || grid.is_candidate(cell) || grid.placed_at(cell) {
            continue;
        }
        if let Some(queen) = state.queens().find(|&q| grid.rules_out(q, cell)) {
            return Hint {
                kind: HintKind::Eliminate(cell),
                rule: Some(RuleId::Propagate),
                message: format!(
                    "The queen at r{}c{} already rules out r{}c{}.",
                    queen.row + 1,
                    queen.col + 1,
                    cell.row + 1,
                    cell.col + 1
                ),
            };
        }
    }

    match next_step(&grid, true) {
        Some(step) => {
            let message = step.explain();
            let kind = match &step.action {
                Action::Place(cell) => HintKind::Place(*cell),
                Action::Eliminate(cells) => HintKind::Eliminate(cells[0]),
            };
            Hint {
                kind,
                rule: Some(step.rule),
                message,
            }
        }
        None => Hint {
            kind: HintKind::Stuck,
            rule: None,
            message: "No further deduction found from this position.".to_string(),
        },
    }
}

/// Applies rules until the board is solved, contradicts itself, or nothing
/// more can be deduced.
fn run(grid: &mut Grid, allow_contradiction: bool) -> Solve {
    let mut steps = Vec::new();
    let mut rule_counts = [0u16; ALL_RULES.len()];

    while !grid.contradiction && !grid.is_solved() {
        let Some(step) = next_step(grid, allow_contradiction) else {
            break;
        };
        rule_counts[step.rule.index()] = rule_counts[step.rule.index()].saturating_add(1);
        grid.apply(&step.action);
        steps.push(step);
    }

    Solve {
        solved: grid.is_solved() && !grid.contradiction,
        contradiction: grid.contradiction,
        steps,
        rule_counts,
    }
}

/// The cheapest deduction available, or `None` if the rules are exhausted.
fn next_step(grid: &Grid, allow_contradiction: bool) -> Option<Step> {
    rule_single(grid)
        .or_else(|| rule_confinement(grid, RuleId::LineConfinement))
        .or_else(|| rule_confinement(grid, RuleId::RegionConfinement))
        .or_else(|| rule_common_elimination(grid))
        .or_else(|| rule_set_elimination(grid))
        .or_else(|| {
            allow_contradiction
                .then(|| rule_contradiction(grid))
                .flatten()
        })
}

/// A unit with exactly one candidate left must put its queen there.
fn rule_single(grid: &Grid) -> Option<Step> {
    for unit in grid.open_units() {
        let cells = grid.candidates_of(unit);
        if set_len(&cells) == 1 {
            let cell = set_cells(&cells).next()?;
            return Some(Step {
                rule: RuleId::Single,
                action: Action::Place(cell),
                subject: Some(unit),
                object: None,
            });
        }
    }
    None
}

/// When every candidate of one unit lies inside a single unit of another
/// family, that second unit's queen is accounted for, so its remaining cells
/// are clear.
///
/// `rule` selects which family the conclusion lands in: [`RuleId::LineConfinement`]
/// targets rows and columns, [`RuleId::RegionConfinement`] targets regions.
fn rule_confinement(grid: &Grid, rule: RuleId) -> Option<Step> {
    let targets: &[Family] = match rule {
        RuleId::LineConfinement => &[Family::Rows, Family::Columns],
        RuleId::RegionConfinement => &[Family::Regions],
        _ => return None,
    };

    for subject in grid.open_units() {
        let cells = grid.candidates_of(subject);
        if set_len(&cells) < 2 {
            continue;
        }
        for &target_family in targets {
            if target_family == subject.family {
                continue;
            }
            let touched = grid.units_touched(&cells, target_family);
            if touched.count_ones() != 1 {
                continue;
            }
            let object = Unit {
                family: target_family,
                index: touched.trailing_zeros() as u8,
            };
            let removed = set_difference(&grid.candidates_of(object), &cells);
            if set_len(&removed) > 0 {
                return Some(Step {
                    rule,
                    action: Action::Eliminate(set_cells(&removed).collect()),
                    subject: Some(subject),
                    object: Some(object),
                });
            }
        }
    }
    None
}

/// A cell ruled out by *every* candidate of some unit is ruled out outright:
/// that unit's queen has to go somewhere, and each option kills the cell.
fn rule_common_elimination(grid: &Grid) -> Option<Step> {
    for unit in grid.open_units() {
        let cells = grid.candidates_of(unit);
        if set_len(&cells) < 2 {
            continue;
        }
        let mut shared: Option<CellSet> = None;
        for cell in set_cells(&cells) {
            let killed = grid.eliminated_by(cell);
            shared = Some(match shared {
                None => killed,
                Some(acc) => set_intersection(&acc, &killed),
            });
            if shared.is_some_and(|s| set_len(&s) == 0) {
                break;
            }
        }
        let removed = set_intersection(&shared.unwrap_or(EMPTY_SET), &grid.cand);
        if set_len(&removed) > 0 {
            return Some(Step {
                rule: RuleId::CommonElimination,
                action: Action::Eliminate(set_cells(&removed).collect()),
                subject: Some(unit),
                object: None,
            });
        }
    }
    None
}

/// Largest locked set considered. Beyond four the reasoning stops being
/// something a player would plausibly spot.
const MAX_SET_SIZE: u32 = 4;

/// If `k` units of one family can only reach `k` units of another family, those
/// `k` counterpart units are consumed and no other unit may use them.
fn rule_set_elimination(grid: &Grid) -> Option<Step> {
    for subject_family in ALL_FAMILIES {
        for object_family in ALL_FAMILIES {
            if subject_family == object_family {
                continue;
            }
            let subjects: Vec<Unit> = grid
                .open_units()
                .filter(|u| u.family == subject_family)
                .collect();
            if subjects.len() < 2 {
                continue;
            }
            let reach: Vec<u16> = subjects
                .iter()
                .map(|&u| grid.units_touched(&grid.candidates_of(u), object_family))
                .collect();

            for combo in 1u32..(1 << subjects.len()) {
                let k = combo.count_ones();
                if !(2..=MAX_SET_SIZE).contains(&k) {
                    continue;
                }
                let mut union = 0u16;
                for (i, &mask) in reach.iter().enumerate() {
                    if combo & (1 << i) != 0 {
                        union |= mask;
                    }
                }
                if union.count_ones() != k {
                    continue;
                }

                // The counterpart units are now exclusive to this set, so
                // strip them out of every other subject unit.
                let mut removed = EMPTY_SET;
                for (i, &unit) in subjects.iter().enumerate() {
                    if combo & (1 << i) != 0 {
                        continue;
                    }
                    let cells = grid.candidates_of(unit);
                    for object_index in bits(union) {
                        let object = Unit {
                            family: object_family,
                            index: object_index,
                        };
                        set_add(
                            &mut removed,
                            &set_intersection(&cells, &grid.candidates_of(object)),
                        );
                    }
                }
                if set_len(&removed) > 0 {
                    let first = combo.trailing_zeros() as usize;
                    return Some(Step {
                        rule: RuleId::SetElimination,
                        action: Action::Eliminate(set_cells(&removed).collect()),
                        subject: Some(subjects[first]),
                        object: Some(Unit {
                            family: object_family,
                            index: union.trailing_zeros() as u8,
                        }),
                    });
                }
            }
        }
    }
    None
}

/// Last resort: assume a candidate, propagate with every cheaper rule, and
/// eliminate it if the position falls apart.
fn rule_contradiction(grid: &Grid) -> Option<Step> {
    // Try the most constrained units first; a contradiction surfaces faster
    // there, and it is also where a player would look.
    let mut units: Vec<(u32, Unit)> = grid
        .open_units()
        .map(|u| (set_len(&grid.candidates_of(u)), u))
        .collect();
    units.sort_by_key(|(count, _)| *count);

    let mut tried = EMPTY_SET;
    for (_, unit) in units {
        for cell in set_cells(&grid.candidates_of(unit)) {
            if set_contains(&tried, cell) {
                continue;
            }
            set_insert(&mut tried, cell);

            let mut trial = grid.clone();
            trial.place(cell);
            run(&mut trial, false);
            if trial.contradiction {
                return Some(Step {
                    rule: RuleId::Contradiction,
                    action: Action::Eliminate(vec![cell]),
                    subject: Some(unit),
                    object: None,
                });
            }
        }
    }
    None
}

/// Candidate bookkeeping for a board mid-deduction.
#[derive(Clone)]
struct Grid {
    n: usize,
    /// `region_of[row][col]`.
    region_of: [[u8; MAX]; MAX],
    /// `region_mask[region][row]` — the columns of that region in that row.
    region_mask: [[u16; MAX]; MAX],
    /// Remaining candidate columns per row. A placed row holds zero.
    cand: CellSet,
    /// Column of the queen placed in each row, if any.
    placed: [Option<u8>; MAX],
    used_cols: u16,
    used_regions: u16,
    placed_count: usize,
    contradiction: bool,
}

impl Grid {
    fn new(size: u8, regions: &[u8]) -> Self {
        assert!(size <= MAX_SIZE, "board size {size} exceeds {MAX_SIZE}");
        let n = usize::from(size);
        assert_eq!(regions.len(), n * n, "region map has the wrong length");

        let mut region_of = [[0u8; MAX]; MAX];
        let mut region_mask = [[0u16; MAX]; MAX];
        for row in 0..n {
            for col in 0..n {
                let region = regions[row * n + col];
                region_of[row][col] = region;
                region_mask[usize::from(region)][row] |= 1 << col;
            }
        }

        let mut cand = EMPTY_SET;
        let full = (1u16 << n) - 1;
        for row in cand.iter_mut().take(n) {
            *row = full;
        }

        Self {
            n,
            region_of,
            region_mask,
            cand,
            placed: [None; MAX],
            used_cols: 0,
            used_regions: 0,
            placed_count: 0,
            contradiction: false,
        }
    }

    fn is_solved(&self) -> bool {
        self.placed_count == self.n
    }

    fn is_candidate(&self, cell: Coord) -> bool {
        set_contains(&self.cand, cell)
    }

    fn placed_at(&self, cell: Coord) -> bool {
        self.placed[usize::from(cell.row)] == Some(cell.col)
    }

    fn region_at(&self, cell: Coord) -> u8 {
        self.region_of[usize::from(cell.row)][usize::from(cell.col)]
    }

    /// True when a queen on `queen` forbids `cell`.
    fn rules_out(&self, queen: Coord, cell: Coord) -> bool {
        cell != queen
            && (cell.row == queen.row
                || cell.col == queen.col
                || self.region_at(cell) == self.region_at(queen)
                || cell.touches(queen))
    }

    /// The cells a queen on `cell` would rule out, excluding `cell` itself.
    fn eliminated_by(&self, cell: Coord) -> CellSet {
        let mut set = EMPTY_SET;
        let full = (1u16 << self.n) - 1;
        let region = usize::from(self.region_at(cell));

        set[usize::from(cell.row)] = full;
        for (row, cells) in set.iter_mut().enumerate().take(self.n) {
            *cells |= 1 << cell.col;
            *cells |= self.region_mask[region][row];
        }
        for row in cell.row.saturating_sub(1)..=(cell.row + 1).min(self.n as u8 - 1) {
            let low = cell.col.saturating_sub(1);
            let high = (cell.col + 1).min(self.n as u8 - 1);
            for col in low..=high {
                set[usize::from(row)] |= 1 << col;
            }
        }
        set[usize::from(cell.row)] &= !(1 << cell.col);
        set
    }

    /// Places a queen and propagates everything it rules out.
    fn place(&mut self, cell: Coord) {
        let row = usize::from(cell.row);
        if self.placed[row].is_some() {
            self.contradiction = true;
            return;
        }
        if self.used_cols & (1 << cell.col) != 0
            || self.used_regions & (1 << self.region_at(cell)) != 0
        {
            self.contradiction = true;
            return;
        }

        let killed = self.eliminated_by(cell);
        self.placed[row] = Some(cell.col);
        self.used_cols |= 1 << cell.col;
        self.used_regions |= 1 << self.region_at(cell);
        self.placed_count += 1;

        set_remove(&mut self.cand, &killed);
        self.cand[row] = 0;
        self.detect_contradiction();
    }

    fn eliminate(&mut self, cells: &[Coord]) {
        for &cell in cells {
            self.cand[usize::from(cell.row)] &= !(1 << cell.col);
        }
        self.detect_contradiction();
    }

    fn apply(&mut self, action: &Action) {
        match action {
            Action::Place(cell) => self.place(*cell),
            Action::Eliminate(cells) => self.eliminate(cells),
        }
    }

    /// Flags positions that can no longer be completed: some constraint has no
    /// queen and nowhere left to put one.
    fn detect_contradiction(&mut self) {
        let stranded = self
            .open_units()
            .any(|unit| set_len(&self.candidates_of(unit)) == 0);
        if stranded {
            self.contradiction = true;
        }
    }

    /// True when this unit still needs a queen.
    fn is_open(&self, unit: Unit) -> bool {
        match unit.family {
            Family::Rows => self.placed[usize::from(unit.index)].is_none(),
            Family::Columns => self.used_cols & (1 << unit.index) == 0,
            Family::Regions => self.used_regions & (1 << unit.index) == 0,
        }
    }

    /// Every unit still awaiting a queen, in a fixed order.
    fn open_units(&self) -> impl Iterator<Item = Unit> {
        let n = self.n as u8;
        ALL_FAMILIES
            .into_iter()
            .flat_map(move |family| (0..n).map(move |index| Unit { family, index }))
            .filter(|&u| self.is_open(u))
    }

    /// The candidate cells belonging to a unit.
    fn candidates_of(&self, unit: Unit) -> CellSet {
        let mut set = EMPTY_SET;
        match unit.family {
            Family::Rows => set[usize::from(unit.index)] = self.cand[usize::from(unit.index)],
            Family::Columns => {
                let bit = 1u16 << unit.index;
                for (out, cand) in set.iter_mut().zip(&self.cand).take(self.n) {
                    *out = *cand & bit;
                }
            }
            Family::Regions => {
                let mask = &self.region_mask[usize::from(unit.index)];
                for ((out, cand), mask) in set.iter_mut().zip(&self.cand).zip(mask).take(self.n) {
                    *out = *cand & *mask;
                }
            }
        }
        set
    }

    /// Which units of `family` the given cells fall into, as a bitmask.
    fn units_touched(&self, cells: &CellSet, family: Family) -> u16 {
        let mut mask = 0u16;
        match family {
            Family::Rows => {
                for (row, &cols) in cells.iter().enumerate().take(self.n) {
                    if cols != 0 {
                        mask |= 1 << row;
                    }
                }
            }
            Family::Columns => {
                for &cols in cells.iter().take(self.n) {
                    mask |= cols;
                }
            }
            Family::Regions => {
                for cell in set_cells(cells) {
                    mask |= 1 << self.region_at(cell);
                }
            }
        }
        mask
    }
}

// --- CellSet helpers -------------------------------------------------------

fn bits(mask: u16) -> impl Iterator<Item = u8> {
    (0..16u8).filter(move |b| mask & (1 << b) != 0)
}

fn set_cells(set: &CellSet) -> impl Iterator<Item = Coord> + '_ {
    set.iter()
        .enumerate()
        .flat_map(|(row, &cols)| bits(cols).map(move |col| Coord::new(row as u8, col)))
}

fn set_len(set: &CellSet) -> u32 {
    set.iter().map(|m| m.count_ones()).sum()
}

fn set_contains(set: &CellSet, cell: Coord) -> bool {
    set[usize::from(cell.row)] & (1 << cell.col) != 0
}

fn set_insert(set: &mut CellSet, cell: Coord) {
    set[usize::from(cell.row)] |= 1 << cell.col;
}

fn set_add(set: &mut CellSet, other: &CellSet) {
    for (row, extra) in set.iter_mut().zip(other) {
        *row |= *extra;
    }
}

fn set_remove(set: &mut CellSet, other: &CellSet) {
    for (row, gone) in set.iter_mut().zip(other) {
        *row &= !*gone;
    }
}

fn set_intersection(a: &CellSet, b: &CellSet) -> CellSet {
    let mut out = EMPTY_SET;
    for i in 0..MAX {
        out[i] = a[i] & b[i];
    }
    out
}

fn set_difference(a: &CellSet, b: &CellSet) -> CellSet {
    let mut out = EMPTY_SET;
    for i in 0..MAX {
        out[i] = a[i] & !b[i];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Mark;
    use crate::rating::Difficulty;
    use crate::solver;
    use crate::test_support::{ascii_regions, puzzle_from_ascii};

    /// Every deduction must agree with the layout's unique solution: a
    /// placement must be a solution cell, an elimination must not be one.
    /// Both the ratings and the hints rest on this.
    fn assert_deductions_are_sound(size: u8, regions: &[u8], solution: &[u8]) {
        let is_solution = |cell: Coord| solution[usize::from(cell.row)] == cell.col;
        let mut grid = Grid::new(size, regions);
        while let Some(step) = next_step(&grid, true) {
            match &step.action {
                Action::Place(cell) => assert!(
                    is_solution(*cell),
                    "{:?} placed a queen at {:?}, which is not in the solution",
                    step.rule,
                    cell
                ),
                Action::Eliminate(cells) => {
                    for cell in cells {
                        assert!(
                            !is_solution(*cell),
                            "{:?} eliminated {:?}, which IS in the solution",
                            step.rule,
                            cell
                        );
                    }
                }
            }
            grid.apply(&step.action);
            assert!(!grid.contradiction, "sound deductions cannot contradict");
        }
    }

    #[test]
    fn solves_a_simple_layout_with_singles_alone() {
        // Each region is a single row, and the columns are forced by adjacency.
        let rows = ["AAAAA", "BBBBB", "CCCCC", "DDDDD", "EEEEE"];
        let (size, regions) = ascii_regions(&rows);
        // Stripes have many solutions, so the logical solver must NOT claim to
        // solve it: an ambiguous layout is not deducible.
        let solve = solve_layout(size, &regions);
        assert!(!solve.solved, "an ambiguous layout must not be 'solved'");
        assert!(solve.rating().is_none());
    }

    #[test]
    fn a_forced_single_is_found_first() {
        // Region A is one cell, so it is an immediate single.
        let rows = ["ABBBB", "CBBBB", "CCDDD", "CCDEE", "CCDEE"];
        let (size, regions) = ascii_regions(&rows);
        let grid = Grid::new(size, &regions);
        let step = rule_single(&grid).expect("the one-cell region is a single");
        assert_eq!(step.rule, RuleId::Single);
        assert_eq!(step.action, Action::Place(Coord::new(0, 0)));
    }

    #[test]
    fn confinement_clears_the_rest_of_the_line() {
        // Region A occupies only row 0, so no other region may use row 0.
        let rows = ["AAABB", "CCCBB", "CCCBB", "DDEEB", "DDEEB"];
        let (size, regions) = ascii_regions(&rows);
        let grid = Grid::new(size, &regions);
        let step = rule_confinement(&grid, RuleId::LineConfinement);
        let step = step.expect("region A is confined to row 0");
        assert_eq!(step.rule, RuleId::LineConfinement);
        match step.action {
            Action::Eliminate(cells) => {
                assert!(cells.iter().all(|c| c.row == 0 && c.col >= 3));
            }
            other => panic!("expected eliminations, got {other:?}"),
        }
    }

    #[test]
    fn common_elimination_only_removes_universally_dead_cells() {
        let rows = ["AABBB", "AABBB", "CCCDD", "CCEDD", "CCEED"];
        let (size, regions) = ascii_regions(&rows);
        let grid = Grid::new(size, &regions);
        if let Some(step) = rule_common_elimination(&grid) {
            let Action::Eliminate(cells) = &step.action else {
                panic!("this rule only eliminates");
            };
            let subject = step
                .subject
                .expect("common elimination reasons from a unit");
            let candidates = grid.candidates_of(subject);
            for &dead in cells {
                for candidate in set_cells(&candidates) {
                    assert!(
                        grid.rules_out(candidate, dead),
                        "{candidate:?} does not rule out {dead:?}, so the elimination is unsound"
                    );
                }
            }
        }
    }

    /// Across generated puzzles at every size and difficulty, no rule ever
    /// concludes something false.
    #[test]
    fn deductions_are_sound_on_generated_puzzles() {
        use crate::generator::generate;
        use crate::rating::ALL_DIFFICULTIES;
        use crate::seed::PuzzleSeed;

        for size in [5u8, 7, 8, 9, 11] {
            for difficulty in ALL_DIFFICULTIES {
                for seed in 0..6u64 {
                    let puzzle = generate(PuzzleSeed::new(size, difficulty, seed * 104_729 + 17));
                    assert_deductions_are_sound(puzzle.size(), puzzle.regions(), puzzle.solution());
                }
            }
        }
    }

    #[test]
    fn contradiction_never_eliminates_a_solution_cell() {
        let rows = ["AABBB", "AABBB", "CCCDD", "CCEDD", "CCEED"];
        let (size, regions) = ascii_regions(&rows);
        let solution = solver::solve_first(size, &regions).unwrap();
        let grid = Grid::new(size, &regions);
        if let Some(step) = rule_contradiction(&grid) {
            let Action::Eliminate(cells) = &step.action else {
                panic!("contradiction only eliminates");
            };
            for cell in cells {
                assert_ne!(
                    solution[usize::from(cell.row)],
                    cell.col,
                    "contradiction wrongly eliminated a solution cell"
                );
            }
        }
    }

    #[test]
    fn rating_reflects_the_hardest_rule_used() {
        let rows = ["ABBBB", "CBBBB", "CCDDD", "CCDEE", "CCDEE"];
        let (size, regions) = ascii_regions(&rows);
        if let Some(rating) = rate_layout(size, &regions) {
            assert_eq!(rating.difficulty, rating.hardest_rule.difficulty());
            assert!(rating.steps > 0);
            for rule in ALL_RULES {
                if rating.count(rule) > 0 {
                    assert!(
                        rule <= rating.hardest_rule,
                        "{rule:?} exceeds the hardest rule"
                    );
                }
            }
        }
    }

    #[test]
    fn hint_flags_a_queen_that_is_not_in_the_solution() {
        let puzzle = puzzle_from_ascii(&["AABBB", "AABBB", "CCCDD", "CCEDD", "CCEED"]);
        let mut state = BoardState::new(puzzle.size());
        let wrong = puzzle
            .cells()
            .find(|&c| !puzzle.is_solution_cell(c))
            .unwrap();
        state.set(wrong, Mark::Queen);
        let hint = next_hint(&puzzle, &state);
        assert_eq!(hint.kind, HintKind::IncorrectQueen(wrong));
    }

    #[test]
    fn hint_reports_a_solved_board_as_complete() {
        let puzzle = puzzle_from_ascii(&["AABBB", "AABBB", "CCCDD", "CCEDD", "CCEED"]);
        let mut state = BoardState::new(puzzle.size());
        for cell in puzzle.solution_cells() {
            state.set(cell, Mark::Queen);
        }
        assert_eq!(next_hint(&puzzle, &state).kind, HintKind::Complete);
    }

    #[test]
    fn hint_points_at_a_cell_a_placed_queen_already_rules_out() {
        let puzzle = puzzle_from_ascii(&["AABBB", "AABBB", "CCCDD", "CCEDD", "CCEED"]);
        let mut state = BoardState::new(puzzle.size());
        let queen = puzzle.solution_cells().next().unwrap();
        state.set(queen, Mark::Queen);

        let hint = next_hint(&puzzle, &state);
        assert_eq!(hint.rule, Some(RuleId::Propagate));
        let cell = hint.cell().expect("propagation hints name a cell");
        assert!(
            queen.row == cell.row
                || queen.col == cell.col
                || puzzle.region_at(queen) == puzzle.region_at(cell)
                || queen.touches(cell)
        );
    }

    /// A hint has to advance the position the player is actually in. Seeding
    /// the grid from their queens alone would leave a board covered in correct
    /// crosses giving the same suggestion as an untouched one.
    #[test]
    fn hint_builds_on_the_players_crosses() {
        use crate::generator::generate;
        use crate::seed::PuzzleSeed;

        let puzzle = generate(PuzzleSeed::new(8, Difficulty::Medium, 4321));
        let opening = next_hint(&puzzle, &BoardState::new(puzzle.size()));

        // Take the opening hint, then take it again and again, applying each
        // one. A hint that ignored the board would hand back the same cell for
        // ever; one that reads the board keeps finding new ground.
        let mut state = BoardState::new(puzzle.size());
        let mut seen = Vec::new();
        for _ in 0..6 {
            let hint = next_hint(&puzzle, &state);
            let Some(cell) = hint.cell() else { break };
            assert!(
                !seen.contains(&cell),
                "hint repeated {cell:?} after it was already acted on: {}",
                hint.message
            );
            seen.push(cell);
            match hint.kind {
                HintKind::Place(cell) => state.set(cell, Mark::Queen),
                HintKind::Eliminate(cell) => state.set(cell, Mark::Cross),
                _ => panic!("unexpected hint on a correct board: {hint:?}"),
            }
        }
        assert!(seen.len() > 1, "the hint never moved past its first answer");

        // And the very first suggestion is no longer what it says once that
        // work is done.
        let later = next_hint(&puzzle, &state);
        assert_ne!(
            later.cell(),
            opening.cell(),
            "the hint is still answering the empty board"
        );
    }

    #[test]
    fn hint_flags_a_cross_that_blocks_the_solution() {
        let puzzle = puzzle_from_ascii(&["AABBB", "AABBB", "CCCDD", "CCEDD", "CCEED"]);
        let mut state = BoardState::new(puzzle.size());
        let needed = puzzle.solution_cells().next().unwrap();
        state.set(needed, Mark::Cross);

        let hint = next_hint(&puzzle, &state);
        assert_eq!(hint.kind, HintKind::IncorrectCross(needed));
        assert!(hint.is_correction());
    }

    /// Crosses are only trusted after the wrong-cross check, so the grid they
    /// seed cannot be contradictory. A board crossed down to nothing but its
    /// solution is the extreme case of that.
    #[test]
    fn a_heavily_crossed_correct_board_still_yields_a_sound_hint() {
        use crate::generator::generate;
        use crate::seed::PuzzleSeed;

        let puzzle = generate(PuzzleSeed::new(9, Difficulty::Hard, 99));
        let mut state = BoardState::new(puzzle.size());
        // Cross off every cell that is not part of the solution.
        for cell in puzzle.cells() {
            if !puzzle.is_solution_cell(cell) {
                state.set(cell, Mark::Cross);
            }
        }

        let hint = next_hint(&puzzle, &state);
        match hint.kind {
            // With everything else ruled out, the only move left is to place.
            HintKind::Place(cell) => assert!(puzzle.is_solution_cell(cell)),
            other => panic!("expected a placement, got {other:?}"),
        }
    }

    #[test]
    fn hint_never_suggests_placing_outside_the_solution() {
        let puzzle = puzzle_from_ascii(&["AABBB", "AABBB", "CCCDD", "CCEDD", "CCEED"]);
        let mut state = BoardState::new(puzzle.size());
        // Cross off everything a fully propagated board would cross off, so the
        // propagation shortcut does not fire and a real rule has to run.
        let mut grid = Grid::new(puzzle.size(), puzzle.regions());
        let queen = puzzle.solution_cells().next().unwrap();
        state.set(queen, Mark::Queen);
        grid.place(queen);
        for cell in puzzle.cells() {
            if !grid.is_candidate(cell) && !grid.placed_at(cell) {
                state.set(cell, Mark::Cross);
            }
        }

        let hint = next_hint(&puzzle, &state);
        match hint.kind {
            HintKind::Place(cell) => assert!(puzzle.is_solution_cell(cell)),
            HintKind::Eliminate(cell) => assert!(!puzzle.is_solution_cell(cell)),
            HintKind::Complete | HintKind::Stuck => {}
            HintKind::IncorrectQueen(_) => panic!("the queen placed was correct"),
            // The crosses came from the solver's own propagation, so none of
            // them can be on a solution cell.
            HintKind::IncorrectCross(cell) => panic!("{cell:?} was crossed correctly"),
        }
    }

    #[test]
    fn difficulty_bands_follow_rule_order() {
        assert_eq!(RuleId::Single.difficulty(), Difficulty::Easy);
        assert_eq!(RuleId::LineConfinement.difficulty(), Difficulty::Medium);
        assert_eq!(RuleId::CommonElimination.difficulty(), Difficulty::Medium);
        assert_eq!(RuleId::SetElimination.difficulty(), Difficulty::Hard);
        assert_eq!(RuleId::Contradiction.difficulty(), Difficulty::Expert);
    }
}
