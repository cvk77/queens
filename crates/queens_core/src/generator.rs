//! Puzzle generation.
//!
//! The generator works forwards rather than backwards: it invents a legal
//! solution first, then paints regions outward from the queens so that each
//! region contains exactly one of them. That construction guarantees the
//! solution is valid, leaving two things to establish — that it is the *only*
//! solution, and that the deductions needed to find it match the requested
//! difficulty.
//!
//! Uniqueness is the hard part. Freshly grown regions are almost always
//! ambiguous on a board of interesting size (roughly one layout in five works
//! at 5x5, and effectively none at 8x8), so re-rolling until one happens to be
//! unique does not scale. Instead an ambiguous layout is *repaired*: the solver
//! names a rival solution, and one of its queens is moved into a neighbouring
//! region, which invalidates the rival while provably leaving the intended
//! solution intact. See [`repair_towards_uniqueness`].
//!
//! Outside Easy, every region must also hold at least two cells (see
//! [`Difficulty::min_region_size`]). A one-cell region places its own queen by
//! the "last cell" rule, with no reasoning required. Growth gives undersized
//! regions first refusal, repair declines any move that would breach the floor
//! and recovers via [`make_room`], and [`smallest_region`] enforces the rule.
//!
//! Everything is driven by a single [`PuzzleSeed`], and the whole search — not
//! just the accepted attempt — consumes the RNG in a fixed order. Saved games
//! store only the seed, so this must stay stable.

use crate::board::{Coord, MAX_SIZE, MIN_SIZE, Puzzle};
use crate::logic;
use crate::rating::{ALL_RULES, Difficulty, Rating, RuleId};
use crate::rng::Rng;
use crate::seed::PuzzleSeed;
use crate::solver;

/// Marker for a cell no region has claimed yet.
const UNASSIGNED: u8 = u8::MAX;

/// How many layouts to try before settling for the closest difficulty found.
const TARGET_ATTEMPTS: u32 = 300;

/// A further budget spent looking for any usable puzzle, if the first pass
/// produced none.
const FALLBACK_ATTEMPTS: u32 = 300;

/// How many rival solutions to knock out of one layout before giving up on it.
const MAX_REPAIRS: u32 = 400;

/// What the generator had to do to satisfy a request.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct GenStats {
    /// Layouts built and examined.
    pub attempts: u32,
    /// Rival solutions knocked out across all attempts.
    pub repairs: u32,
    /// Layouts abandoned because they could not be repaired into uniqueness.
    pub not_unique: u32,
    /// Layouts thrown out for containing a region below the difficulty's
    /// minimum size.
    pub region_too_small: u32,
    /// Layouts thrown out because logic alone could not finish them.
    pub not_deducible: u32,
    /// Layouts that were fine but landed in the wrong difficulty band.
    pub wrong_band: u32,
    /// False when the returned puzzle missed the requested band.
    pub hit_target: bool,
}

/// Generates the puzzle for a seed.
///
/// Always succeeds. If the requested difficulty proves unreachable for the
/// board size, the closest achievable puzzle is returned instead and
/// [`Puzzle::rating`] reports what was actually produced — so the UI can show
/// the player what they really got.
pub fn generate(seed: PuzzleSeed) -> Puzzle {
    generate_with_stats(seed).0
}

