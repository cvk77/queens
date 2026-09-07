//! `queens-gen` — headless generation and benchmarking for `queens_core`.
//!
//! The game itself is the slow way to find out whether the generator is
//! healthy. This exercises it directly: it asserts every puzzle is uniquely
//! solvable, reports how often requests land in the band they asked for, and
//! times the search — all without opening a window.
//!
//! ```text
//! queens-gen generate --size 9 --difficulty hard [--seed 42]
//! queens-gen bench [--sizes 5-12] [--difficulties all] [--count 50]
//! ```

use std::fmt::Write as _;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use queens_core::rating::{ALL_DIFFICULTIES, ALL_RULES};
use queens_core::{
    Coord, Difficulty, MAX_SIZE, MIN_SIZE, Puzzle, PuzzleSeed, generator, logic, rng, solver,
};

const USAGE: &str = "\
queens-gen — generate and benchmark Queens puzzles

USAGE:
    queens-gen generate [OPTIONS]     Build one puzzle and print it
    queens-gen bench [OPTIONS]        Generate many and report health

GENERATE OPTIONS:
    --size <5..12>                    Board edge length          [default: 9]
    --difficulty <easy|medium|hard|expert>                       [default: medium]
    --seed <u64>                      Omit for a random seed
    --solution                        Mark solution cells in lowercase

