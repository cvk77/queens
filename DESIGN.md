# Design notes

Why the code is shaped the way it is. For what the game does, see
[README.md](README.md); for where things live, [INVENTORY.md](INVENTORY.md).
Contributor commands and rules live in [AGENTS.md](AGENTS.md).

## The puzzle logic knows nothing about Bevy

`queens_core` depends on `serde` and, on wasm, `web-time`. It has no Bevy
dependency. The board, the four constraints, both solvers and the generator
live there; the Bevy app is a shell around that logic.

This is what makes the generator testable at all. Puzzle generation is a search
with statistical properties (how often it succeeds, how long it takes, what
difficulty spread it produces) and those can only be established over hundreds
of samples. `queens_cli` runs the whole thing headlessly in seconds, which no
amount of playing the game would achieve. The split also keeps the compile loop
for the logic under a second, rather than paying Bevy's link time to change a
solver rule.

## Difficulty is measured, not guessed

The obvious implementation of difficulty is board size. It is also wrong: an
11×11 with small tight regions can fall out in a few forced moves, while a 7×7
can need a genuinely hard deduction.

So `logic.rs` is a second solver that works the way a person does. It keeps a
candidate set and applies named rules cheapest-first:

| Rule | What it sees |
|---|---|
| `Propagate` | Eliminate everything a placed queen rules out |
| `Single` | A unit with one candidate left |
| `LineConfinement` | A unit's candidates all lie in one row or column |
| `RegionConfinement` | A unit's candidates all lie in one region |
| `CommonElimination` | A cell ruled out by *every* candidate of some unit |
| `SetElimination` | `k` units confined to `k` counterpart units, which those `k` then own outright |
| `Contradiction` | Assume a candidate, propagate, eliminate on contradiction |

A puzzle's difficulty is the band of the hardest rule the solve required. The
same engine serves hints, which is what guarantees a hint is always a step the
player could have taken.

A rule reasons about *units* — a row, a column, a region — and a hint has to
name them in terms the player can see. Rows and columns are numbered along the
edges of the board, so a number works. A region is only ever a colour, and
nothing on screen numbers one, so `queens_core` takes the names from the caller
([`RegionNames`]) rather than inventing an index: the game passes the colour
names of whichever palette it is drawing with, colourblind or not.

**A hint never names its rule.** `Step::explain` writes out what was seen and
what follows, because the rule names are labels for people who already know the
moves: "a locked set starting at row 3" tells a player who has not met one
neither what a locked set is nor which units make up this one. Naming both
sides in full — *between them, rows 2, 5 and 7 can only reach columns 1, 4 and
8* — is the explanation, and it is also checkable against the board in front of
them. That is why `Step` carries `subjects` and `objects` as vectors: a
one-unit-a-side `Step` could not say what the set was.

The rule names never reach the player. Nothing on the play screen labels a
move, and the message line under the board holds a hint or nothing at all: a
board that announces which step it will need hands the player the shape of the
solve before they have looked at it. `RuleId::name` and
[`RuleId::description`] are for the CLI audit and for this document.

[`RegionNames`]: crates/queens_core/src/logic.rs
[`RuleId::description`]: crates/queens_core/src/rating.rs

**The bands were calibrated against data, not intuition.** The first cut put
`CommonElimination` in Hard, which sounded right and was not: measuring which
rule actually decides each puzzle (the `yield_rates` diagnostic in
`generator.rs`) showed it deciding roughly half of all boards at every size. It
is the everyday move of Queens — "wherever this region's queen goes, that cell
dies" — so it belongs in Medium. Spotting a locked set is the real step up. With
that correction the spread is usable at every size; before it, nearly everything
was "Hard".

A related measurement killed an assumption: region *shape* barely matters.
Sweeping the growth lumpiness from 0 to 16 (`lumpiness_sweep`) hardly shifts
which rule a puzzle needs, because board size dominates. Lumpiness is therefore
documented as a looks knob and randomised for variety, and the rating check is
the only thing that decides difficulty.

## Generation builds forwards, then repairs

The generator invents a legal solution first, then paints `n` regions outward
from the queens by randomised multi-source flood fill. Each region starts on one
queen, so every region ends up with exactly one — which means the sampled
solution is valid by construction. Only two things are left to establish: that
it is the *only* solution, and that its difficulty matches the request.

Uniqueness is the hard part, and the obvious approach does not survive contact
with measurement. Freshly grown regions are ambiguous most of the time: about
one layout in five is uniquely solvable at 5×5, and **effectively none at 8×8**.
Re-rolling until one happens to be unique does not scale at all.