/// [`generate`], plus a report on the search. Used by the CLI to benchmark the
/// generator.
pub fn generate_with_stats(seed: PuzzleSeed) -> (Puzzle, GenStats) {
    let size = seed.size.clamp(MIN_SIZE, MAX_SIZE);
    let n = usize::from(size);
    let mut rng = Rng::new(seed.seed);
    let mut stats = GenStats::default();
    // Keyed on what was *asked* for, not on what comes out: a player who chose
    // Medium should not be handed a free queen because the search had to settle
    // for an Easy-rated board.
    let min_region = seed.difficulty.min_region_size();

    // Best puzzle seen so far, keyed by how far its band sits from the target
    // and then by how much deduction it takes — at equal difficulty, a longer
    // solve is the more interesting puzzle.
    let mut best: Option<(usize, u16, Candidate)> = None;
    // A last resort: unique, but beyond the rules we model.
    let mut undeducible: Option<Candidate> = None;

    for attempt in 0..(TARGET_ATTEMPTS + FALLBACK_ATTEMPTS) {
        // Once something usable exists, stop after the target budget.
        if attempt >= TARGET_ATTEMPTS && best.is_some() {
            break;
        }
        stats.attempts += 1;

        let solution = sample_solution(n, &mut rng);
        let lumpiness = random_lumpiness(&mut rng);
        let mut regions = grow_regions(n, &solution, &mut rng, lumpiness, min_region);

        let repairs =
            repair_towards_uniqueness(size, &mut regions, &solution, &mut rng, min_region);
        stats.repairs += repairs.unwrap_or(0);
        if repairs.is_none() {
            stats.not_unique += 1;
            continue;
        }

        // Growth and repair both prefer to respect the minimum but neither can
        // promise it, so this is the check that actually enforces the rule.
        if smallest_region(size, &regions) < min_region {
            stats.region_too_small += 1;
            continue;
        }

        let Some(rating) = logic::rate_layout(size, &regions) else {
            stats.not_deducible += 1;
            undeducible.get_or_insert(Candidate {
                regions: regions.clone(),
                solution: solution.clone(),
                rating: unrated(),
            });
            continue;
        };

        let distance = rating.difficulty.index().abs_diff(seed.difficulty.index());
        if distance == 0 {
            stats.hit_target = true;
            return (Puzzle::new(size, regions, solution, rating, seed), stats);
        }
        stats.wrong_band += 1;

        let steps = rating.steps;
        let better = best
            .as_ref()
            .is_none_or(|(d, s, _)| (distance, u16::MAX - steps) < (*d, u16::MAX - *s));
        if better {
            best = Some((
                distance,
                steps,
                Candidate {
                    regions,
                    solution,
                    rating,
                },
            ));
        }
    }

    let candidate = best
        .map(|(_, _, c)| c)
        .or(undeducible)
        .unwrap_or_else(|| last_resort(size, min_region, &mut rng));

    (
        Puzzle::new(
            size,
            candidate.regions,
            candidate.solution,
            candidate.rating,
            seed,
        ),
        stats,
    )
}

struct Candidate {
    regions: Vec<u8>,
    solution: Vec<u8>,
    rating: Rating,
}

/// Keeps building layouts until one is unique, ignoring difficulty entirely.
///
/// Reached only if several hundred attempts all failed to reach uniqueness,
/// which has not been observed at any supported size. The loop is unbounded on
/// purpose: returning a puzzle with more than one solution would break an
/// invariant the rest of the crate relies on, and each attempt succeeds with
/// high probability, so this terminates quickly.
fn last_resort(size: u8, min_region: usize, rng: &mut Rng) -> Candidate {
    let n = usize::from(size);
    for attempt in 0u32.. {
        // If even this cannot find a layout meeting the minimum, a solvable
        // puzzle with one small region beats no puzzle at all.
        let min_region = if attempt < 500 { min_region } else { 1 };

        let solution = sample_solution(n, rng);
        let mut regions = grow_regions(n, &solution, rng, 3.0, min_region);
        if repair_towards_uniqueness(size, &mut regions, &solution, rng, min_region).is_none() {
            continue;
        }
        if smallest_region(size, &regions) < min_region {
            continue;
        }
        let rating = logic::rate_layout(size, &regions).unwrap_or_else(unrated);
        return Candidate {
            regions,
            solution,
            rating,
        };
    }
    unreachable!("the loop above only exits by returning")
}

/// The rating given to a layout the deduction rules could not finish. Such a
/// puzzle is solvable but needs search, so it is the hardest thing we offer.
fn unrated() -> Rating {
    Rating {
        difficulty: Difficulty::Expert,
        hardest_rule: RuleId::Contradiction,
        rule_counts: [0; ALL_RULES.len()],
        steps: 0,
    }
}

/// Picks a queen column for every row: a permutation in which consecutive rows
/// never sit in adjacent columns.
///
/// A permutation already keeps queens off shared rows and columns, and queens
/// two or more rows apart cannot touch, so this single constraint is the whole
/// of the touching rule.
fn sample_solution(n: usize, rng: &mut Rng) -> Vec<u8> {
    loop {
        if let Some(solution) = try_sample_solution(n, rng) {
            return solution;
        }
    }
}

