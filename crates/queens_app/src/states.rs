//! The screens the game moves between.

use bevy::prelude::*;

/// Top-level screen. Each one owns its UI, tagged with `DespawnOnExit` so
/// leaving a screen tears it down without a bespoke cleanup system.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    MainMenu,
    /// Choosing a board size and difficulty.
    NewGame,
    /// Building a puzzle on a background thread.
    Generating,
    /// A puzzle is on screen.
    Playing,
    Stats,
    Settings,
}

/// Where a game in progress stands.
///
/// A sub-state of [`AppState::Playing`] rather than a screen of its own, so
/// pausing or winning leaves the board on screen underneath the overlay.
#[derive(SubStates, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[source(AppState = AppState::Playing)]
pub enum PlayState {
    /// Clock running, input accepted.
    #[default]
    Active,
    Paused,
    /// Solved. The board stays visible behind the victory panel.
    Won,
}
