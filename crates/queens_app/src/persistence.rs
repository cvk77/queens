//! Settings, statistics and the resume slot, stored as RON in the user's data
//! directory.
//!
//! An in-progress game is saved as its [`PuzzleSeed`] plus the player's marks —
//! never the board itself. Generation is deterministic, so the puzzle is rebuilt
//! on load. That keeps the file tiny and makes the determinism guarantee in
//! `queens_core` load-bearing rather than incidental.

#[cfg(not(target_arch = "wasm32"))]
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
    /// Hints spent on solved puzzles. Counted alongside `solved` rather than
    /// `started` so it stays comparable with the times: both describe the
    /// puzzles that were actually finished. Defaulted rather than versioned,
    /// so a save written before this existed still loads.
    #[serde(default)]
    pub hints_used: u32,
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
    /// Hints spent so far, so putting a puzzle down and picking it up again
    /// does not wipe the slate.
    #[serde(default)]
    pub hints_used: u32,
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

    /// Records a solve, the hints it took, and the best time.
    pub fn record_solved(&mut self, difficulty: Difficulty, seconds: f32, hints: u32) {
        let stats = self.stats_for_mut(difficulty);
        stats.solved += 1;
        stats.total_seconds += seconds;
        stats.hints_used += hints;
        stats.best_seconds = Some(match stats.best_seconds {
            Some(best) => best.min(seconds),
            None => seconds,
        });
    }

    /// Reads the stored save, falling back to defaults for anything unreadable.
    ///
    /// A corrupt or outdated save is never fatal: the player loses their
    /// statistics, not their ability to launch the game.
    pub fn load() -> Self {
        let Some(contents) = backend::read() else {
            return Self::default();
        };
        match ron::from_str::<SaveData>(&contents) {
            Ok(data) if data.version == SAVE_VERSION => data,
            Ok(data) => {
                warn!(
                    "save is version {} but this build expects {SAVE_VERSION}; starting fresh",
                    data.version
                );
                Self::default()
            }
            Err(error) => {
                warn!(
                    "could not read the save at {}: {error}",
                    backend::describe()
                );
                Self::default()
            }
        }
    }

    /// Writes the save, reporting rather than propagating failures — a game
    /// should not stop because a write did.
    pub fn save(&self) {
        match ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            Ok(text) => backend::write(&text),
            Err(error) => warn!("could not serialise the save: {error}"),
        }
    }
}

/// Where a save is kept, which is the only thing about persistence that
/// differs between a desktop build and a browser one. The format either side
/// writes is the same RON, so [`SAVE_VERSION`] governs both.
mod backend {
    use super::*;

    /// The save file on disk. `None` if the platform has no data directory.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn path() -> Option<PathBuf> {
        dirs::data_dir().map(|dir| dir.join("queens").join("save.ron"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn describe() -> String {
        path().map_or_else(
            || "<no data directory>".to_string(),
            |p| p.display().to_string(),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn read() -> Option<String> {
        std::fs::read_to_string(path()?).ok()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn write(text: &str) {
        let Some(path) = path() else {
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            warn!("could not create {}: {error}", parent.display());
            return;
        }
        if let Err(error) = std::fs::write(&path, text) {
            warn!("could not write {}: {error}", path.display());
        }
    }

    /// The key the save lives under in `localStorage`. Namespaced because an
    /// itch.io page shares an origin with every other game on that domain.
    #[cfg(target_arch = "wasm32")]
    const STORAGE_KEY: &str = "de.treestack.queens.save";

    /// `localStorage`, when the browser will give it to us. Private browsing
    /// and blocked site data both make this `None`, which is treated exactly
    /// like a missing save file: defaults, and writes that quietly go nowhere.
    #[cfg(target_arch = "wasm32")]
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    #[cfg(target_arch = "wasm32")]
    pub fn describe() -> String {
        format!("localStorage[{STORAGE_KEY}]")
    }

    #[cfg(target_arch = "wasm32")]
    pub fn read() -> Option<String> {
        storage()?.get_item(STORAGE_KEY).ok()?
    }

    #[cfg(target_arch = "wasm32")]
    pub fn write(text: &str) {
        let Some(storage) = storage() else {
            warn!("no localStorage available; this session will not be saved");
            return;
        };
        if storage.set_item(STORAGE_KEY, text).is_err() {
            warn!("could not write to localStorage; it may be full");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_accumulate_the_hints_they_took() {
        let mut save = SaveData::default();
        save.record_solved(Difficulty::Hard, 100.0, 3);
        save.record_solved(Difficulty::Hard, 80.0, 0);
        save.record_solved(Difficulty::Hard, 90.0, 1);
        // A different band keeps its own tally.
        save.record_solved(Difficulty::Easy, 20.0, 5);

        let hard = save.stats_for(Difficulty::Hard);
        assert_eq!(hard.solved, 3);
        assert_eq!(hard.hints_used, 4);
        assert_eq!(hard.best_seconds, Some(80.0));

        let easy = save.stats_for(Difficulty::Easy);
        assert_eq!(easy.hints_used, 5);
    }

    /// New fields are defaulted rather than versioned, so a save written before
    /// hints were counted still loads and keeps the statistics it does have.
    #[test]
    fn a_save_predating_the_hint_counters_still_loads() {
        // Copied from a file this build's predecessor wrote: the fixed-size
        // stats array is a RON tuple, not a list.
        let older = r#"(
            version: 2,
            settings: (size: 8, difficulty: Medium, auto_cross: true, colourblind: false),
            stats: (
                (started: 4, solved: 2, best_seconds: Some(61.5), total_seconds: 200.0),
                (started: 0, solved: 0, best_seconds: None, total_seconds: 0.0),
                (started: 0, solved: 0, best_seconds: None, total_seconds: 0.0),
                (started: 0, solved: 0, best_seconds: None, total_seconds: 0.0),
            ),
            in_progress: Some((
                seed: (size: 8, difficulty: Medium, seed: 74763209),
                marks: [],
                elapsed: 12.0,
            )),
        )"#;

        let save: SaveData = ron::from_str(older).expect("an older save should still parse");
        let easy = save.stats_for(Difficulty::Easy);
        assert_eq!(easy.solved, 2);
        assert_eq!(easy.hints_used, 0);
        assert_eq!(save.in_progress.expect("resume slot").hints_used, 0);
    }
}
