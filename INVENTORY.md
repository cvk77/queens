# Inventory

A map of the three-crate workspace. Start with the task index, then the
per-file notes. Crate paths below are relative to `crates/`; bare filenames
are relative to the corresponding crate's `src/` directory. Repository-level
paths in "Everything else" are relative to the repository root.

For contributor commands and validation, see [AGENTS.md](AGENTS.md).

## Where to look

| To change… | Go to |
|---|---|
| The four rules, conflict detection, win condition | `queens_core/src/rules.rs` |
| What a difficulty band means | `queens_core/src/rating.rs` (`RuleId::difficulty`) |
| The smallest allowed region | `queens_core/src/rating.rs` (`Difficulty::min_region_size`) |
| Deduction rules, the sentence a hint is phrased as | `queens_core/src/logic.rs` |
| Puzzle generation, uniqueness repair | `queens_core/src/generator.rs` |
| Uniqueness / solution counting | `queens_core/src/solver.rs` |
| Board sizes, `Coord`, `Mark`, `Puzzle` | `queens_core/src/board.rs` |
| Anything about the RNG or seed length | `queens_core/src/rng.rs` |
| The `queens-gen` CLI and its audit | `queens_cli/src/main.rs` |
| Colours, the embedded font, type scale, buttons, panels | `queens_app/src/theme.rs` |
| Board drawing, cell borders, the queen crown, the hint pulse | `queens_app/src/game/board.rs` |
| Clicks, drags, keyboard shortcuts | `queens_app/src/game/interaction.rs` |
| The sounds, and when each one plays | `queens_app/src/audio.rs` |
| Undo, auto-cross, hint requests, the hint tally | `queens_app/src/session.rs` |
| Top bar, toolbar, timer, hint line | `queens_app/src/game/hud.rs` |
| Pause / victory overlays, win detection, autosave | `queens_app/src/game/mod.rs` |
| Menus, size/difficulty pickers, seed and share code entry | `queens_app/src/menu.rs` |
| The How to Play screen and its hand-drawn example boards | `queens_app/src/howto.rs` |
| Save file, settings, statistics | `queens_app/src/persistence.rs` |
| The loading screen and background generation | `queens_app/src/generation.rs` |
| Screens and sub-states | `queens_app/src/states.rs` |
| The scripted screenshot / gesture run | `queens_app/src/capture.rs` |
| The startup check for a newer release | `queens_app/src/update_check.rs` |

## `queens_core` — the puzzle, with no engine attached

Depends on `serde`, plus `web-time` on wasm. No Bevy dependency.

| File | What is in it |
|---|---|
| `lib.rs` | Module list, re-exports, a doctest showing the basic flow |
| `board.rs` | `Coord` (with `touches`, `neighbours`), `Side`, `Mark`, `Puzzle`, `BoardState`, `MIN_SIZE` = 5, `MAX_SIZE` = 12 |
| `rules.rs` | `violations`, `conflicting_cells`, `is_solved`, `rules_out`, `eliminated_by`, `auto_cross` |
| `rating.rs` | `RuleId` (7 rules, each with a `name` and a `description` for tooling and documentation), `Difficulty` (with a share-code `letter`/`from_letter`), `Rating`, `min_region_size` |
| `solver.rs` | Exhaustive bitmask search: `count_solutions`, `has_unique_solution`, `solve_first`, `find_alternative` |
| `logic.rs` | The deductive solver: `Grid`, the seven rules, `rate_layout`, `next_hint`, `Hint`/`HintKind`, `RegionNames` |
| `generator.rs` | `generate`, `generate_with_stats`, region growth, `repair_towards_uniqueness`, `make_room`, `smallest_region`, `regions_are_valid` |
| `rng.rs` | Vendored SplitMix64 `Rng`, `entropy_seed`, `MAX_FRESH_SEED` |
| `seed.rs` | `PuzzleSeed` — size, requested difficulty, `u64`; `Display`/`parse` encode and decode it as a share code |
| `test_support.rs` | `#[cfg(test)]` only: build boards from ASCII letters |

`Puzzle` is deliberately opaque — built only by `generate`, with invariants
listed on the type. `logic.rs` holds the board as one `u16` candidate mask per
row (`CellSet = [u16; 12]`), sized to the supported maximum of 12.

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

