//! Settings, statistics and the resume slot, stored as RON in the user's data
//! directory.
//!
//! An in-progress game is saved as its [`PuzzleSeed`] plus the player's marks —
//! never the board itself. Generation is deterministic, so the puzzle is rebuilt
//! on load. That keeps the file tiny and makes the determinism guarantee in
//! `queens_core` load-bearing rather than incidental.

use std::path::PathBuf;

use bevy::prelude::*;
use queens_core::{Difficulty, Mark, PuzzleSeed, rating::ALL_DIFFICULTIES};
use serde::{Deserialize, Serialize};

/// Bumped when an old file can no longer be trusted; older files are discarded
/// rather than migrated.
///
/// This tracks the generator as well as the format. A resume slot holds only a
/// seed, so a change to how seeds turn into puzzles would restore the player's
/// marks onto a different board. Bump this whenever generation changes.
///
/// - 2: regions must hold at least two cells outside Easy.
pub const SAVE_VERSION: u32 = 2;

/// How often an in-progress game is written back while playing.
const AUTOSAVE_SECONDS: f32 = 5.0;

/// Player preferences.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Settings {
    /// Board size the New Game screen opens on.
    pub size: u8,
    /// Difficulty the New Game screen opens on.
    pub difficulty: Difficulty,
    /// Cross off every cell a queen rules out, the moment it is placed.
    pub auto_cross: bool,
    /// Swap in the colour-blind-safe region palette.
    pub colourblind: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            size: 8,
            difficulty: Difficulty::Medium,
            auto_cross: true,
            colourblind: false,
        }
    }
}

/// A player's record in one difficulty band.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct DifficultyStats {
    pub started: u32,
    pub solved: u32,
    /// Best solve time in seconds.
    pub best_seconds: Option<f32>,
    /// Time spent on solved puzzles, for an average.
    pub total_seconds: f32,
}

impl DifficultyStats {
    pub fn average_seconds(&self) -> Option<f32> {
        (self.solved > 0).then(|| self.total_seconds / self.solved as f32)
    }
}

/// A game the player can come back to.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InProgress {
    pub seed: PuzzleSeed,
    pub marks: Vec<Mark>,
    pub elapsed: f32,
    /// Which of those crosses the auto-cross assist placed. Defaulted rather
    /// than versioned, so a save written before this existed still loads.
    #[serde(default)]
    pub auto_crossed: Vec<bool>,
}

/// Everything that outlives a session.
#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct SaveData {
    pub version: u32,
    pub settings: Settings,
    /// Indexed by [`Difficulty::index`] — an array rather than a map so the
    /// file stays readable and no key ever fails to parse.
    pub stats: [DifficultyStats; ALL_DIFFICULTIES.len()],
    pub in_progress: Option<InProgress>,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            settings: Settings::default(),
            stats: [DifficultyStats::default(); ALL_DIFFICULTIES.len()],
            in_progress: None,
        }
    }
}

impl SaveData {
    pub fn stats_for(&self, difficulty: Difficulty) -> &DifficultyStats {
        &self.stats[difficulty.index()]
    }

    pub fn stats_for_mut(&mut self, difficulty: Difficulty) -> &mut DifficultyStats {
        &mut self.stats[difficulty.index()]
    }

    /// Records the start of a puzzle.
    pub fn record_started(&mut self, difficulty: Difficulty) {
        self.stats_for_mut(difficulty).started += 1;
    }

    /// Records a solve and updates the best time.
    pub fn record_solved(&mut self, difficulty: Difficulty, seconds: f32) {
        let stats = self.stats_for_mut(difficulty);
        stats.solved += 1;
        stats.total_seconds += seconds;
        stats.best_seconds = Some(match stats.best_seconds {
            Some(best) => best.min(seconds),
            None => seconds,
        });
    }

    /// Where the save file lives. `None` if the platform has no data directory.
    pub fn path() -> Option<PathBuf> {
        dirs::data_dir().map(|dir| dir.join("queens").join("save.ron"))
    }

    /// Reads the save file, falling back to defaults for anything unreadable.
    ///
    /// A corrupt or outdated file is never fatal: the player loses their
    /// statistics, not their ability to launch the game.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(contents) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match ron::from_str::<SaveData>(&contents) {
            Ok(data) if data.version == SAVE_VERSION => data,
            Ok(data) => {
                warn!(
                    "save file is version {} but this build expects {SAVE_VERSION}; starting fresh",
                    data.version
                );
                Self::default()
            }
            Err(error) => {
                warn!("could not read save file at {}: {error}", path.display());
                Self::default()
            }
        }
    }

    /// Writes the save file, reporting rather than propagating failures — a
    /// game should not stop because a disk write did.
    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            warn!("could not create {}: {error}", parent.display());
            return;
        }
        match ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            Ok(text) => {
                if let Err(error) = std::fs::write(&path, text) {
                    warn!("could not write {}: {error}", path.display());
                }
            }
            Err(error) => warn!("could not serialise the save file: {error}"),
        }
    }
}

/// Loads the save file at startup and writes it back whenever it changes.
pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SaveData::load())
            .insert_resource(AutosaveTimer(Timer::from_seconds(
                AUTOSAVE_SECONDS,
                TimerMode::Repeating,
            )))
            .add_systems(Last, write_when_changed);
    }
}

#[derive(Resource)]
struct AutosaveTimer(Timer);

/// Persists on change, throttled so a burst of edits collapses into one write.
///
/// Change detection is enough because [`SaveData`] is only mutated at moments
/// that matter — a setting toggled, a puzzle finished, an autosave tick — not
/// on every click.
fn write_when_changed(
    save: Res<SaveData>,
    time: Res<Time>,
    mut timer: ResMut<AutosaveTimer>,
    mut pending: Local<bool>,
) {
    if save.is_changed() {
        *pending = true;
    }
    timer.0.tick(time.delta());
    if *pending && timer.0.just_finished() {
        save.save();
        *pending = false;
    }
}

/// Writes immediately, for the moments worth not waiting on — quitting, or
/// leaving a game.
pub fn flush(save: &SaveData) {
    save.save();
}
