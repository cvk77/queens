//! The loading screen, and the background thread that builds a puzzle.
//!
//! Generation is a search: it rejects layouts until one is uniquely solvable
//! *and* lands in the requested difficulty band. That is normally a few
//! milliseconds, but an Easy 12x12 is a rare thing to stumble on and can take
//! seconds. Running it on the async compute pool keeps the window responsive
//! and gives the player something to look at meanwhile.

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use queens_core::{Puzzle, generator};

use crate::persistence::SaveData;
use crate::session::{PuzzleRequest, Session};
use crate::states::AppState;
use crate::theme;

pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Generating), (spawn_screen, start_task))
            .add_systems(
                Update,
                (animate_loading_dots, finish_task).run_if(in_state(AppState::Generating)),
            );
    }
}

/// The in-flight generation job. Dropping this entity cancels the task.
#[derive(Component)]
struct GenerationTask(Task<Puzzle>);

/// One of the three dots that pulse in sequence while a search runs, so a
/// long Expert search does not look frozen. Carries its place in the
/// sequence, which sets its phase offset.
#[derive(Component)]
struct LoadingDot(usize);

/// How many dots pulse across the loading screen.
const LOADING_DOTS: usize = 3;
/// How far apart in phase each dot trails the one before it.
const LOADING_DOT_PHASE: f32 = 0.6;
/// How fast the pulse cycles, in radians per second.
const LOADING_PULSE_RATE: f32 = 3.0;

fn spawn_screen(mut commands: Commands, request: Res<PuzzleRequest>) {
    let seed = request.seed;
    commands.spawn((
        theme::screen(DespawnOnExit(AppState::Generating)),
        children![(
            theme::panel(),
            children![
                theme::title("Building a puzzle"),
                loading_dots(),
                theme::subtitle(format!("{0}x{0}   {1}", seed.size, seed.difficulty)),
            ],
        )],
    ));
}

/// Three flat, filled dots — a "still working" tell that pulses rather than a
/// line of text that grows and shrinks.
fn loading_dots() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(theme::GRID),
            margin: UiRect::vertical(Val::Px(theme::GRID)),
            ..default()
        },
        Children::spawn(SpawnIter((0..LOADING_DOTS).map(|i| {
            (
                LoadingDot(i),
                Node {
                    width: Val::Px(14.0),
                    height: Val::Px(14.0),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(theme::ACCENT),
            )
        }))),
    )
}

/// Hands the search to the async compute pool.
fn start_task(mut commands: Commands, request: Res<PuzzleRequest>) {
    let seed = request.seed;
    let task = AsyncComputeTaskPool::get().spawn(async move { generator::generate(seed) });
    commands.spawn((GenerationTask(task), DespawnOnExit(AppState::Generating)));
}

/// Pulses each dot's size and opacity in sequence, entirely as a function of
/// elapsed time, so nothing here can drift out of step with itself.
fn animate_loading_dots(
    time: Res<Time>,
    mut dots: Query<(&LoadingDot, &mut BackgroundColor, &mut UiTransform)>,
) {
    for (dot, mut background, mut transform) in &mut dots {
        let phase = time.elapsed_secs() * LOADING_PULSE_RATE - dot.0 as f32 * LOADING_DOT_PHASE;
        let t = phase.sin() * 0.5 + 0.5;
        transform.scale = Vec2::splat(0.7 + 0.3 * t);
        let colour = theme::ACCENT.to_srgba();
        background.0 = Color::srgba(colour.red, colour.green, colour.blue, 0.35 + 0.65 * t);
    }
}

/// Picks up the finished puzzle and starts the game.
fn finish_task(
    mut commands: Commands,
    mut tasks: Query<&mut GenerationTask>,
    mut next_state: ResMut<NextState<AppState>>,
    mut save: ResMut<SaveData>,
    request: Res<PuzzleRequest>,
) {
    for mut task in &mut tasks {
        let Some(puzzle) = block_on(poll_once(&mut task.0)) else {
            continue;
        };

        // A fresh puzzle counts as an attempt; a resumed one was counted when
        // it was first started.
        if request.restore.is_none() {
            save.record_started(puzzle.rating().difficulty);
        }
        if puzzle.rating().difficulty != request.seed.difficulty {
            info!(
                "requested {} at {}x{}; the closest reachable puzzle was {}",
                request.seed.difficulty,
                request.seed.size,
                request.seed.size,
                puzzle.rating().difficulty
            );
        }

        commands.insert_resource(Session::new(puzzle, request.restore.clone()));
        next_state.set(AppState::Playing);
    }
}
