# Working on this repository

A Queens puzzle game in Rust on Bevy 0.19. Three crates: `queens_core` (the
puzzle logic, no Bevy), `queens_cli` (`queens-gen`, headless tooling),
`queens_app` (`queens`, the game).

Read [INVENTORY.md](INVENTORY.md) before searching for anything — it maps every
file and indexes tasks to locations. [DESIGN.md](DESIGN.md) explains why the
design is what it is; [README.md](README.md) covers the rules and controls.

## Commands

```sh
cargo test --workspace                      # 125 tests + 1 doctest, 2 ignored
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo play                                  # alias: run with dynamic linking
cargo audit                                 # alias: the generator audit below
cargo run -p queens_app                     # play, statically linked
```

The two aliases live in `.cargo/config.toml`, which also points Windows at
`rust-lld`. Measured here, a touch-one-file rebuild of the statically linked
binary went from ~13s to ~9s on the linker alone, and to ~6s once the dev
profile stopped emitting debuginfo for dependencies. `cargo play` is ~4s and is
the normal way to run the game while working on it.

The toolchain is pinned in `rust-toolchain.toml`, so CI and a development
machine agree. Bumping it is the `channel` line; rustup fetches the rest.

A release build of `queens_app` takes ~13 minutes (thin LTO, one codegen unit).
Use a debug build unless you are producing artifacts.

The game also targets the browser, which is how it ships on itch.io:

```sh
cargo clippy -p queens_app --target wasm32-unknown-unknown -- -D warnings
cd crates/queens_app && trunk build --release   # dist/, ready to zip for itch
```

**Both targets have to build.** The web build is a set of `cfg`s on one
codebase, not a fork, so a change that only compiles for the desktop is a
broken change. See "The web build" below.

## Invariants that must not break

**Generation is deterministic.** The same `PuzzleSeed` must always yield a
byte-identical puzzle. Saved games store only the seed and rebuild the board on
load, so this is not a nicety. The whole search — including rejected attempts —
consumes the RNG in a fixed order. Do not add a wall-clock budget or anything
else machine-dependent to the generator.

**Any change to generation bumps `SAVE_VERSION`** (`persistence.rs`). Otherwise
a resume restores the player's marks onto a different board, which is worse than
losing the save.

**A new save field is `#[serde(default)]`, not a version bump.** A bump throws
the file away, and with it the player's whole record; a defaulted field lets an
older file load and start counting from zero. Bump only when an old file would
be *wrong* rather than merely incomplete.

**`queens_core` never depends on Bevy.** It is what makes the generator testable
at scale.

**Every puzzle has exactly one solution**, `n` contiguous regions each holding
one solution queen, and — outside Easy — no region smaller than two cells. If
you touch `generator.rs`, prove it with the CLI audit rather than by inspection.

## Verifying changes

Match the layer to the change:

- **Generator or solver** — run the audit at scale. It has caught real problems
  that unit tests could not:
  ```sh
  cargo run -p queens_cli --release -- bench --sizes 5-12 --difficulties all --count 20
  ```
  Watch the `in band` percentages, `attempts`, `min area` and the timings. It
  exits non-zero if any puzzle fails an invariant.

- **UI or input** — run the scripted capture, which drives the real app through
  every screen and feeds mouse gestures through Bevy's picking pipeline:
  ```sh
  QUEENS_CAPTURE=target/capture cargo run -p queens_app
  ```
  It writes a PNG per screen and exits non-zero on a failed check. **Look at the
  screenshots.** They have caught bugs no assertion did — tofu glyphs, wrapping
  labels, a misaligned column.

- **A bug fix** — confirm the new test fails without the fix. A test that passes
  before and after proves nothing.

## Bevy 0.19 traps

Several of these differ from older Bevy and from what the crate docs imply. All
are confirmed against the vendored source in `~/.cargo/registry`.

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

**No em dashes, middle dots, `♛` or `×` in any user-visible string** — the
queen and its crown are drawn from UI nodes rather than a glyph, and every
region name is asserted plain ASCII in `theme.rs`'s tests. Not a font
limitation any more (the embedded Space Grotesk carries a normal Latin set),
but a deliberate one: Doc comments and Markdown are fine.

## The web build

One codebase, `cfg`d in five places. Each one exists for a reason that is not
obvious from the desktop side:

