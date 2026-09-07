//! Difficulty taxonomy: the deduction rules a puzzle demands, and the band
//! those rules place it in.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The deduction rules the logical solver knows, ordered from cheapest and most
/// obvious to most demanding. A puzzle's difficulty is the band of the hardest
/// rule needed to finish it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum RuleId {
    /// Eliminate everything a placed queen rules out. Always applied.
    Propagate,
    /// A row, column or region with a single remaining candidate.
    Single,
    /// Some unit's candidates all lie in a single row or column, so that
    /// line's queen is spoken for and the rest of it is clear.
    LineConfinement,
    /// Some unit's candidates all lie in a single region, so that region's
    /// queen is spoken for and the rest of it is clear.
    RegionConfinement,
    /// A cell ruled out by *every* candidate of some unit is ruled out.
    CommonElimination,
    /// `k` units confined to `k` counterpart units consume them exclusively.
    SetElimination,
    /// Assume a candidate, propagate, and eliminate it if that contradicts.
    Contradiction,
}

/// Every rule, in application order.
pub const ALL_RULES: [RuleId; 7] = [
    RuleId::Propagate,
    RuleId::Single,
    RuleId::LineConfinement,
    RuleId::RegionConfinement,
    RuleId::CommonElimination,
    RuleId::SetElimination,
    RuleId::Contradiction,
];

impl RuleId {
    /// Position in [`ALL_RULES`], used to index per-rule counters.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The difficulty band a puzzle lands in when this is the hardest rule it
    /// needs.
    ///
    /// The split is calibrated against how often each rule actually turns out
    /// to be the deciding one (see the `yield_rates` diagnostic in
    /// `generator.rs`). Shared elimination sounds advanced but is the everyday
    /// move of Queens — "wherever this region's queen goes, that cell dies" —
    /// and decides roughly half of all puzzles, so it sits in Medium. Spotting
    /// a locked set across several units is the genuine step up.
    pub const fn difficulty(self) -> Difficulty {
        match self {
            RuleId::Propagate | RuleId::Single => Difficulty::Easy,
            RuleId::LineConfinement | RuleId::RegionConfinement | RuleId::CommonElimination => {
                Difficulty::Medium
            }
            RuleId::SetElimination => Difficulty::Hard,
            RuleId::Contradiction => Difficulty::Expert,
        }
    }

    /// Short human-readable name, shown by the hint system.
    pub const fn name(self) -> &'static str {
        match self {
            RuleId::Propagate => "Propagation",
            RuleId::Single => "Last cell",
            RuleId::LineConfinement => "Confined to a line",
            RuleId::RegionConfinement => "Confined to a region",
            RuleId::CommonElimination => "Shared elimination",
            RuleId::SetElimination => "Locked set",
            RuleId::Contradiction => "Contradiction",
        }
    }

    /// What the rule asks the player to notice, in one sentence.
    ///
    /// [`RuleId::name`] is a label to recognise a move by once you know it, and
    /// no help at all before then: "Locked set" tells a player nothing about
    /// what to look for. This is the sentence that would.
    ///
    /// Neither reaches the player during a game — the board never names a move,
    /// and never says in advance which ones a puzzle will need — so this is for
    /// tooling and documentation that talks *about* the rules. Deliberately in
    /// the board's vocabulary (rows, columns, regions, queens) and free of the
    /// solver's own terms, and ASCII only, so it can be shown as it stands.
    pub const fn description(self) -> &'static str {
        match self {
            RuleId::Propagate => {
                "A queen rules out the rest of its row, its column, its region, and every cell it touches."
            }
            RuleId::Single => {
                "A row, column or region with one cell left has to put its queen there."
            }
            RuleId::LineConfinement => {
                "When all a region has left sits in one row or column, that line's queen is one of those cells."
            }
            RuleId::RegionConfinement => {
                "When all a row or column has left sits in one region, that region's queen is one of those cells."
            }
            RuleId::CommonElimination => {
                "A cell that dies wherever some region's queen goes is dead outright."
            }
            RuleId::SetElimination => {
                "When two or three rows have only the same two or three columns left, those columns are theirs alone."
            }
            RuleId::Contradiction => {
                "Try a cell, follow it through, and cross it off if it leads to an impossible board."
            }
        }
    }
}

/// How hard a puzzle is, defined by the deductions it requires rather than by
/// board size.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub enum Difficulty {
    #[default]
    Easy,
    Medium,
    Hard,
    Expert,
}

/// Every difficulty, easiest first.
pub const ALL_DIFFICULTIES: [Difficulty; 4] = [
    Difficulty::Easy,
    Difficulty::Medium,
    Difficulty::Hard,
    Difficulty::Expert,
];

impl Difficulty {
    pub const fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Medium => "Medium",
            Difficulty::Hard => "Hard",
            Difficulty::Expert => "Expert",
        }
    }

    /// Ordinal, for measuring how far a fallback puzzle missed its target band.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The smallest region this difficulty tolerates.
    ///
    /// A one-cell region is a free queen: [`RuleId::Single`] places it
    /// immediately, because a region with one cell has one candidate. That is a
    /// fair leg-up on Easy, but on any harder board it hands the player a piece
    /// of the answer for nothing, so Medium and above require every region to
    /// hold at least two cells.
    pub const fn min_region_size(self) -> usize {
        match self {
            Difficulty::Easy => 1,
            Difficulty::Medium | Difficulty::Hard | Difficulty::Expert => 2,
        }
    }

    /// Parses a name case-insensitively, for the CLI.
    pub fn parse(s: &str) -> Option<Self> {
        ALL_DIFFICULTIES
            .into_iter()
            .find(|d| d.name().eq_ignore_ascii_case(s))
    }
}

impl fmt::Display for Difficulty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The result of rating a puzzle with the logical solver.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Rating {
    /// Band of [`Rating::hardest_rule`].
    pub difficulty: Difficulty,
    /// Hardest rule the solver had to reach for.
    pub hardest_rule: RuleId,
    /// How many times each rule fired, indexed by [`RuleId::index`].
    pub rule_counts: [u16; ALL_RULES.len()],
    /// Total deduction steps, a rough proxy for how long the puzzle takes.
    pub steps: u16,
}

impl Rating {
    /// How many times `rule` fired while solving.
    pub fn count(&self, rule: RuleId) -> u16 {
        self.rule_counts[rule.index()]
    }
}