fn try_sample_solution(n: usize, rng: &mut Rng) -> Option<Vec<u8>> {
    // Generous relative to the real cost; only guards against a pathological
    // shuffle order on the largest boards.
    let mut budget = 20_000u32;
    let mut solution = vec![0u8; n];
    let mut used = vec![false; n];
    let mut order: Vec<u8> = (0..n as u8).collect();

    fn place(
        row: usize,
        n: usize,
        rng: &mut Rng,
        solution: &mut Vec<u8>,
        used: &mut Vec<bool>,
        order: &mut Vec<u8>,
        budget: &mut u32,
    ) -> bool {
        if row == n {
            return true;
        }
        if *budget == 0 {
            return false;
        }
        *budget -= 1;

        rng.shuffle(order);
        let columns = order.clone();
        for col in columns {
            let c = usize::from(col);
            if used[c] {
                continue;
            }
            if row > 0 && solution[row - 1].abs_diff(col) <= 1 {
                continue;
            }
            used[c] = true;
            solution[row] = col;
            if place(row + 1, n, rng, solution, used, order, budget) {
                return true;
            }
            used[c] = false;
        }
        false
    }

    place(0, n, rng, &mut solution, &mut used, &mut order, &mut budget).then_some(solution)
}

/// How uneven region sizes should be: 0 grows every region at the same rate,
/// higher values let some regions outrun others.
///
/// This is a *looks* knob, not a difficulty knob. Sweeping it from 0 to 16 (see
/// the `lumpiness_sweep` diagnostic) barely shifts which rule a puzzle ends up
/// needing — board size dominates that — so it is randomised purely to keep
/// boards from all looking alike, and the rating check decides difficulty.
fn random_lumpiness(rng: &mut Rng) -> f32 {
    rng.unit_f32() * 8.0
}

/// Paints `n` contiguous regions outward from the solution queens.
///
/// Each region starts on one queen and grows by claiming a random cell on its
/// own frontier, so regions stay orthogonally connected and every region ends
/// up holding exactly one queen — which is what makes the sampled solution a
/// legal one.
///
/// Regions short of `min_region` cells get first refusal on every step, so a
/// region cannot be walled in at one cell while its neighbours sprawl. That is
/// a preference, not a guarantee — a region can still run out of frontier
/// early — so the caller checks the result with [`smallest_region`].
fn grow_regions(
    n: usize,
    solution: &[u8],
    rng: &mut Rng,
    lumpiness: f32,
    min_region: usize,
) -> Vec<u8> {
    let size = n as u8;
    let mut regions = vec![UNASSIGNED; n * n];
    let mut frontier: Vec<Vec<usize>> = vec![Vec::new(); n];
    let appetite: Vec<f32> = (0..n).map(|_| 1.0 + rng.unit_f32() * lumpiness).collect();
    // Every region starts out holding its own queen.
    let mut sizes = vec![1usize; n];

    for (row, &col) in solution.iter().enumerate() {
        let seed_cell = Coord::new(row as u8, col);
        regions[flat(n, seed_cell)] = row as u8;
        for neighbour in seed_cell.orthogonal_neighbours(size) {
            frontier[row].push(flat(n, neighbour));
        }
    }

    let mut remaining = n * n - n;
    while remaining > 0 {
        for region in frontier.iter_mut() {
            region.retain(|&i| regions[i] == UNASSIGNED);
        }

        // While any region that can still grow is under the minimum, only such
        // regions are eligible. At least one then has a positive weight, so
        // this cannot stall.
        let feeding_the_hungry = (0..n).any(|g| sizes[g] < min_region && !frontier[g].is_empty());
        let weights: Vec<f32> = (0..n)
            .map(|g| {
                let skip = frontier[g].is_empty() || (feeding_the_hungry && sizes[g] >= min_region);
                if skip { 0.0 } else { appetite[g] }
            })
            .collect();
        // The board is connected and the regions cover it, so while cells
        // remain some region always has a frontier.
        let Some(region) = rng.weighted_index(&weights) else {
            break;
        };

        let pick = rng.below(frontier[region].len());
        let cell_index = frontier[region].swap_remove(pick);
        regions[cell_index] = region as u8;
        sizes[region] += 1;
        remaining -= 1;

        let cell = unflat(n, cell_index);
        for neighbour in cell.orthogonal_neighbours(size) {
            let i = flat(n, neighbour);
            if regions[i] == UNASSIGNED {
                frontier[region].push(i);
            }
        }
    }

    debug_assert!(!regions.contains(&UNASSIGNED), "region growth left holes");
    regions
}

