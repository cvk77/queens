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
                (animate_ellipsis, finish_task).run_if(in_state(AppState::Generating)),
            );
    }
}

/// The in-flight generation job. Dropping this entity cancels the task.
#[derive(Component)]
struct GenerationTask(Task<Puzzle>);

#[derive(Component)]
struct Ellipsis(f32);

fn spawn_screen(mut commands: Commands, request: Res<PuzzleRequest>) {
    let seed = request.seed;
    commands.spawn((
        theme::screen(DespawnOnExit(AppState::Generating)),
        children![(
            theme::panel(),
            children![
                theme::text("Building a puzzle", 30.0, theme::TEXT),
                (theme::subtitle(""), Ellipsis(0.0),),
                theme::subtitle(format!("{0}x{0}   {1}", seed.size, seed.difficulty)),
            ],
        )],
    ));
}

/// Hands the search to the async compute pool.
fn start_task(mut commands: Commands, request: Res<PuzzleRequest>) {
    let seed = request.seed;
    let task = AsyncComputeTaskPool::get().spawn(async move { generator::generate(seed) });
    commands.spawn((GenerationTask(task), DespawnOnExit(AppState::Generating)));
}

/// A cheap "still working" tell, so a long Expert search does not look frozen.
fn animate_ellipsis(time: Res<Time>, mut labels: Query<(&mut Ellipsis, &mut Text)>) {
    for (mut ellipsis, mut label) in &mut labels {
        ellipsis.0 += time.delta_secs();
        let dots = ((ellipsis.0 * 2.5) as usize % 4) + 1;
        label.0 = ".".repeat(dots);
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
