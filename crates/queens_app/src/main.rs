//! Queens — a puzzle game.
//!
//! Divide an `n` x `n` board into `n` coloured regions and place `n` queens so
//! that every row, every column and every region holds exactly one, and no two
//! touch. All the puzzle logic lives in `queens_core`; this crate is the game
//! around it.

// Bevy system signatures are unavoidably verbose; the type-complexity lint
// fires on ordinary, idiomatic queries.
#![allow(clippy::type_complexity)]

mod capture;
mod game;
mod generation;
mod menu;
mod persistence;
mod session;
mod states;
mod theme;

use bevy::prelude::*;

use states::{AppState, PlayState};

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Queens".into(),
                resolution: (1024u32, 820u32).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(theme::BACKGROUND))
        .init_state::<AppState>()
        .add_sub_state::<PlayState>()
        .add_plugins((
            theme::ThemePlugin,
            capture::CapturePlugin,
            persistence::PersistencePlugin,
            generation::GenerationPlugin,
            menu::MenuPlugin,
            game::GamePlugin,
        ))
        .add_systems(Startup, spawn_camera)
        .run()
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