/// Edits `regions` until `solution` is the only way to solve it. Returns the
/// number of rival solutions eliminated, or `None` if the layout got stuck.
///
/// # How a rival is killed
/// Take a rival solution `alt` and a row where it disagrees with `solution`.
/// The cell `x = (row, alt[row])` holds a rival queen but never one of ours —
/// our queen in that row sits in a different column. Move `x` into a
/// neighbouring region `h`:
///
/// - `h` now contains two rival queens, so `alt` breaks the one-per-region
///   rule and is no longer a solution.
/// - No queen of `solution` changed region, since `x` is not one of them, so
///   `solution` still has exactly one queen per region and remains valid.
///
/// Requiring `h` to be orthogonally adjacent to `x` keeps `h` connected. If
/// removing `x` would split its old region, the stranded pieces travel with it
/// — see [`relocate`] — so every region stays contiguous and keeps exactly one
/// queen of `solution`, the invariant the whole crate depends on.
///
/// A move that would shrink the region losing `x` below `min_region` cells is
/// skipped, which is what keeps the no-single-cell-regions rule intact through
/// repair as well as growth.
fn repair_towards_uniqueness(
    size: u8,
    regions: &mut [u8],
    solution: &[u8],
    rng: &mut Rng,
    min_region: usize,
) -> Option<u32> {
    let n = usize::from(size);

    for repairs in 0..MAX_REPAIRS {
        let Some(alt) = solver::find_alternative(size, regions, solution) else {
            return Some(repairs);
        };

        let mut disagreements: Vec<usize> = (0..n).filter(|&r| alt[r] != solution[r]).collect();
        rng.shuffle(&mut disagreements);

        let mut moved = false;
        'search: for row in disagreements {
            let cell = Coord::new(row as u8, alt[row]);
            debug_assert_ne!(
                solution[row], cell.col,
                "the moved cell must not be one of our own queens"
            );
            let from = regions[flat(n, cell)];

            let mut neighbours: Vec<Coord> = cell.orthogonal_neighbours(size).collect();
            rng.shuffle(&mut neighbours);
            for neighbour in neighbours {
                // A neighbour in the same region offers no move at all. This
                // is much the commonest case, and it must not be mistaken for
                // a refusal: calling `make_room` here would churn the layout
                // to no purpose.
                if regions[flat(n, neighbour)] == from {
                    continue;
                }

                if try_move_beside(size, regions, solution, cell, neighbour, min_region) {
                    moved = true;
                    break 'search;
                }

                // A genuine refusal: taking `cell` away would leave its
                // region's queen with too little to stand on. Hand that region
                // another cell next to its queen and try again — the retry
                // cannot fail for the same reason, because the queen now has a
                // second cell of its own.
                if make_room(size, regions, solution, from, min_region, rng)
                    && try_move_beside(size, regions, solution, cell, neighbour, min_region)
                {
                    moved = true;
                    break 'search;
                }
            }
        }

        if !moved {
            return None;
        }
    }
    None
}

/// Moves `cell` into whichever region `neighbour` belongs to, if that is a
/// different region and the move is legal.
///
/// Both regions are looked up fresh, because an earlier [`make_room`] may have
/// moved cells — including `neighbour` itself — between them.
fn try_move_beside(
    size: u8,
    regions: &mut [u8],
    solution: &[u8],
    cell: Coord,
    neighbour: Coord,
    min_region: usize,
) -> bool {
    let n = usize::from(size);
    let from = regions[flat(n, cell)];
    let into = regions[flat(n, neighbour)];
    into != from && try_relocate(size, regions, solution, cell, into, min_region)
}

