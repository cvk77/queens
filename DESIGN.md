# Design notes

Why the code is shaped the way it is. For what the game does, see
[README.md](README.md); for where things live, [INVENTORY.md](INVENTORY.md).

## The puzzle logic knows nothing about Bevy

`queens_core` has one dependency, `serde`. Everything about the game — the
board, the four constraints, both solvers, the generator — lives there, and the
Bevy app is a shell around it.

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
| `SetElimination` | `k` units confined to `k` counterpart units |
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

[`RegionNames`]: crates/queens_core/src/logic.rs

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
(`relocate`), so contiguity holds on both sides. Each repair kills at least one
rival, and the layout converges. This took uniqueness yield from ~0% to 30–97%
depending on size.

Region shapes that come out of repair are irregular in a way pure growth never
produces, which is a pleasant side effect: the boards look hand-drawn.

## No single-cell regions outside Easy

A one-cell region is a free queen: `Single` places it immediately, with no
reasoning at all. That is a fair leg-up on Easy and a giveaway anywhere else, so
Medium and above require every region to hold at least two cells.

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
Generation is deterministic, so the puzzle is rebuilt on load. That keeps the
save file tiny and version-tolerant.

It also makes determinism a hard invariant rather than a nicety. The entire
search consumes the RNG in a fixed order, including the attempts it rejects, and
a test asserts that the same seed yields a byte-identical puzzle.

Two consequences:

- **The RNG is vendored.** A 60-line SplitMix64 lives in `rng.rs` instead of a
  `rand` dependency. A dependency bump that changed the RNG stream or the
  shuffle algorithm would silently invalidate every save file, and no amount of
  version pinning makes that risk worth carrying for the twenty lines involved.
- **`SAVE_VERSION` tracks the generator, not just the format.** Any change to
  how a seed becomes a puzzle must bump it, or a resume restores the player's
  marks onto a *different* board — which is worse than losing the save. It went
  to `2` when the region-size floor landed.

Freshly minted seeds are capped at eight digits (`MAX_FRESH_SEED`) so a player
can read one off the screen and type it back in. Any `u64` is still a valid
seed; only the ones the game invents are short.

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

## Rendering

The board is a CSS grid of `Node` entities — one per cell — rather than sprites.
Bevy UI gives layout, scaling to any board size and hit-testing for free, and the
menus share the same vocabulary.

Region boundaries come from per-side border widths. Each cell draws a heavy
border on any edge whose neighbour belongs to a different region and a hairline
elsewhere; both cells draw their own, which is exactly the bold divider the
puzzle needs, at no extra entities.

**Marks are drawn from nodes, not glyphs.** Bevy's built-in font is an ASCII
subset, so `♛` and `×` render as tofu. The queen is a ringed disc with a crown
built from rotated squares behind a band; the cross is an ASCII `X`. This is also
why no UI string contains an em dash or a middle dot — they appeared as boxes.

Marks are shown by switching `Node.display` between `None` and `Flex`, not by
toggling `Visibility`: a hidden node still occupies layout space, which knocks
the visible mark off centre.

Pausing conceals every mark. The regions stay, since they are the puzzle itself,
but a stopped clock should not buy free study time.

## Generation runs off the main thread

The search is normally a few milliseconds, but a rare request — an Easy 12×12, or
an Expert one — can take seconds, because such puzzles are genuinely scarce.
It runs on `AsyncComputeTaskPool` behind a loading screen, so the window stays
responsive.

A wall-clock budget would be the obvious way to cap the wait, and it is ruled
out: generation must stay deterministic, and a time limit would make the result
depend on machine speed and load, breaking resume-from-seed. Instead the attempt
budget is fixed, and if the requested band proves unreachable the closest
achievable puzzle is returned with its *actual* rating shown in the HUD.

## How this is verified

Three layers, because each catches things the others cannot.

**Unit tests** (71 functions, 69 of them run by default) carry the invariants.
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

## Known trade-offs

- **Easy and Expert at 12×12 are slow.** Both are scarce (a few percent of
  layouts), so the search grinds: median around 1–2s, with a tail past 5s.
  Mitigated by the background thread and the honest fallback rating, not solved.
- **A sweep is one undo step per cell.** Dragging fifteen crosses takes fifteen
  undos to reverse. Defensible, but not obviously right.
- **Tests run on Linux only in CI.** The logic is platform-independent and the
  other three platforms are built but not tested, which trades a little coverage
  for a lot of CI time.
