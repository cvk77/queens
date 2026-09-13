# Repository guidance

## Start here
This is the shared contributor guide for coding agents. Read
[INVENTORY.md](INVENTORY.md) before searching: it maps tasks to source files.
[DESIGN.md](DESIGN.md) owns architectural rationale and known trade-offs;
[README.md](README.md) owns player documentation and installation.
## Workspace
Queens is a Rust 2024 workspace using Bevy 0.19. Use the toolchain pinned in
`rust-toolchain.toml`; consult the manifests for dependency versions.

Keep puzzle logic in `queens_core` without Bevy dependencies, headless tooling
in `queens_cli`, and UI/platform integration in `queens_app`. The detailed file
map lives in [INVENTORY.md](INVENTORY.md).
## Commands and verification
Run from the repository root unless stated otherwise:

```sh
cargo play                                  # development game, dynamic linking
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

Use `cargo fmt --all` to format edits. Prefer development builds for app work;
release linking is expensive. Linux prerequisites are listed in `README.md`.
`cargo audit-puzzles` runs the puzzle generator audit. `cargo audit` remains
available for the RustSec dependency scanner when installed.

Match additional validation to the change:

- Generator or solver: run
  `cargo run -p queens_cli --release --locked -- bench --sizes 5-12 --difficulties all --count 20`.
  Check invariant failures, difficulty-band rates, attempts, minimum region
  areas and timings.
- UI or input: run `QUEENS_CAPTURE=target/capture cargo run -p queens_app` in an
  environment with a window. The script checks real pointer gestures and
  captures screens. Inspect the PNGs for layout, wrapping and glyph problems;
  assertions alone do not cover appearance.
- Cross-platform app changes: keep both native and browser targets building.
  Run `cargo clippy -p queens_app --target wasm32-unknown-unknown --locked -- -D warnings`
  with that target installed. From `crates/queens_app`, use
  `trunk build --release` to validate bundling or `trunk serve --release --open`
  for browser interaction.
- Bug fixes: add a meaningful regression test and confirm it fails without the
  fix. Tests live alongside the relevant code; core ASCII-board helpers are in
  `test_support.rs`.

Report which checks ran and any environmental blockers. Documentation-only
changes do not require building the game.

## Invariants
- Queens is not classic N-Queens: exactly one queen per row, column and region,
  with no touching queens, including diagonally. Longer diagonals are legal.
- Preserve a unique solution and `n` contiguous regions, each containing one
  solution queen. Normal generation requires at least two cells per region for
  non-Easy requests and rates difficulty by deductions, not size. The existing
  fallback exceptions are documented in [DESIGN.md](DESIGN.md#generation-budgets-and-fallbacks);
  do not mistake them for guarantees or silently broaden them.
- The same `PuzzleSeed` must regenerate a byte-identical puzzle. Preserve RNG
  consumption order, including rejected attempts; never introduce wall-clock
  budgets or machine-dependent choices into generation.
- Changes to how a seed becomes a puzzle require bumping `SAVE_VERSION` in
  the app's `persistence.rs`, because resumed games rebuild their board from the
  seed. Refactoring that preserves generated output does not invalidate saves.
- Add compatible save fields with `#[serde(default)]`. Bump the save version
  only when existing data would be incorrect, not merely incomplete: a version
  bump discards the player's saved record.
- Preserve native/browser separation and target-specific dependencies; see
  [DESIGN.md](DESIGN.md#the-browser-build-is-the-same-game-not-a-port).

## Coding conventions
- Document every public item; workspace lints enforce missing documentation.
- Explain constraints and rationale in comments, not edit history or debugging
  stories. Use British spelling in prose and project identifiers, while keeping
  upstream API spellings.
- Name tests after the property they establish, rather than `test_foo`.
- Reuse theme helpers and existing Bevy patterns. Check the Bevy notes below before
  changing bundles, observers, gesture handling or text layout: older examples
  can use incompatible APIs.
- Keep em dashes, middle dots, queen glyphs and multiplication signs out of
  user-visible strings. Queen tokens are drawn with UI nodes. Markdown and doc
  comments are exempt.
- Hints explain what was observed and what follows, identifying numbered rows
  and columns and regions by colour. Do not expose rule names or solver jargon
  on the play screen.
- Emit `Sound` messages through `audio.rs` rather than playing cues directly.
  Embedded sound replacements must remain decodable PCM WAV files; validate
  them with `every_cue_decodes_to_audible_samples`.

## Bevy 0.19 notes
Repository-specific API and interaction notes; verify against the pinned Bevy
source when upgrading.

| Thing | The 0.19 reality |
|---|---|
| `BorderRadius` | A **field on `Node`**, not a component. `Node { border_radius: .. }` |
| A duplicate component in one bundle | **Panics at spawn.** This is why `accent_button` exists rather than passing a second `ButtonTint` |
| `AppExit` | A `Message`, not an `Event`: `MessageWriter<AppExit>` and `.write()` |
| `TextFont` | `font: FontSource`, `font_size: FontSize::Px(..)` |
| `LineHeight` | Its **own component**, not a `TextFont` field, and not in the prelude: `bevy::text::LineHeight`. Defaults to `RelativeToFont(1.2)` |
| `Resource` | A subtrait of `Component`; you cannot derive both |
| Observers | `On<Pointer<Click>>`, target via `event.event_target()` |
| State scoping | `DespawnOnExit(state)`, not `StateScoped` |
| Run conditions | `.and_then(..)`; plain `.and(..)` is deprecated |
| `DragStart` | Fires on the **first pixel** of movement — no distance threshold |
| `Click` vs `DragEnd` | On release, `Click` fires **first** |
| `Click` and `Release` | Go to the entity hovered **the previous frame**, so a press that drifts onto a neighbour before releasing produces **no `Click` at all**, on either entity. `DragEnd` goes to the entity that was pressed, which makes it the dependable end of a gesture |
| Hiding a UI node | Use `Node.display`; a `Visibility::Hidden` node still takes layout space |
| A game-wide default font | Overwrite `Assets<Font>` at `AssetId::<Font>::default()` (`theme::ThemePlugin`) rather than threading a `Handle<Font>` through every call that builds a `TextFont` — `FontSource::default()` resolves to that same id |

## Browser checks
After a toolchain change, review `index.html`'s `data-wasm-opt-params` against
the wasm features emitted by rustc. After a wasm-bindgen update, keep the
`Trunk.toml` tool pin aligned with the crate version in `Cargo.lock`.

If web audio is silent, inspect the AudioContext gesture-resume script in
`index.html`; see the browser design notes for why it exists. Test in a visible
browser window: background tabs can throttle or suspend animation frames,
affecting the clock, animation and autosave.

## CI and packaging
CI runs native tests and binary builds on Linux, Windows and macOS, plus
formatting and clippy on Linux and a smaller headless generator audit. Browser
checks and scripted screenshot capture are local checks, not current CI jobs.

Release builds and packaging run for `v*` tags, manual `workflow_dispatch`
runs, and eligible pull-request runs carrying the `packaging` label. Only tags
publish releases. For packaging changes, use a manual run or a labelled PR run
to verify artifacts before tagging; `.github/workflows/ci.yml` defines the
actual event triggers.

The macOS packaging job requires `MACOS_CERTIFICATE`, `MACOS_CERTIFICATE_PWD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_API_KEY`, `APPLE_API_KEY_ID` and
`APPLE_API_ISSUER`. For local ad-hoc signing without notarisation, run
`packaging/macos/build_dmg.sh` without identity or API-key arguments.