/// Grows `region` by one cell next to its own solution queen, so a cell can
/// afterwards be moved out without stranding that queen.
///
/// Donating a cell is itself a relocation, so it goes through [`try_relocate`]
/// and inherits every guarantee — the donor stays contiguous, keeps its own
/// queen, and does not fall below the minimum.
fn make_room(
    size: u8,
    regions: &mut [u8],
    solution: &[u8],
    region: u8,
    min_region: usize,
    rng: &mut Rng,
) -> bool {
    let n = usize::from(size);
    let Some(queen) = queen_of_region(size, regions, solution, region) else {
        return false;
    };

    let mut donors: Vec<Coord> = queen
        .orthogonal_neighbours(size)
        .filter(|&cell| regions[flat(n, cell)] != region)
        .collect();
    rng.shuffle(&mut donors);
    donors
        .into_iter()
        .any(|cell| try_relocate(size, regions, solution, cell, region, min_region))
}

/// Moves `cell` into region `into`, dragging along any part of its old region
/// that `cell` was holding together.
///
/// The old region keeps the component containing its own solution queen; every
/// other component would be orphaned, so it travels with `cell`. None of those
/// cells can be a solution queen, because a region holds exactly one and it
/// stays behind — so this shuffles no queen between regions.
///
/// Returns `false`, changing nothing, when the old region would be left with
/// fewer than `min_region` cells.
fn try_relocate(
    size: u8,
    regions: &mut [u8],
    solution: &[u8],
    cell: Coord,
    into: u8,
    min_region: usize,
) -> bool {
    let n = usize::from(size);
    let from = regions[flat(n, cell)];
    let anchor = queen_of_region(size, regions, solution, from)
        .expect("every region holds one solution queen");
    if anchor == cell {
        // `cell` is its region's own solution queen. Moving it would put two
        // queens in one region and leave another with none, so refuse.
        return false;
    }

    // Everything in the old region still reachable from its queen without
    // passing through `cell` stays put.
    let mut stays = vec![false; n * n];
    let removed = flat(n, cell);
    let mut stack = vec![flat(n, anchor)];
    stays[flat(n, anchor)] = true;
    let mut staying = 0;
    while let Some(i) = stack.pop() {
        staying += 1;
        for neighbour in unflat(n, i).orthogonal_neighbours(size) {
            let j = flat(n, neighbour);
            if !stays[j] && j != removed && regions[j] == from {
                stays[j] = true;
                stack.push(j);
            }
        }
    }

    // Whatever stays behind *is* the old region afterwards, so this is its
    // final size.
    if staying < min_region {
        return false;
    }

    for i in 0..n * n {
        if regions[i] == from && !stays[i] {
            regions[i] = into;
        }
    }
    regions[removed] = into;
    true
}

/// The cell of `region` that holds the intended solution's queen.
fn queen_of_region(size: u8, regions: &[u8], solution: &[u8], region: u8) -> Option<Coord> {
    let n = usize::from(size);
    solution.iter().enumerate().find_map(|(row, &col)| {
        let cell = Coord::new(row as u8, col);
        (regions[flat(n, cell)] == region).then_some(cell)
    })
}

fn flat(n: usize, cell: Coord) -> usize {
    usize::from(cell.row) * n + usize::from(cell.col)
}

fn unflat(n: usize, index: usize) -> Coord {
    Coord::new((index / n) as u8, (index % n) as u8)
}

/// The cell count of the smallest region.
///
/// Used to enforce [`Difficulty::min_region_size`]: a one-cell region is a free
/// queen, so only Easy is allowed one.
pub fn smallest_region(size: u8, regions: &[u8]) -> usize {
    let n = usize::from(size);
    let mut counts = vec![0usize; n];
    for &region in regions {
        counts[usize::from(region)] += 1;
    }
    counts.into_iter().min().unwrap_or(0)
}

