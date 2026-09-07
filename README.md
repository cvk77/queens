# Queens

A logic puzzle game built with [Bevy](https://bevy.org). Randomly generated
boards from 5×5 to 12×12, in four difficulty bands that mean what they say.

```
cargo run -p queens_app --release
```

## The rules

An `n × n` board is divided into `n` coloured regions. Place `n` queens so that
there is exactly one in every **row**, every **column** and every **region** —
and so that **no two queens touch**, not even diagonally.

That last rule is where Queens parts company with classic N-Queens: full
diagonals are unrestricted, only adjacency matters. Two queens on the same
diagonal are perfectly legal as long as they are two or more rows apart.

```
 . . Q . .      Q on r0c2
 . . . . Q      legal: three columns away, not touching
 . Q . . .
 . . . . .      illegal would be r1c3 — diagonally adjacent to r0c2
```

Every generated puzzle has **exactly one** solution, and every one can be solved
by reasoning alone. You never need to guess.

## Playing

| Input | Effect |
|---|---|
| Left click | Cycle a cell: empty → cross → queen → empty |
| Right click | Place or lift a queen directly |
| Left drag | Sweep a run of crosses; drag from a crossed cell to rub them out |
| `Esc` | Pause and resume |
| `H` | Hint |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo (`Ctrl+Shift+Z` also redoes) |
| `Ctrl+R` | Clear the board |

Crosses are your own notes — the rules ignore them. Queens that break a rule are
outlined in red as soon as you place them.

**Hints** come from the same deductive solver that rates the puzzles, so a hint
is always a step you could have taken yourself. It reads the board you are
actually looking at: your crosses are taken as read and the hint carries on from
there. If a queen or a cross of yours contradicts the solution, the hint says so
instead, because nothing sound can be deduced past a false premise.

A hint names what it is talking about the way the board shows it: rows and
columns by the numbers running alongside the grid, a region by its colour. Every
cell the step applies to is outlined, not just the first. The line it is written
on keeps its height whether it holds a hint or the puzzle's rating, so nothing
moves under you when you ask.

**Pausing** hides every queen and cross. The clock stops, so the board stops
being readable too.

## Difficulty

Difficulty is what the puzzle demands of you, not how big it is. Each generated
board is solved by a solver that applies named deduction rules from the most
obvious to the most demanding, and the band is set by the hardest rule it needed.

| Band | Hardest step required |
|---|---|
| **Easy** | A row, column or region with only one cell left |
| **Medium** | Confinement, and cells ruled out by every option a region has |
| **Hard** | A locked set: `k` units that can only reach `k` counterparts |
| **Expert** | Proof by contradiction |

Board size and difficulty are chosen independently, so an Easy 11×11 and an
Expert 6×6 are both perfectly reasonable requests. Outside Easy, every region
holds at least two cells — a one-cell region places its own queen for nothing.

## Seeds

Every puzzle is reproducible from an eight-digit seed, shown in the top bar
during play and on the victory panel. Type it into the **Seed** box on the New
Game screen to play that exact board again, or leave the box blank for something
new.

Saved games store only the seed, so the resume slot is a few bytes and the board
is rebuilt on load.

## Settings and progress

Settings, per-difficulty statistics and one in-progress game live in
`save.ron` under your platform's data directory (`%APPDATA%\queens` on Windows,
`~/.local/share/queens` on Linux, `~/Library/Application Support/queens` on
macOS). A corrupt or outdated file is discarded rather than fatal.

- **Auto-cross** crosses off every cell a queen rules out the moment you place
  it. Lift that queen again and its crosses go with it; the ones you made
  yourself stay.
- **Colour-blind palette** swaps in region colours chosen to stay separable.

## Building

Needs Rust 1.95 or newer.

```sh
cargo run -p queens_app --release   # play
cargo run -p queens_app --features dev   # much faster rebuilds while developing
cargo test --workspace
```

On Linux, Bevy needs a few system libraries:

```sh
sudo apt-get install pkg-config libx11-dev libasound2-dev libudev-dev \
                     libxkbcommon-dev libwayland-dev
```

Prebuilt binaries for Windows, Linux and macOS are attached to each release, and
built for every push by `.github/workflows/ci.yml`.

## The generator tool

`queens-gen` exercises the puzzle generator without opening a window — useful
for checking generator changes at a scale no amount of playing would reach.

```sh
# One puzzle, printed as region letters with the solution in lower case
cargo run -p queens_cli --release -- generate --size 9 --difficulty hard --solution

# Hundreds of puzzles, auditing every invariant and timing the search
cargo run -p queens_cli --release -- bench --sizes 5-12 --difficulties all --count 20
```

`bench` checks each puzzle for a unique solution, contiguous regions, a rating
that still holds on a fresh solve, and byte-identical regeneration from its seed.
It exits non-zero if any puzzle fails.

## Layout

| Crate | What it is |
|---|---|
| `queens_core` | Board, rules, solvers and the generator. No Bevy dependency. |
| `queens_cli` | `queens-gen`, the headless generator and benchmark tool. |
| `queens_app` | `queens`, the game. |

Further reading: [DESIGN.md](DESIGN.md) for why it is built this way,
[INVENTORY.md](INVENTORY.md) for a file-by-file map.
