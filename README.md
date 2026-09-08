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
on keeps its height whether or not it holds a hint, so nothing moves under you
when you ask.

Every hint spells out what it saw and what follows from it, so none of them
require knowing the name of the move. The names below are only for talking
about them; the game never shows you one, and never tells you in advance which
moves your puzzle will ask for.

| The move | What you are looking for | How a hint puts it |
|---|---|---|
| **Propagation** | A queen rules out the rest of its row, its column, its region, and every cell it touches. | *The queen at r3c5 already rules out r4c6.* |
| **Last cell** | A row, column or region with one cell left has to put its queen there. | *Row 3 has only one cell left for its queen: r3c5.* |
| **Confined to a line** | All a region has left sits in one row or column, so that line's queen is one of those cells and the rest of the line is clear. | *Every remaining cell of the teal region lies in column 8, so the queen of column 8 must be one of them and the rest of column 8 can be crossed off.* |
| **Confined to a region** | The same the other way round: all a row or column has left sits in one region. | *Every remaining cell of row 4 lies in the sand region, so the queen of the sand region must be one of them and the rest of the sand region can be crossed off.* |
| **Shared elimination** | A cell that dies wherever some region's queen goes is dead outright — you do not need to know which cell that queen takes. | *Wherever the queen of the coral region goes, these cells are ruled out.* |
| **Locked set** | Two or three rows with only the same two or three columns left between them. Those columns are theirs: one each, in some order, and no other row gets a look in. The same holds for any two families — rows and regions, columns and regions. | *Between them, rows 2, 5 and 7 can only reach columns 1, 4 and 8, so those are spoken for and no other row can use them.* |
| **Contradiction** | Nothing above works. Try a cell, follow it through, and cross it off if it leads to an impossible board. | Only ever needed on Expert. |

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
| **Hard** | A locked set: two or three rows sharing exactly that many columns |
| **Expert** | Proof by contradiction |

Board size and difficulty are chosen independently, so an Easy 11×11 and an
Expert 6×6 are both perfectly reasonable requests. Outside Easy, every region
holds at least two cells — a one-cell region places its own queen for nothing.

## Seeds and share codes

Every puzzle is reproducible from its board size, difficulty and RNG seed,
shown together as a share code in the top bar during play and on the victory
panel. Click it to copy it, then paste it into the **Share code** box on the
New Game screen to play that exact puzzle again - board size and difficulty
lock to what the code says while it is set.

To reuse just the seed at a size or difficulty of your own choosing, type it
into the separate **Seed** box instead and pick size and difficulty as usual,
or leave both boxes blank for something new.

Saved games store only the seed, size and difficulty, so the resume slot is a
few bytes and the board is rebuilt on load.

## Settings and progress

Settings, per-difficulty statistics and one in-progress game live in
`save.ron` under your platform's data directory (`%APPDATA%\queens` on Windows,
`~/.local/share/queens` on Linux, `~/Library/Application Support/queens` on
macOS). A corrupt or outdated file is discarded rather than fatal.

**Statistics** are per difficulty: puzzles solved out of started, your best and
average solve times, and the hints you spent. Hints are counted per puzzle
rather than per press — asking again without touching the board just re-reads
the hint already on screen — and they follow a puzzle across a save and resume.
Only the puzzles you went on to solve are counted, which is what keeps the
figure comparable with the times beside it. The victory panel shows the tally
for the puzzle you just finished.

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

## Credits

Set in [Space Grotesk](https://github.com/floriankarsten/space-grotesk),
embedded in the binary under its SIL Open Font Licence
(`crates/queens_app/assets/fonts/OFL.txt`).

## Layout

| Crate | What it is |
|---|---|
| `queens_core` | Board, rules, solvers and the generator. No Bevy dependency. |
| `queens_cli` | `queens-gen`, the headless generator and benchmark tool. |
| `queens_app` | `queens`, the game. |

Further reading: [DESIGN.md](DESIGN.md) for why it is built this way,
[INVENTORY.md](INVENTORY.md) for a file-by-file map.