| Where | Why |
|---|---|
| `rng.rs` `entropy_seed` | `std::time::SystemTime::now()` does not merely lack a clock on `wasm32-unknown-unknown`, it **panics**: std routes the target to its `unsupported` implementation. `web-time` reads the browser clock. This compiles clean and only fails at runtime, on the first "New Game" |
| `persistence.rs` `backend` | `dirs::data_dir()` has no answer in a browser. The RON and `SAVE_VERSION` are identical either side; only the store differs (a file, or `localStorage`) |
| `update_check.rs` | `ureq` pulls `ring`, which needs a C toolchain to build for wasm. Stubbed to "nothing newer", since itch always serves the current build |
| `main.rs` `primary_window` | A page places the canvas; there is no window to give a resolution to |
| `menu.rs` | No Quit: exiting would leave a dead canvas the player cannot get back from |

`ureq` and `dirs` are `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`
so they are not even built for the web. `web-time` and `web-sys` were already
in the lockfile via Bevy, so none of this added a dependency.

**wasm-opt has to be told which wasm features rustc used.** rustc enables
`bulk-memory`, `multivalue`, `mutable-globals`, `nontrapping-fptoint`,
`reference-types` and `sign-ext` by default; wasm-opt rejects input using a
feature it was not told to expect, and the failure is a wall of validator
errors, not a clear message. The list lives in `index.html`'s
`data-wasm-opt-params`. Check it against `rustc --target wasm32-unknown-unknown
--print target-features` after a toolchain bump.

**A browser will not let the page make a sound until it has been clicked.**
cpal builds the `AudioContext` while Bevy is starting up, so it is created
suspended and the `resume()` cpal makes there is refused; Chrome does not
resume it later of its own accord (verified over the DevTools protocol: still
`suspended` after a click, with "The AudioContext was not allowed to start" in
the console). Nothing in Rust can reach that context, so `index.html` wraps the
`AudioContext` constructor, remembers what the page builds and resumes them on
the first pointer, key or touch event. That script is the first place to look
if the browser build goes quiet; the desktop build can tell you nothing about
it.

**Testing the web build in a hidden or backgrounded tab will mislead you.**
Chrome throttles `requestAnimationFrame` to zero there, and Bevy's loop runs on
it, so the clock stops, animations freeze and the autosave timer never fires.
None of that is a bug. Use a real, visible window.

## Sound

Four WAV files in `crates/queens_app/assets/sounds/`, `include_bytes!`d by
`audio.rs`. `bevy/wav` is the only decoder built, so a replacement has to be a
PCM WAV — and Bevy builds rodio's decoder with an `unwrap()` inside, so a file
it cannot read panics the first time that cue plays, not at startup.
`every_cue_decodes_to_audible_samples` is what turns that into a failing test
instead of a crash in someone's game.

Nothing plays a cue directly: write a `Sound` message and let `audio.rs` decide.
The settings toggle, the volume balance between four recordings made at
different levels, and the 45ms floor that keeps a drag-sweep ticking rather
than buzzing all live there, and only there.

## Conventions

- Comments explain **why**, not what. State the invariant or the constraint, not
  the history of how the code came to be that way. No references to past bugs,
  fixes or debugging sessions.
- Every public item carries a doc comment. Non-obvious decisions get a sentence
  of rationale where the code is.
- British spelling in prose and in our own identifiers (`colour`, `colourblind`).
  Bindings that mirror a Bevy type keep its spelling (`BackgroundColor`, and a
  local `color: Color`).
- Tests are named as the property they establish
  (`removing_a_queen_takes_its_auto_crosses_with_it`), not `test_foo`.
- **A hint never names its rule, and never uses solver vocabulary.** It says
  what was seen and what follows, naming rows and columns by their numbers and
  regions by their colour, so the player can check it against the board. "A
  locked set uses up column 5" is what this replaced. `RuleId::name` and
  `RuleId::description` are for the CLI audit and the docs: nothing on the play
  screen names a rule, and the game never says in advance which steps a puzzle
  will need.
- `cargo fmt` and a clippy run with `-D warnings` both pass; CI enforces them.

## Known rough edges

- Easy and Expert at 12×12 are slow to generate (~1–2s median, a tail past 5s).
  Both are inherently scarce. Runs on a background thread with a loading screen.
- A drag-sweep records one undo step per cell.
- CI runs the tests on all three platforms, but the release build and its
  packaging only for a `v*` tag or a `workflow_dispatch`, so a packaging
  mistake surfaces at tag time. Trigger the workflow by hand before tagging.
- The macOS leg signs, notarizes and DMGs `queens.app`
  (`packaging/macos/build_dmg.sh`) and needs six repo secrets:
  `MACOS_CERTIFICATE`, `MACOS_CERTIFICATE_PWD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_API_KEY`, `APPLE_API_KEY_ID`, `APPLE_API_ISSUER`. Without them that
  step fails outright rather than falling back to unsigned; run the script
  locally with no `--identity`/API key args to ad-hoc sign and skip
  notarization instead.
- Nothing runs the scripted capture in CI: it needs a window, so screenshots
  are only ever checked locally.
