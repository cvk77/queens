# Working on this repository

A Queens puzzle game in Rust on Bevy 0.19. Three crates: `queens_core` (the
puzzle logic, no Bevy), `queens_cli` (`queens-gen`, headless tooling),
`queens_app` (`queens`, the game).

Read [INVENTORY.md](INVENTORY.md) before searching for anything — it maps every
file and indexes tasks to locations. [DESIGN.md](DESIGN.md) explains why the
design is what it is; [README.md](README.md) covers the rules and controls.

## Commands

```sh
cargo test --workspace                      # 69 tests + 1 doctest
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p queens_app                     # play
cargo run -p queens_app --features dev      # dynamic linking, far faster rebuilds
```

A release build of `queens_app` takes ~13 minutes (thin LTO, one codegen unit).
Use a debug build unless you are producing artifacts.

## Invariants that must not break

**Generation is deterministic.** The same `PuzzleSeed` must always yield a
byte-identical puzzle. Saved games store only the seed and rebuild the board on
load, so this is not a nicety. The whole search — including rejected attempts —
consumes the RNG in a fixed order. Do not add a wall-clock budget or anything
else machine-dependent to the generator.

**Any change to generation bumps `SAVE_VERSION`** (`persistence.rs`). Otherwise
a resume restores the player's marks onto a different board, which is worse than
losing the save.

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
| `Resource` | A subtrait of `Component`; you cannot derive both |
| Observers | `On<Pointer<Click>>`, target via `event.event_target()` |
| State scoping | `DespawnOnExit(state)`, not `StateScoped` |
| Run conditions | `.and_then(..)`; plain `.and(..)` is deprecated |
| `DragStart` | Fires on the **first pixel** of movement — no distance threshold |
| `Click` vs `DragEnd` | On release, `Click` fires **first** |
| Hiding a UI node | Use `Node.display`; a `Visibility::Hidden` node still takes layout space |

**The built-in font is an ASCII subset.** No em dashes, middle dots, `♛` or `×`
in any user-visible string — they render as tofu. The queen and its crown are
drawn from UI nodes for the same reason. Doc comments and Markdown are fine.

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
- `cargo fmt` and a clippy run with `-D warnings` both pass; CI enforces them.

## Known rough edges

- Easy and Expert at 12×12 are slow to generate (~1–2s median, a tail past 5s).
  Both are inherently scarce. Runs on a background thread with a loading screen.
- A drag-sweep records one undo step per cell.
- CI tests on Linux only; the other three platforms are built but not tested.
