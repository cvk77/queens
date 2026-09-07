# Inventory

A map of the codebase: 23 source files, ~7,600 lines, three crates. Start with
the task index, then the per-file notes.

## Where to look

| To change… | Go to |
|---|---|
| The four rules, conflict detection, win condition | `queens_core/src/rules.rs` |
| What a difficulty band means | `queens_core/src/rating.rs` (`RuleId::difficulty`) |
| The smallest allowed region | `queens_core/src/rating.rs` (`Difficulty::min_region_size`) |
| Deduction rules, hint text | `queens_core/src/logic.rs` |
| Puzzle generation, uniqueness repair | `queens_core/src/generator.rs` |
| Uniqueness / solution counting | `queens_core/src/solver.rs` |
| Board sizes, `Coord`, `Mark`, `Puzzle` | `queens_core/src/board.rs` |
| Anything about the RNG or seed length | `queens_core/src/rng.rs` |
| The `queens-gen` CLI and its audit | `queens_cli/src/main.rs` |
| Colours, fonts, buttons, panels | `queens_app/src/theme.rs` |
| Board drawing, cell borders, the queen crown | `queens_app/src/game/board.rs` |
| Clicks, drags, keyboard shortcuts | `queens_app/src/game/interaction.rs` |
| Undo, auto-cross, hint requests | `queens_app/src/session.rs` |
| Top bar, toolbar, timer, hint line | `queens_app/src/game/hud.rs` |
| Pause / victory overlays, win detection, autosave | `queens_app/src/game/mod.rs` |
| Menus, size/difficulty pickers, seed entry | `queens_app/src/menu.rs` |
| Save file, settings, statistics | `queens_app/src/persistence.rs` |
| The loading screen and background generation | `queens_app/src/generation.rs` |
| Screens and sub-states | `queens_app/src/states.rs` |
| The scripted screenshot / gesture run | `queens_app/src/capture.rs` |

## `queens_core` — the puzzle, with no engine attached

Only depends on `serde`. 57 tests (2 of them `#[ignore]` diagnostics).

| File | Lines | What is in it |
|---|---|---|
| `lib.rs` | 50 | Module list, re-exports, a doctest showing the basic flow |
| `board.rs` | 358 | `Coord` (with `touches`, `neighbours`), `Side`, `Mark`, `Puzzle`, `BoardState`, `MIN_SIZE` = 5, `MAX_SIZE` = 12 |
| `rules.rs` | 253 | `violations`, `conflicting_cells`, `is_solved`, `rules_out`, `eliminated_by`, `auto_cross` |
| `rating.rs` | 163 | `RuleId` (7 rules), `Difficulty`, `Rating`, `min_region_size` |
| `solver.rs` | 259 | Exhaustive bitmask search: `count_solutions`, `has_unique_solution`, `solve_first`, `find_alternative` |
| `logic.rs` | 1296 | The deductive solver: `Grid`, the seven rules, `rate_layout`, `next_hint`, `Hint`/`HintKind`, `RegionNames` |
| `generator.rs` | 983 | `generate`, `generate_with_stats`, region growth, `repair_towards_uniqueness`, `make_room`, `smallest_region`, `regions_are_valid` |
| `rng.rs` | 179 | Vendored SplitMix64 `Rng`, `entropy_seed`, `MAX_FRESH_SEED` |
| `seed.rs` | 37 | `PuzzleSeed` — size, requested difficulty, `u64` |
| `test_support.rs` | 68 | `#[cfg(test)]` only: build boards from ASCII letters |

`Puzzle` is deliberately opaque — built only by `generate`, with invariants
listed on the type. `logic.rs` holds the board as one `u16` candidate mask per
row (`CellSet = [u16; 12]`), which is why `MAX_SIZE` is 12.

### Notable entry points

```rust
generate(PuzzleSeed::new(9, Difficulty::Hard, 42)) -> Puzzle
rules::is_solved(&puzzle, &board) -> bool
logic::next_hint(&puzzle, &board, RegionNames::numbered()) -> Hint
logic::rate_layout(size, regions) -> Option<Rating>
solver::has_unique_solution(size, regions) -> bool
generator::smallest_region(size, regions) -> usize
```

## `queens_cli` — headless generation and audit

| File | Lines | What is in it |
|---|---|---|
| `main.rs` | 420 | `generate` and `bench` subcommands, the invariant `audit`, hand-rolled argument parsing |

`audit` re-checks every invariant on a finished puzzle: contiguous regions, the
region-size floor for its requested band, exactly one solution, the stored
solution matching, a rating that still holds on a fresh solve, and byte-identical
regeneration from its seed.