So an ambiguous layout is **repaired**. The solver names a rival solution, and
one of that rival's queens is moved into a neighbouring region:

- The destination region now holds two rival queens, so the rival breaks the
  one-per-region rule and is no longer a solution.
- No queen of the *intended* solution changed region — the moved cell is never
  one of ours, because our queen in that row sits in a different column — so the
  intended solution survives untouched.

Requiring the destination to be orthogonally adjacent keeps it contiguous. If
removing the cell would split its old region, the stranded pieces travel with it
(`try_relocate`), so contiguity holds on both sides. Each repair kills at least one
rival, and the layout converges. This took uniqueness yield from ~0% to 30–97%
depending on size.

Region shapes that come out of repair are irregular in a way pure growth never
produces, which is a pleasant side effect: the boards look hand-drawn.

## Region-size constraints

A one-cell region is a free queen: `Single` places it immediately, with no
reasoning at all. That is a fair leg-up on Easy and a giveaway anywhere else, so
normal generation for Medium and above requires every region to hold at least
two cells. The last-resort exception is described under
[generation budgets and fallbacks](#generation-budgets-and-fallbacks).

Enforcing this was not a one-line change, because **the repair step had been
relying on creating them.** When a region's queen is attached to the rest of it
only through the cell being moved, the old repair happily shrank that region down
to just the queen. Forbidding that froze those moves and 12×12 Medium started
rejecting 36 of every 40 attempts as unrepairable.

The fix is `make_room`: when a move is refused because it would strand a queen,
donate a cell next to that queen from a neighbouring region first, then the move
is legal. The donation is itself a `try_relocate`, so it inherits every
guarantee — the donor stays contiguous, keeps its own queen, and does not fall
below the floor. Afterwards, non-Easy generation needs *fewer* attempts than
before the constraint existed.

Growth gives undersized regions first refusal, repair declines anything that
would breach the floor, and `smallest_region` is the check that actually
enforces the rule.

## Determinism is load-bearing

A saved game stores its `PuzzleSeed` and the player's marks — never the board.
Generation is deterministic, so the puzzle is rebuilt on load. The save also
keeps elapsed time, auto-cross provenance and hints used. Omitting the generated
layout keeps it compact, but ties compatibility to the generation algorithm.

It also makes determinism a hard invariant rather than a nicety. The entire
search consumes the RNG in a fixed order, including the attempts it rejects, and
a test asserts that the same seed yields a byte-identical puzzle.

Two consequences:

- **The RNG is vendored.** SplitMix64 lives in `rng.rs` instead of a
  `rand` dependency. A dependency bump that changed the RNG stream or the
  shuffle algorithm would silently invalidate every save file, and no amount of
  version pinning removes that compatibility obligation.
- **`SAVE_VERSION` tracks the generator, not just the format.** Any change to
  how a seed becomes a puzzle must bump it, or a resume restores the player's
  marks onto a *different* board. Output-preserving refactors do not require a
  bump; compatible new save fields use defaults. See the contributor guide.

Freshly minted seeds are capped at eight digits (`MAX_FRESH_SEED`) so a player
can read one off the screen and type it back in. Any `u64` is still a valid
seed; only the ones the game invents are short.

Statistics compare puzzles of the same size and rated difficulty: larger boards
take longer even within one difficulty band. Old aggregate statistics are ignored;
size-specific records default to empty without invalidating settings or saved games.

## Input: a click and a sweep must not fight

Bevy starts a drag on the **first pixel** of movement while a button is held,
with no distance threshold. Nearly every real click therefore reports a
`DragStart` too, and the two gestures disagree about what a crossed cell becomes
next: a sweep clears it, a click advances it to a queen. Acting on `DragStart`
directly meant a double-click with a pixel of twitch went empty → cross → empty
and never placed anything.

A stroke now records its intent and **touches nothing until the pointer leaves
the cell it began on**, which is the honest definition of a sweep and makes an
in-cell wiggle indistinguishable from a click. When the sweep does reach a second
cell it paints the origin retroactively.

The swallow decision needs no stored flag: Bevy emits `Click` before `DragEnd`,
so the click handler asks the still-live stroke whether it actually painted. An
earlier version kept a sticky boolean, which could outlive a stroke that ended
over a different cell and eat an unrelated click later.

## Undo, and taking a queen back

History is whole-board snapshots, not a move log. At 144 bytes a board the
memory is irrelevant, and it makes undo immune to the replay bugs a log invites —
which matters because auto-cross turns one click into many changes.

Auto-cross also creates an expectation: if placing a queen marks a swathe of
cells, lifting it should take them back, or a mis-click leaves debris to tidy by
hand. `Session` therefore records **which crosses the assist placed** rather than
the player. Removing a queen clears only its own crosses, and only those no
remaining queen still rules out. It is one click, so it is one undo step. The
provenance flags travel in the snapshots and in the save file, so undo and resume
both keep working.

## Sound says what happened, and says it once

Four short recordings, one per thing that can happen to a cell — a tick for a
cross, a brighter one for a queen, a falling one for a mark coming off — and a
fanfare for a solved board. They are `include_bytes!`d in, like the font: the
game has no asset pipeline, and an `assets/` directory would have to be carried
into the macOS bundle, the release archives and the trunk build before a single
cue played. Only the `wav` decoder is compiled in, because that is what these
four files are.

Nothing plays a cue where it happens. A click, a drag that ended as a click, a
sweep and a solve all write a `Sound` message instead, and one system decides
what reaches the speakers. That is what keeps the settings toggle, the balance
between four recordings made at different levels, and the rate limit in one
place rather than at four call sites that each know a little of it.

The rate limit is the reason it is worth the indirection. A sweep marks a cell
every frame or two, and it hands back the cell it started on together with the
one it reached, so several cells can change in a single frame. A cue per cell
is not a run of ticks, it is a buzz. Cues from the board are floored at 45ms
apart, which is slower than a sweep and faster than anything a player does on
purpose.

What stays silent is as deliberate: undo, redo, reset, hints and every menu.
Undo and reset change a swathe of cells at once, so there is no one thing that
just happened for a cue to name. A sound here always means the board changed
under your hand.

## Rendering

The board is a `Display::Grid` of `Node` entities — one per cell — rather than
sprites. Bevy UI gives layout, scaling to any board size and hit-testing for
free, and the menus share the same vocabulary.

Region boundaries come from per-side border widths. Each cell draws a heavy
border on any edge whose neighbour belongs to a different region and a hairline
elsewhere; both cells draw their own, which is exactly the bold divider the
puzzle needs, at no extra entities.

**Marks are drawn from nodes, not glyphs.** The queen is a ringed disc with a
crown built from rotated squares behind a band; the cross is an ASCII `X`.
Neither Bevy's old built-in font nor the Space Grotesk embedded now carries a
chess glyph, so `♛` is tofu regardless of which one is set; the disc and crown
render identically either way. UI text follows the character conventions in
[AGENTS.md](AGENTS.md#coding-conventions). The test in `theme.rs` checks region
names for ASCII, not every user-visible string.

Marks are shown by switching `Node.display` between `None` and `Flex`, not by
toggling `Visibility`: a hidden node still occupies layout space, which knocks
the visible mark off centre.

Pausing conceals every mark. The regions stay, since they are the puzzle itself,
but a stopped clock should not buy free study time.

## The design is flat, and that is a rule, not an omission

Every colour is a solid fill standing for something — a selection, a state, a
region — never a gradient or a shadow doing the work light and shade would do
for a physical object. Shadows and gradients are deliberately left unused:
the colour itself should communicate the distinction.

Type carries the hierarchy that ornament would elsewhere: a screen's own name
(`theme::title`/`theme::hero`) is set big, bold and uppercase because it is the
first thing the screen is, not a caption sitting over the real content. Numbers
that must not jitter in width as their digits change — the clock, the queen
count, a stats row — go through `theme::numeric`, which turns on tabular
figures; nothing here fakes a monospaced grid by eye.

A "disabled" control (`theme::DISABLED`) is a translucent fill rather than a
flat tone borrowed from whatever colour the panel happens to be: a solid colour
that happened to match its container turned the New Game screen's locked size
and difficulty rows invisible during development, which is what pushed the fix
towards "muted against anything behind it" instead of "a fixed dark grey".

**Motion stays inside the idle-redraw budget rather than forcing it open.** A
button's hover/press tint and a hinted cell's breathing outline (`board.rs`)
both animate every `Update`, but `WinitSettings` still only *redraws* on input
or every `IDLE_REDRAW_WAIT`: a continuously-redrawing window was tried and
pegged the CPU for the sake of motion nobody was looking at while it sat idle.
The animations are tuned to still read as smooth at that coarser cadence — the
button transition converges in one or two idle ticks regardless of how long
each one is, and the hint pulse is slow enough that even a chunkier sample rate
looks like breathing rather than steps. A screen used to rise and settle into
place on entering too (`EnterMotion`); it was cut, not for this tension, but
because the motion itself did not earn its keep.

## Generation budgets and fallbacks

Generation runs on `AsyncComputeTaskPool`. Native builds use a worker thread so
long searches leave the loading screen responsive. The browser build does not
enable atomics or worker-thread generation, so expensive searches can block the
page, including its loading animation.

The main search uses fixed attempt budgets, never elapsed time, to keep results
independent of machine speed. It prefers the requested difficulty, then the
closest deductively solvable candidate, showing the returned rating in the HUD.
The share code retains the requested difficulty because that is part of the seed.

There are two further fallbacks in `generator.rs`: a unique layout the deductive
solver could not finish may be returned as Expert with zero rating steps; if no
candidate exists, `last_resort` searches without a total attempt bound. After
500 attempts in that last-resort loop, it relaxes the minimum region area to one.
These paths preserve uniqueness but mean deductive solvability, the requested
band and the non-Easy region-size floor are not unconditional guarantees.

The CLI audit accepts an unrated layout labelled Expert, but still rejects a
region smaller than the requested band's minimum. Keep these limitations visible
when evaluating generation changes; passing an audit does not prove that every
puzzle can be finished by the deductive solver.

## How this is verified

Three layers, because each catches things the others cannot. Commands and
required checks live in [AGENTS.md](AGENTS.md#commands-and-verification).

**Unit tests** carry the invariants.
The most important is
`deductions_are_sound_on_generated_puzzles`: across generated puzzles at every
size and difficulty, no deduction rule ever concludes something false. Others
pin determinism, region structure, the repair step's guarantees, and the
input-gesture state machine.

**The CLI audit** covers what unit tests cannot: statistical health. A `bench`
run generates hundreds of puzzles and re-checks every invariant on each one,
reporting band hit rates, attempt counts, the smallest region and timing
percentiles. This is what caught the uniqueness collapse at 8×8 and the
mis-calibrated difficulty bands.

**The scripted capture** (`QUEENS_CAPTURE=dir`) drives the real app through every
screen, writes a PNG of each, and feeds mouse gestures through Bevy's actual
picking pipeline while asserting what they did to the board. It exits non-zero on
a failed check. Looking at the output caught four bugs that no test would have:
a panic from a duplicate component in a bundle, tofu glyphs, a wrapping counter
and a misaligned table column.

## The browser build is the same game, not a port

The web version is the same binary target compiled for `wasm32-unknown-unknown`,
with target-specific branches rather than a parallel implementation. Besides
time and persistence, `update_check.rs` omits the native network check,
`main.rs` configures a page-owned canvas, and `menu.rs` omits Quit. Native
`ureq` and `dirs` dependencies are excluded from wasm in the app manifest.

Persistence is the interesting one. Rather than making the save layer abstract,
`persistence.rs` keeps one `SaveData` and one RON format and swaps only *where
the bytes go* — a file under `dirs::data_dir()`, or `localStorage`. That means
`SAVE_VERSION` still governs both, and a change to generated output invalidates
both, which is the property that made the version number load-bearing in the
first place. An abstraction with two implementations would have let the two
drift; sharing the serialisation code keeps the format in one place.

`queens_core` gained a wasm-only dependency (`web-time`) and that is not a
breach of the no-Bevy rule. The rule exists so the generator stays testable at
scale without a renderer, and a clock shim does not touch that. It was needed
because `SystemTime::now()` *panics* on this target rather than failing to
compile, so the alternative was a runtime crash on the first randomly seeded
game.

Sound is the one thing the page has to solve rather than the crate. A browser
will not let a page make a sound until it has been interacted with, and cpal
builds the game's `AudioContext` while Bevy is starting up — so it is created
suspended, the `resume()` cpal makes there is refused, and Chrome never revisits
that decision on its own. Nothing on the Rust side can reach that context; it
belongs to cpal, several layers under `bevy_audio`. So `index.html` remembers
every context the page constructs and resumes them on the first gesture. It is
the only place in this project where behaviour lives in the page rather than in
the game, and it is there because that is the only place with a handle on the
problem.

## Known trade-offs

- **Large Easy and Expert boards can take seconds to generate.** Those bands
  are scarce at 12×12. Task execution and fallback limits are described above.
- **A sweep is one undo step per cell.** Dragging fifteen crosses takes fifteen
  undos to reverse.
- **Web and visual checks remain manual.** Native CI does not cover browser
  behaviour or screenshot appearance; see the contributor verification guide.