BENCH OPTIONS:
    --sizes <A-B|A,B,C>               Sizes to cover             [default: 5-12]
    --difficulties <all|name,name>    Bands to request           [default: all]
    --count <n>                       Puzzles per size/band      [default: 25]
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("generate") => run_generate(&args[1..]),
        Some("bench") => run_bench(&args[1..]),
        Some("-h") | Some("--help") | Some("help") | None => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some(other) => Err(format!("unknown command {other:?}\n\n{USAGE}")),
    };

    match result {
        Ok(healthy) => {
            if healthy {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

// --- generate --------------------------------------------------------------

fn run_generate(args: &[String]) -> Result<bool, String> {
    let opts = Options::parse(args)?;
    let size = opts
        .number("--size", 9)?
        .clamp(u64::from(MIN_SIZE), u64::from(MAX_SIZE)) as u8;
    let difficulty = opts.difficulty("--difficulty", Difficulty::Medium)?;
    let seed = match opts.number_opt("--seed")? {
        Some(seed) => seed,
        None => rng::entropy_seed(),
    };
    let show_solution = opts.flag("--solution");
    opts.finish()?;

    let start = Instant::now();
    let (puzzle, stats) = generator::generate_with_stats(PuzzleSeed::new(size, difficulty, seed));
    let elapsed = start.elapsed();

    println!("{}", render(&puzzle, show_solution));
    println!("seed        {seed}");
    println!("size        {}x{}", puzzle.size(), puzzle.size());
    println!(
        "requested   {difficulty}{}",
        if stats.hit_target {
            String::new()
        } else {
            format!("  (not reached — returned {})", puzzle.rating().difficulty)
        }
    );

    let rating = puzzle.rating();
    println!(
        "rating      {} — hardest rule: {}",
        rating.difficulty,
        rating.hardest_rule.name()
    );
    println!("steps       {}", rating.steps);
    let mut breakdown = String::new();
    for rule in ALL_RULES {
        if rating.count(rule) > 0 {
            let _ = write!(breakdown, "  {}x{}", rating.count(rule), rule.name());
        }
    }
    println!("rules      {breakdown}");
    println!(
        "areas       {} regions, smallest {} cells",
        puzzle.size(),
        generator::smallest_region(puzzle.size(), puzzle.regions())
    );
    println!(
        "search      {} attempts, {} repairs, {:.1?}",
        stats.attempts, stats.repairs, elapsed
    );
    println!(
        "rejected    {} ambiguous, {} area too small, {} undeducible, {} wrong band",
        stats.not_unique, stats.region_too_small, stats.not_deducible, stats.wrong_band
    );

    // Never hand out a puzzle with more than one answer, even from a dev tool.
    let solutions = solver::count_solutions(puzzle.size(), puzzle.regions(), 5);
    if solutions != 1 {
        eprintln!("FAIL: puzzle has {solutions} solutions");
        return Ok(false);
    }
    Ok(true)
}

/// Renders a board as region letters, optionally lowercasing solution cells.
fn render(puzzle: &Puzzle, show_solution: bool) -> String {
    const LETTERS: &[u8] = b"ABCDEFGHIJKL";
    let mut out = String::new();
    for row in 0..puzzle.size() {
        out.push_str("    ");
        for col in 0..puzzle.size() {
            let cell = Coord::new(row, col);
            let letter = LETTERS[usize::from(puzzle.region_at(cell))] as char;
            let glyph = if show_solution && puzzle.is_solution_cell(cell) {
                letter.to_ascii_lowercase()
            } else {
                letter
            };
            out.push(glyph);
            out.push(' ');
        }
        out.push('\n');
    }
    out
}

// --- bench -----------------------------------------------------------------

fn run_bench(args: &[String]) -> Result<bool, String> {
    let opts = Options::parse(args)?;
    let sizes = opts.sizes("--sizes")?;
    let difficulties = opts.difficulties("--difficulties")?;
    let count = opts.number("--count", 25)? as usize;
    opts.finish()?;

    println!(
        "benchmarking {} puzzles ({} sizes x {} bands x {count})\n",
        sizes.len() * difficulties.len() * count,
        sizes.len(),
        difficulties.len()
    );
    println!(
        "{:>4} {:>8} {:>7} {:>8} {:>8} {:>9} {:>9} {:>9}",
        "size", "band", "in band", "attempts", "min area", "p50", "p99", "max"
    );

    let mut failures = 0usize;
    let mut grand_total = Duration::ZERO;

    for &size in &sizes {
        for &difficulty in &difficulties {
            let mut timings = Vec::with_capacity(count);
            let mut in_band = 0usize;
            let mut attempts = 0u64;
            // Smallest region seen in the batch, to show how tightly the
            // no-single-cell-regions rule is binding.
            let mut min_area = usize::MAX;

            for i in 0..count {
                let seed = PuzzleSeed::new(size, difficulty, mix(size, difficulty, i));
                let start = Instant::now();
                let (puzzle, stats) = generator::generate_with_stats(seed);
                let elapsed = start.elapsed();
                timings.push(elapsed);
                grand_total += elapsed;
                attempts += u64::from(stats.attempts);

                if puzzle.rating().difficulty == difficulty {
                    in_band += 1;
                }
                min_area =
                    min_area.min(generator::smallest_region(puzzle.size(), puzzle.regions()));

                // Every invariant the game relies on, checked on every puzzle.
                if let Err(problem) = audit(&puzzle) {
                    failures += 1;
                    eprintln!(
                        "FAIL size {size} {difficulty} seed {}: {problem}",
                        seed.seed
                    );
                }
            }

            timings.sort_unstable();
            println!(
                "{size:>4} {:>8} {:>6.0}% {:>8.1} {:>8} {:>9.2?} {:>9.2?} {:>9.2?}",
                difficulty.name(),
                100.0 * in_band as f64 / count as f64,
                attempts as f64 / count as f64,
                min_area,
                percentile(&timings, 0.50),
                percentile(&timings, 0.99),
                timings.last().copied().unwrap_or_default(),
            );
        }
    }

    println!("\ntotal generation time {grand_total:.2?}");
    if failures == 0 {
        println!("all puzzles passed the invariant audit");
        Ok(true)
    } else {
        eprintln!("{failures} puzzle(s) FAILED the invariant audit");
        Ok(false)
    }
}

/// Re-checks everything the rest of the game assumes about a generated puzzle.
fn audit(puzzle: &Puzzle) -> Result<(), String> {
    let size = puzzle.size();

    if !generator::regions_are_valid(size, puzzle.regions()) {
        return Err("region map is not n contiguous regions covering the board".into());
    }

    // Difficulty is keyed on the request, not on the rating that came out.
    let requested = puzzle.seed().difficulty;
    let required = requested.min_region_size();
    let smallest = generator::smallest_region(size, puzzle.regions());
    if smallest < required {
        return Err(format!(
            "smallest region holds {smallest} cell(s), below the {required} that {requested} requires"
        ));
    }

    let solutions = solver::count_solutions(size, puzzle.regions(), 5);
    if solutions != 1 {
        return Err(format!("{solutions} solutions, expected exactly 1"));
    }

    match solver::solve_first(size, puzzle.regions()) {
        Some(found) if found == puzzle.solution() => {}
        Some(_) => return Err("stored solution is not the one the solver finds".into()),
        None => return Err("no solution at all".into()),
    }

    match logic::rate_layout(size, puzzle.regions()) {
        Some(rating) if &rating == puzzle.rating() => {}
        Some(_) => return Err("stored rating disagrees with a fresh rating".into()),
        // Only legitimate for the fallback path, which reports Expert.
        None if puzzle.rating().difficulty == Difficulty::Expert => {}
        None => return Err("logic cannot solve it, yet it is not rated Expert".into()),
    }

    let regenerated = generator::generate(puzzle.seed());
    if &regenerated != puzzle {
        return Err("regenerating from the seed produced a different puzzle".into());
    }
    Ok(())
}

/// A distinct seed per (size, band, index), so bench runs are reproducible.
fn mix(size: u8, difficulty: Difficulty, index: usize) -> u64 {
    let raw = u64::from(size) << 40 | (difficulty.index() as u64) << 32 | index as u64;
    rng::Rng::new(raw).next_u64()
}

fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[index]
}

// --- argument parsing ------------------------------------------------------

/// Minimal `--flag value` parsing. A dev tool does not need a CLI framework,
/// and skipping one keeps the workspace's dependency list short.
struct Options {
    values: Vec<(String, Option<String>)>,
    used: std::cell::RefCell<Vec<String>>,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut values = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let key = args[i].clone();
            if !key.starts_with("--") {
                return Err(format!("expected an option, found {key:?}"));
            }
            let value = args
                .get(i + 1)
                .filter(|next| !next.starts_with("--"))
                .cloned();
            i += if value.is_some() { 2 } else { 1 };
            values.push((key, value));
        }
        Ok(Self {
            values,
            used: std::cell::RefCell::new(Vec::new()),
        })
    }

    fn get(&self, key: &str) -> Option<Option<&str>> {
        self.used.borrow_mut().push(key.to_string());
        self.values
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_deref())
    }

    fn flag(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    fn number_opt(&self, key: &str) -> Result<Option<u64>, String> {
        match self.get(key) {
            None => Ok(None),
            Some(None) => Err(format!("{key} needs a value")),
            Some(Some(raw)) => raw
                .parse()
                .map(Some)
                .map_err(|_| format!("{key} expects a number, got {raw:?}")),
        }
    }

    fn number(&self, key: &str, default: u64) -> Result<u64, String> {
        Ok(self.number_opt(key)?.unwrap_or(default))
    }

    fn difficulty(&self, key: &str, default: Difficulty) -> Result<Difficulty, String> {
        match self.get(key) {
            None => Ok(default),
            Some(None) => Err(format!("{key} needs a value")),
            Some(Some(raw)) => {
                Difficulty::parse(raw).ok_or_else(|| format!("unknown difficulty {raw:?}"))
            }
        }
    }

    /// Parses `5-12` or `6,8,9`.
    fn sizes(&self, key: &str) -> Result<Vec<u8>, String> {
        let raw = match self.get(key) {
            None => return Ok((MIN_SIZE..=MAX_SIZE).collect()),
            Some(None) => return Err(format!("{key} needs a value")),
            Some(Some(raw)) => raw,
        };

        let parse_one = |s: &str| -> Result<u8, String> {
            s.trim()
                .parse::<u8>()
                .map_err(|_| format!("{key}: {s:?} is not a size"))
                .and_then(|n| {
                    (MIN_SIZE..=MAX_SIZE)
                        .contains(&n)
                        .then_some(n)
                        .ok_or_else(|| format!("{key}: {n} is outside {MIN_SIZE}..={MAX_SIZE}"))
                })
        };

        if let Some((low, high)) = raw.split_once('-') {
            let (low, high) = (parse_one(low)?, parse_one(high)?);
            if low > high {
                return Err(format!("{key}: {low}-{high} is an empty range"));
            }
            Ok((low..=high).collect())
        } else {
            raw.split(',').map(parse_one).collect()
        }
    }

    fn difficulties(&self, key: &str) -> Result<Vec<Difficulty>, String> {
        match self.get(key) {
            None => Ok(ALL_DIFFICULTIES.to_vec()),
            Some(None) => Err(format!("{key} needs a value")),
            Some(Some("all")) => Ok(ALL_DIFFICULTIES.to_vec()),
            Some(Some(raw)) => raw
                .split(',')
                .map(|name| {
                    Difficulty::parse(name.trim())
                        .ok_or_else(|| format!("unknown difficulty {name:?}"))
                })
                .collect(),
        }
    }

    /// Rejects options that were never looked at, so a typo is not silently
    /// ignored.
    fn finish(&self) -> Result<(), String> {
        let used = self.used.borrow();
        match self.values.iter().find(|(k, _)| !used.contains(k)) {
            Some((key, _)) => Err(format!("unknown option {key:?}\n\n{USAGE}")),
            None => Ok(()),
        }
    }
}