## `queens_app` — the game

18 tests.

| File | Lines | What is in it |
|---|---|---|
| `main.rs` | 52 | `App` setup, plugin registration, the camera, `#![allow(clippy::type_complexity)]` |
| `states.rs` | 34 | `AppState` (MainMenu, NewGame, Generating, Playing, Stats, Settings) and `PlayState` sub-state (Active, Paused, Won) |
| `theme.rs` | 354 | Palette, both region palettes and their colour names, `screen`/`panel`/`row`/`text`/`title`, `menu_button`/`accent_button`/`small_button`, `ButtonTint` hover system, `format_time` |
| `session.rs` | 377 | `Session` — the live puzzle, marks, clock, conflicts, undo snapshots, auto-cross provenance. Also `PuzzleRequest` and `Restore` |
| `persistence.rs` | 225 | `SaveData`, `Settings`, `DifficultyStats`, `InProgress`, `SAVE_VERSION`, throttled write-on-change |
| `generation.rs` | 99 | `OnEnter(Generating)`: spawns the search on `AsyncComputeTaskPool`, polls it, animates the ellipsis |
| `menu.rs` | 562 | Main menu, New Game (size, difficulty, seed entry), Statistics, Settings; `SeedInput` |
| `game/mod.rs` | 217 | `GamePlugin`, screen layout, clock, win detection, autosave, pause and victory overlays |
| `game/board.rs` | 425 | The grid, its row and column rulers, cell borders, the node-drawn queen and crown, `refresh_board` |
| `game/hud.rs` | 253 | Top bar (size, difficulty, seed, clock, counter), the fixed-height message line and toolbar; `refresh_hud` |
| `game/interaction.rs` | 344 | `PaintStroke`, the click/drag observers, keyboard shortcuts |
| `capture.rs` | 636 | The scripted run: screenshots every screen and asserts real pointer gestures |

### How a game starts

```
MainMenu  ──New Game──▶  NewGame  ──Start──▶  Generating  ──▶  Playing
    └──────Continue───────────────────────────────┘              │
                                                      Active ◀─▶ Paused
                                                          └──▶ Won
```

`PuzzleRequest` is inserted before entering `Generating`; `generation.rs` builds
the puzzle on a worker thread and inserts `Session` before entering `Playing`.
Every screen's root node carries `DespawnOnExit(state)`, so teardown is
automatic.

### Resources

| Resource | Lives in | Exists when |
|---|---|---|
| `SaveData` | `persistence.rs` | Always, from startup |
| `PuzzleRequest` | `session.rs` | From the moment a game is asked for |
| `Session` | `session.rs` | Only during `Playing` — gate systems on `resource_exists::<Session>` |
| `PaintStroke` | `game/interaction.rs` | Always |
| `SeedInput` | `menu.rs` | Always |

## Tests

| Where | Count | Covers |
|---|---|---|
| `queens_core/src/logic.rs` | 15 | Rule soundness, rating, every hint path |
| `queens_core/src/generator.rs` | 14 | Determinism, uniqueness, region structure, repair, size floor, band hit rate |
| `queens_core/src/rules.rs` | 10 | Each violation kind, the diagonal corner case, auto-cross |
| `queens_core/src/solver.rs` | 6 | Counting, caps, agreement with brute force |
| `queens_core/src/board.rs` | 5 | Geometry, adjacency, mark cycle |
| `queens_core/src/rng.rs` | 5 | Reproducibility, uniformity, shuffle |
| `queens_app/src/game/interaction.rs` | 7 | The gesture state machine |
| `queens_app/src/session.rs` | 5 | Queen removal taking its auto-crosses, undo |
| `queens_app/src/menu.rs` | 4 | Seed field parsing and its digit cap |

Two `#[ignore]` diagnostics in `generator.rs` print measurements rather than
assert:

```sh
cargo test -p queens_core --lib --release -- --ignored --nocapture yield_rates
cargo test -p queens_core --lib --release -- --ignored --nocapture lumpiness_sweep
```

## Everything else

| Path | What it is |
|---|---|
| `Cargo.toml` | Workspace: members, shared dependencies, dev and release profiles |
| `.github/workflows/ci.yml` | Checks, the generator audit, a 4-platform binary matrix, tag releases |
| `README.md` | The game, its rules, how to build and play |
| `DESIGN.md` | Why the code is shaped this way |
| `CLAUDE.md` | Conventions, invariants and version-specific traps |
