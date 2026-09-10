//! Queens — a puzzle game.
//!
//! Divide an `n` x `n` board into `n` coloured regions and place `n` queens so
//! that every row, every column and every region holds exactly one, and no two
//! touch. All the puzzle logic lives in `queens_core`; this crate is the game
//! around it.

// Bevy system signatures are unavoidably verbose; the type-complexity lint
// fires on ordinary, idiomatic queries.
#![allow(clippy::type_complexity)]

mod audio;
mod capture;
mod game;
mod generation;
mod howto;
mod menu;
mod persistence;
mod session;
mod states;
mod theme;
mod update_check;

use bevy::prelude::*;
use bevy::winit::{UpdateMode, WinitSettings};
use core::time::Duration;

use states::{AppState, HowToPlayPage, PlayState};

/// How long a still board goes between redraws.
///
/// Bevy's default `WinitSettings` redraws continuously even with nothing to
/// show for it, which is what pegs the CPU while the game sits at a menu or a
/// board waiting on the player. A wait this short is still far more often
/// than anything here animates on its own (the once-a-second game clock, the
/// loading ellipsis), so nothing on screen visibly steps; a pointer, key or
/// window event wakes the loop immediately regardless of the wait.
const IDLE_REDRAW_WAIT: Duration = Duration::from_millis(100);

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(primary_window()),
            ..default()
        }))
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::reactive(IDLE_REDRAW_WAIT),
            unfocused_mode: UpdateMode::reactive_low_power(Duration::from_secs(1)),
        })
        .insert_resource(ClearColor(theme::BACKGROUND))
        .init_state::<AppState>()
        .add_sub_state::<PlayState>()
        .add_sub_state::<HowToPlayPage>()
        .add_plugins((
            theme::ThemePlugin,
            audio::SoundPlugin,
            capture::CapturePlugin,
            persistence::PersistencePlugin,
            generation::GenerationPlugin,
            menu::MenuPlugin,
            game::GamePlugin,
            howto::HowToPlayPlugin,
            update_check::UpdateCheckPlugin,
        ))
        .add_systems(Startup, spawn_camera)
        .run()
}

/// The window the game opens in.
///
/// On the web there is no window to size: the page places the canvas, and
/// fit_canvas_to_parent lets its container decide how big the game is, which
/// is what an itch.io embed frame expects.
fn primary_window() -> Window {
    Window {
        title: "Queens".into(),
        #[cfg(not(target_arch = "wasm32"))]
        resolution: (1024u32, 820u32).into(),
        #[cfg(target_arch = "wasm32")]
        canvas: Some("#queens".into()),
        #[cfg(target_arch = "wasm32")]
        fit_canvas_to_parent: true,
        ..default()
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