| File | What is in it |
|---|---|
| `main.rs` | `generate` and `bench` subcommands, the invariant `audit`, hand-rolled argument parsing |

`audit` re-checks every invariant on a finished puzzle: contiguous regions, the
region-size floor for its requested band, exactly one solution, the stored
solution matching, rating consistency and byte-identical regeneration from its
seed. An unrated fallback is accepted only when labelled Expert; a region-size
violation still fails the audit. See [DESIGN.md](DESIGN.md#generation-budgets-and-fallbacks).

## `queens_app` — the game

| File | What is in it |
|---|---|
| `main.rs` | `App` setup, plugin registration, the camera, idle-redraw `WinitSettings`, `#![allow(clippy::type_complexity)]` |
| `states.rs` | `AppState` (MainMenu, NewGame, Generating, Playing, Stats, Settings, HowToPlay), `PlayState` sub-state (Active, Paused, Won) and `HowToPlayPage` sub-state (Goal, Touching, Controls, Hints) |
| `theme.rs` | The embedded Space Grotesk font, palette, both region palettes and their colour names, the type scale (`hero`/`title`/`label`/`text`/`numeric`/`subtitle`/`footnote`), `screen`/`panel`/`row`, `menu_button`/`accent_button`/`small_button`, `ButtonTint` and its animated hover/press system, `format_time` |
| `session.rs` | `Session` — the live puzzle, marks, clock, conflicts, undo snapshots, auto-cross provenance, hints used. Also `PuzzleRequest` and `Restore` |
| `persistence.rs` | `SaveData`, `Settings`, `DifficultyStats`, `InProgress`, `SAVE_VERSION`, throttled write-on-change |
| `generation.rs` | `OnEnter(Generating)`: spawns the search on `AsyncComputeTaskPool`, polls it, pulses the loading dots |
| `menu.rs` | Main menu (with the version and copyright line, and an update notice above it when one is available, all pinned to the bottom), New Game (size, difficulty, seed entry, share code entry that locks and dims size/difficulty to it, click-to-focus between the two typed fields), Statistics, Settings; `SeedInput`, `ShareCodeInput`, `FocusedField` |
| `howto.rs` | The How to Play screen: four pages (Goal, No Touching, Controls, Hints and Difficulty) paginated by `HowToPlayPage`, each illustrated with a hand-drawn demo board built from `board::queen_token`/`cross_token` rather than a real `Puzzle` |
| `game/mod.rs` | `GamePlugin`, screen layout, clock, win detection, autosave, pause and victory overlays |
| `game/board.rs` | The grid, its row and column rulers, cell borders, the node-drawn queen and crown, `refresh_board`, the hint's breathing outline; `queen_token`/`cross_token` are the always-shown variants `howto.rs` reuses for its example boards |
| `game/hud.rs` | Top bar (size, difficulty, share-code-that-copies, clock, counter), the fixed-height message line and toolbar; `refresh_hud` |
| `game/interaction.rs` | `PaintStroke` and its sweep threshold, the click/drag observers, keyboard shortcuts matched by the character a key produces (`letter_just_pressed`) rather than its physical position, and the platform's own modifier (`MODIFIER`: Command on macOS, Control elsewhere) |
| `capture.rs` | The scripted run: screenshots every screen (including each How to Play page and a faked update notice) and asserts real pointer gestures |
| `audio.rs` | The four embedded WAV cues, the `Sound` message the rest of the game writes, the settings gate and the retrigger floor that keeps a sweep ticking rather than buzzing |
| `update_check.rs` | `UpdateCheckPlugin`: a once-at-startup, best-effort GitHub check for a newer release, run on `AsyncComputeTaskPool`; `LatestRelease` is `None` unless one is found |

### How a game starts

```
MainMenu  ──New Game──▶  NewGame  ──Start──▶  Generating  ──▶  Playing
    └──────Continue───────────────────────────────┘              │
                                                      Active ◀─▶ Paused
                                                          └──▶ Won
```

`PuzzleRequest` is inserted before entering `Generating`; `generation.rs` builds
the puzzle on `AsyncComputeTaskPool` and inserts `Session` before entering
`Playing`. Native builds use a worker thread; the browser limitation is covered
in [DESIGN.md](DESIGN.md#generation-budgets-and-fallbacks).
Every screen's root node carries `DespawnOnExit(state)`, so teardown is
automatic.

### Resources

| Resource | Lives in | Exists when |
|---|---|---|
| `SaveData` | `persistence.rs` | Always, from startup |
| `PuzzleRequest` | `session.rs` | From the moment a game is asked for |
| `Session` | `session.rs` | Only during `Playing` — gate systems on `resource_exists::<Session>` |
| `PaintStroke` | `game/interaction.rs` | Always |
| `Cues` | `audio.rs` | Always, from the first frame |
| `SeedInput` | `menu.rs` | Always |
| `ShareCodeInput` | `menu.rs` | Always |
| `FocusedField` | `menu.rs` | Always |

## Tests

| Where | Covers |
|---|---|
| `queens_core/src/logic.rs` | Rule soundness, rating, every hint path, how a hint is worded |
| `queens_core/src/generator.rs` | Determinism, uniqueness, region structure, repair, size floor, band hit rate |
| `queens_core/src/rules.rs` | Each violation kind, the diagonal corner case, auto-cross |
| `queens_core/src/solver.rs` | Counting, caps, agreement with brute force |
| `queens_core/src/board.rs` | Geometry, adjacency, mark cycle |
| `queens_core/src/rng.rs` | Reproducibility, uniformity, shuffle |
| `queens_core/src/rating.rs` | Difficulty letter round-tripping, case, an unknown letter |
| `queens_core/src/seed.rs` | Share code round-tripping through `Display`/`parse`, and its rejection cases |
| `queens_app/src/game/interaction.rs` | The gesture state machine, including a tap that drifts off its cell; letter shortcuts matching by produced character rather than physical key, case-insensitively, and never on a non-character key |
| `queens_app/src/session.rs` | Queen removal taking its auto-crosses, undo, when a hint counts |
| `queens_app/src/persistence.rs` | Hints accumulating on a solve, and older saves still loading |
| `queens_app/src/menu.rs` | Seed and share code field parsing, their character caps, focus keeping keystrokes out of the wrong field, settings syncing to a started share code, the copyright line staying ASCII |
| `queens_app/src/game/hud.rs` | The wordiest hint fitting the message slot |
| `queens_app/src/audio.rs` | Every embedded cue decoding, which mark makes which sound, and a sweep ticking rather than buzzing |
| `queens_app/src/theme.rs` | Every region colour having a distinct ASCII name |
| `queens_app/src/update_check.rs` | Version comparison: patch/minor ordering, equal and older versions, and unparseable or non-triple tags never counting as newer |

Two `#[ignore]` diagnostics in `generator.rs` print measurements rather than
assert:

```sh
cargo test -p queens_core --lib --release -- --ignored --nocapture yield_rates
cargo test -p queens_core --lib --release -- --ignored --nocapture lumpiness_sweep
```

## Everything else

| Path | What it is |
|---|---|
| `Cargo.toml` | Workspace: members, shared dependencies, the `missing_docs` lint, dev and release profiles |
| `.cargo/config.toml` | `rust-lld` as the Windows linker, and the `play` and `audit-puzzles` aliases |
| `rust-toolchain.toml` | The pinned toolchain, so CI and a development machine agree |
| `.gitattributes` | LF endings, so a shell script edited on Windows still runs on macOS |
| `.github/dependabot.yml` | Monthly cargo and actions updates, with Bevy grouped into one pull request |
| `crates/queens_app/assets/` | The font and the four WAV cues. Embedded with `include_bytes!`; no external font or sound files need shipping. The web bundle also includes HTML and generated JavaScript |
| `crates/queens_app/index.html` | The web build's page: the canvas the game draws into, the wasm-opt feature flags it will not build without, and the script that resumes the audio context a browser starts suspended |
| `crates/queens_app/Trunk.toml` | `trunk build --release` settings; its `dist/` is the itch.io upload |
| `.github/workflows/ci.yml` | Checks, the generator audit, a 3-platform binary matrix, tag releases; the macOS leg signs, notarizes and DMGs the game |
| `packaging/macos/` | `build_dmg.sh` (bundles, codesigns, DMGs and notarizes `queens.app`; ad-hoc-signs and skips notarization when run locally with no credentials), `Info.plist`, `AppIcon.icns` and the `generate_icon.py` that drew it |
| `README.md` | The game, its rules, how to build and play |
| `DESIGN.md` | Why the code is shaped this way |
| `AGENTS.md` | Shared contributor commands, invariants, conventions and platform notes |