/// Checks the structural invariants of a region map: `size` regions, every cell
/// claimed, and each region orthogonally connected.
///
/// The generator upholds these by construction; this exists to assert that in
/// tests and to validate layouts arriving from outside.
pub fn regions_are_valid(size: u8, regions: &[u8]) -> bool {
    let n = usize::from(size);
    if regions.len() != n * n || regions.iter().any(|&r| usize::from(r) >= n) {
        return false;
    }

    let mut counts = vec![0usize; n];
    for &region in regions {
        counts[usize::from(region)] += 1;
    }
    if counts.contains(&0) {
        return false;
    }

    (0..n).all(|region| {
        let start = regions
            .iter()
            .position(|&r| usize::from(r) == region)
            .expect("every region has at least one cell");
        let mut seen = vec![false; n * n];
        let mut stack = vec![start];
        seen[start] = true;
        let mut reached = 0;
        while let Some(i) = stack.pop() {
            reached += 1;
            for neighbour in unflat(n, i).orthogonal_neighbours(size) {
                let j = flat(n, neighbour);
                if !seen[j] && usize::from(regions[j]) == region {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        reached == counts[region]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{BoardState, Mark};
    use crate::rating::ALL_DIFFICULTIES;
    use crate::rules;

    fn seeds(count: u64) -> impl Iterator<Item = u64> {
        (0..count).map(|i| i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xABCD)
    }

    /// Sizes worth covering: the two ends of the range plus the sizes the menu
    /// offers as presets.
    const SAMPLE_SIZES: [u8; 5] = [5, 7, 8, 9, 11];

    #[test]
    fn sampled_solutions_obey_the_touching_rule() {
        for size in MIN_SIZE..=MAX_SIZE {
            let n = usize::from(size);
            let mut rng = Rng::new(u64::from(size));
            for _ in 0..50 {
                let solution = sample_solution(n, &mut rng);
                assert_eq!(solution.len(), n);

                let mut sorted = solution.clone();
                sorted.sort_unstable();
                assert_eq!(
                    sorted,
                    (0..size).collect::<Vec<_>>(),
                    "solution is not a permutation"
                );
                for pair in solution.windows(2) {
                    assert!(
                        pair[0].abs_diff(pair[1]) > 1,
                        "queens in consecutive rows touch: {solution:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn grown_regions_are_contiguous_and_hold_one_queen_each() {
        for size in MIN_SIZE..=MAX_SIZE {
            let n = usize::from(size);
            let mut rng = Rng::new(u64::from(size) * 31);
            for _ in 0..20 {
                let solution = sample_solution(n, &mut rng);
                let regions = grow_regions(n, &solution, &mut rng, 4.0, 1);

                assert!(
                    regions_are_valid(size, &regions),
                    "invalid region map for size {size}"
                );
                for (row, &col) in solution.iter().enumerate() {
                    assert_eq!(
                        regions[row * n + usize::from(col)],
                        row as u8,
                        "queen of row {row} is not in its own region"
                    );
                }
            }
        }
    }

    /// The repair step is only sound if it preserves the intended solution and
    /// every structural invariant while it removes rivals.
    #[test]
    fn repair_preserves_the_solution_and_the_region_structure() {
        for size in SAMPLE_SIZES {
            let n = usize::from(size);
            let mut rng = Rng::new(u64::from(size) * 7717);
            for _ in 0..15 {
                let solution = sample_solution(n, &mut rng);
                let mut regions = grow_regions(n, &solution, &mut rng, 3.0, 1);
                let Some(_) = repair_towards_uniqueness(size, &mut regions, &solution, &mut rng, 1)
                else {
                    continue;
                };

                assert!(
                    regions_are_valid(size, &regions),
                    "repair broke the region structure at size {size}"
                );
                for (row, &col) in solution.iter().enumerate() {
                    let region = regions[row * n + usize::from(col)];
                    let queens_in_region = solution
                        .iter()
                        .enumerate()
                        .filter(|&(r, &c)| regions[r * n + usize::from(c)] == region)
                        .count();
                    assert_eq!(
                        queens_in_region, 1,
                        "region {region} holds {queens_in_region} solution queens"
                    );
                }
                assert!(
                    solver::has_unique_solution(size, &regions),
                    "repair reported success but the layout is still ambiguous"
                );
                assert_eq!(
                    solver::solve_first(size, &regions).as_deref(),
                    Some(solution.as_slice()),
                    "the surviving solution is not the intended one"
                );
            }
        }
    }

    /// A one-cell region places its own queen for free, so only Easy may have
    /// one. This is the rule as the player experiences it.
    #[test]
    fn only_easy_puzzles_may_contain_a_single_cell_region() {
        for size in SAMPLE_SIZES {
            for difficulty in ALL_DIFFICULTIES {
                for seed in seeds(6) {
                    let puzzle = generate(PuzzleSeed::new(size, difficulty, seed));
                    let smallest = smallest_region(puzzle.size(), puzzle.regions());
                    assert!(
                        smallest >= difficulty.min_region_size(),
                        "{difficulty} {size}x{size} seed {seed}: smallest region has \
                         {smallest} cell(s), below the minimum of {}",
                        difficulty.min_region_size()
                    );
                }
            }
        }
    }

    /// Repair may only ever refuse a move, never fix an existing small region,
    /// so it has to preserve a minimum that growth already met.
    #[test]
    fn repair_never_shrinks_a_region_below_the_minimum() {
        const MIN: usize = 2;

        for size in SAMPLE_SIZES {
            let n = usize::from(size);
            let mut rng = Rng::new(u64::from(size) * 977);
            let (mut grown_ok, mut grown_small, mut repaired) = (0, 0, 0);

            for _ in 0..40 {
                let solution = sample_solution(n, &mut rng);
                let mut regions = grow_regions(n, &solution, &mut rng, 3.0, MIN);

                if smallest_region(size, &regions) < MIN {
                    grown_small += 1;
                    continue;
                }
                grown_ok += 1;

                if repair_towards_uniqueness(size, &mut regions, &solution, &mut rng, MIN).is_none()
                {
                    continue;
                }
                repaired += 1;
                assert!(
                    smallest_region(size, &regions) >= MIN,
                    "repair left a region below {MIN} cells at size {size}"
                );
            }

            assert!(repaired > 0, "no layout at size {size} reached uniqueness");
            // Growth only prefers the minimum, so the odd miss is expected; a
            // high miss rate would mean the priority pass is not working.
            assert!(
                grown_small * 4 <= grown_ok,
                "growth missed the minimum {grown_small} times against \
                 {grown_ok} successes at size {size}"
            );
        }
    }

    #[test]
    fn generated_puzzles_have_exactly_one_solution() {
        for seed in seeds(20) {
            for size in SAMPLE_SIZES {
                let puzzle = generate(PuzzleSeed::new(size, Difficulty::Medium, seed));
                assert_eq!(
                    solver::count_solutions(puzzle.size(), puzzle.regions(), 5),
                    1,
                    "puzzle from seed {seed} size {size} is not uniquely solvable"
                );
                assert!(
                    regions_are_valid(puzzle.size(), puzzle.regions()),
                    "puzzle from seed {seed} size {size} has a broken region map"
                );
            }
        }
    }

    #[test]
    fn the_stored_solution_solves_the_puzzle() {
        for seed in seeds(20) {
            let puzzle = generate(PuzzleSeed::new(8, Difficulty::Hard, seed));
            let mut board = BoardState::new(puzzle.size());
            for cell in puzzle.solution_cells() {
                board.set(cell, Mark::Queen);
            }
            assert!(
                rules::is_solved(&puzzle, &board),
                "stored solution is illegal for seed {seed}"
            );
        }
    }

    #[test]
    fn generation_is_deterministic() {
        for seed in seeds(10) {
            let request = PuzzleSeed::new(9, Difficulty::Hard, seed);
            assert_eq!(
                generate(request),
                generate(request),
                "same seed produced different puzzles"
            );
        }
    }

    #[test]
    fn different_seeds_produce_different_puzzles() {
        let a = generate(PuzzleSeed::new(8, Difficulty::Medium, 1));
        let b = generate(PuzzleSeed::new(8, Difficulty::Medium, 2));
        assert_ne!(a.regions(), b.regions());
    }

    #[test]
    fn size_is_clamped_to_the_supported_range() {
        assert_eq!(
            generate(PuzzleSeed::new(0, Difficulty::Easy, 1)).size(),
            MIN_SIZE
        );
        assert_eq!(
            generate(PuzzleSeed::new(99, Difficulty::Easy, 1)).size(),
            MAX_SIZE
        );
    }

    #[test]
    fn the_rating_matches_a_fresh_rating_of_the_layout() {
        for seed in seeds(10) {
            let puzzle = generate(PuzzleSeed::new(8, Difficulty::Hard, seed));
            if let Some(rating) = logic::rate(&puzzle) {
                assert_eq!(&rating, puzzle.rating(), "stored rating is stale");
            }
        }
    }

    #[test]
    fn requests_usually_land_in_the_band_they_asked_for() {
        for difficulty in ALL_DIFFICULTIES {
            let hits = seeds(10)
                .filter(|&seed| {
                    generate(PuzzleSeed::new(9, difficulty, seed))
                        .rating()
                        .difficulty
                        == difficulty
                })
                .count();
            assert!(
                hits >= 8,
                "only {hits}/10 seeds produced a {difficulty} puzzle on a 9x9 board"
            );
        }
    }
}

#[cfg(test)]
mod diagnostics {
    use super::*;

    /// Prints how often layouts survive each generator filter, and the spread
    /// of difficulties they land in.
    ///
    /// `cargo test -p queens_core --lib -- --ignored --nocapture yield_rates`
    #[test]
    #[ignore]
    fn yield_rates() {
        for size in [5u8, 7, 8, 9, 11, 12] {
            let n = usize::from(size);
            let mut rng = Rng::new(0xFEED);
            let (mut unique, mut deducible, mut total_repairs) = (0, 0, 0);
            let mut hardest = [0usize; ALL_RULES.len()];
            const TRIES: usize = 120;

            let start = std::time::Instant::now();
            for _ in 0..TRIES {
                let solution = sample_solution(n, &mut rng);
                let mut regions = grow_regions(n, &solution, &mut rng, 3.0, 1);
                let Some(repairs) =
                    repair_towards_uniqueness(size, &mut regions, &solution, &mut rng, 1)
                else {
                    continue;
                };
                unique += 1;
                total_repairs += repairs;
                if let Some(rating) = logic::rate_layout(size, &regions) {
                    deducible += 1;
                    hardest[rating.hardest_rule.index()] += 1;
                }
            }
            println!(
                "size {size:>2}: unique {unique:>3}/{TRIES}  deducible {deducible:>3}  \
                 avg repairs {:>5.1}  {:?}/layout\n          hardest rule {hardest:?}",
                f64::from(total_repairs) / f64::from(unique.max(1)),
                start.elapsed() / TRIES as u32,
            );
        }
    }
}

#[cfg(test)]
mod calibration {
    use super::*;

    /// Sweeps region lumpiness to see how it steers the hardest rule required.
    /// `cargo test -p queens_core --lib --release -- --ignored --nocapture lumpiness_sweep`
    #[test]
    #[ignore]
    fn lumpiness_sweep() {
        for size in [7u8, 9, 11] {
            let n = usize::from(size);
            for lumpiness in [0.0f32, 1.0, 2.0, 4.0, 8.0, 16.0] {
                let mut rng = Rng::new(0xC0FFEE);
                let mut hardest = [0usize; ALL_RULES.len()];
                let mut unique = 0;
                const TRIES: usize = 150;
                for _ in 0..TRIES {
                    let solution = sample_solution(n, &mut rng);
                    let mut regions = grow_regions(n, &solution, &mut rng, lumpiness, 1);
                    if repair_towards_uniqueness(size, &mut regions, &solution, &mut rng, 1)
                        .is_none()
                    {
                        continue;
                    }
                    unique += 1;
                    if let Some(rating) = logic::rate_layout(size, &regions) {
                        hardest[rating.hardest_rule.index()] += 1;
                    }
                }
                println!(
                    "size {size:>2} lumpiness {lumpiness:>4}: unique {unique:>3}/{TRIES}  \
                     single {} line {} region {} common {} set {} contra {}",
                    hardest[1], hardest[2], hardest[3], hardest[4], hardest[5], hardest[6]
                );
            }
            println!();
        }
    }
}
