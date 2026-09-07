//! The playing screen: board, bars, overlays and the rules that tie them
//! together.

pub(crate) mod board;
mod hud;
mod interaction;

use bevy::prelude::*;

use crate::persistence::{InProgress, SaveData};
use crate::session::{PuzzleRequest, Session};
use crate::states::{AppState, PlayState};
use crate::theme;

/// How often a game in progress is written to disk while playing.
const AUTOSAVE_SECONDS: f32 = 15.0;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<interaction::PaintStroke>()
            .add_systems(OnEnter(AppState::Playing), spawn_screen)
            .add_systems(OnExit(AppState::Playing), leave_game)
            .add_systems(OnEnter(PlayState::Paused), spawn_pause_overlay)
            .add_systems(OnEnter(PlayState::Won), (record_win, spawn_victory_overlay))
            .add_systems(
                Update,
                (
                    board::refresh_board,
                    hud::refresh_hud,
                    hud::clear_seed_acknowledgement,
                    interaction::keyboard_shortcuts,
                    autosave,
                )
                    .run_if(in_state(AppState::Playing).and_then(resource_exists::<Session>)),
            )
            .add_systems(
                Update,
                (tick_clock, detect_win)
                    .chain()
                    .run_if(in_state(PlayState::Active).and_then(resource_exists::<Session>)),
            );
    }
}

/// Lays out the playing screen: status bar, board, toolbar.
fn spawn_screen(mut commands: Commands, session: Res<Session>, save: Res<SaveData>) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                row_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(18.0)),
                ..default()
            },
            BackgroundColor(theme::BACKGROUND),
            DespawnOnExit(AppState::Playing),
        ))
        .with_children(|screen| {
            hud::spawn_top_bar(screen, &session);
            screen
                .spawn(Node {
                    flex_grow: 1.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|middle| board::spawn_grid(middle, &session, &save));
            hud::spawn_bottom_bar(screen);
        });
}

fn tick_clock(time: Res<Time>, mut session: ResMut<Session>) {
    // Bypass change detection: the clock ticks every frame, and treating that
    // as a session change would redraw the whole board sixty times a second.
    session.bypass_change_detection().elapsed += time.delta_secs();
}

fn detect_win(session: Res<Session>, mut next_play: ResMut<NextState<PlayState>>) {
    if session.is_changed() && session.is_solved() {
        next_play.set(PlayState::Won);
    }
}

/// Periodically stores the game so an unexpected exit does not lose it.
fn autosave(
    time: Res<Time>,
    session: Res<Session>,
    mut save: ResMut<SaveData>,
    mut timer: Local<Option<Timer>>,
) {
    let timer =
        timer.get_or_insert_with(|| Timer::from_seconds(AUTOSAVE_SECONDS, TimerMode::Repeating));
    if timer.tick(time.delta()).just_finished() && !session.is_solved() {
        save.in_progress = Some(snapshot(&session));
    }
}

/// Stores an unfinished game on the way out, and clears the slot on a finished
/// one so "Continue" never offers a solved board.
fn leave_game(mut commands: Commands, session: Option<Res<Session>>, mut save: ResMut<SaveData>) {
    if let Some(session) = session {
        save.in_progress = (!session.is_solved()).then(|| snapshot(&session));
    }
    crate::persistence::flush(&save);
    commands.remove_resource::<Session>();
}

fn snapshot(session: &Session) -> InProgress {
    InProgress {
        seed: session.puzzle.seed(),
        marks: session.board.marks().to_vec(),
        elapsed: session.elapsed,
        auto_crossed: session.auto_crossed().to_vec(),
        hints_used: session.hints_used,
    }
}

fn record_win(session: Res<Session>, mut save: ResMut<SaveData>) {
    save.record_solved(
        session.puzzle.rating().difficulty,
        session.elapsed,
        session.hints_used,
    );
    save.in_progress = None;
}

// --- overlays --------------------------------------------------------------

/// A dimmed sheet over the board, holding a panel.
fn overlay(despawn_on: impl Bundle) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.82)),
        GlobalZIndex(10),
        despawn_on,
    )
}

fn spawn_pause_overlay(mut commands: Commands) {
    commands
        .spawn(overlay(DespawnOnExit(PlayState::Paused)))
        .with_children(|sheet| {
            sheet.spawn(theme::panel()).with_children(|panel| {
                panel.spawn(theme::title("Paused"));
                panel.spawn(theme::subtitle("The clock is stopped."));
                panel.spawn(theme::accent_button("Resume")).observe(
                    |_click: On<Pointer<Click>>, mut next: ResMut<NextState<PlayState>>| {
                        next.set(PlayState::Active);
                    },
                );
                panel.spawn(theme::menu_button("Main Menu")).observe(
                    |_click: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
                        next.set(AppState::MainMenu);
                    },
                );
            });
        });
}

fn spawn_victory_overlay(mut commands: Commands, session: Res<Session>, save: Res<SaveData>) {
    let size = session.puzzle.size();
    let rating = session.puzzle.rating().difficulty;
    let elapsed = session.elapsed;
    let best = save.stats_for(rating).best_seconds;
    let is_best = best.is_none_or(|best| elapsed <= best + f32::EPSILON);

    commands
        .spawn(overlay(DespawnOnExit(PlayState::Won)))
        .with_children(|sheet| {
            sheet.spawn(theme::panel()).with_children(|panel| {
                panel.spawn(theme::title("Solved"));
                panel.spawn(theme::text(
                    theme::format_time(elapsed),
                    38.0,
                    if is_best { theme::SUCCESS } else { theme::TEXT },
                ));
                panel.spawn(theme::subtitle(if is_best {
                    format!("A new best for {rating}.")
                } else {
                    format!(
                        "{size}x{size} {rating}   best {}",
                        best.map(theme::format_time).unwrap_or_default()
                    )
                }));
                panel.spawn(theme::subtitle(match session.hints_used {
                    0 => "Solved without a hint.".to_string(),
                    1 => "1 hint used.".to_string(),
                    n => format!("{n} hints used."),
                }));
                panel.spawn(theme::subtitle(format!(
                    "Seed {}",
                    session.puzzle.seed().seed
                )));

                panel.spawn(theme::accent_button("New Puzzle")).observe(
                    |_click: On<Pointer<Click>>,
                     mut commands: Commands,
                     save: Res<SaveData>,
                     mut next: ResMut<NextState<AppState>>| {
                        let seed = queens_core::PuzzleSeed::random(
                            save.settings.size,
                            save.settings.difficulty,
                        );
                        commands.insert_resource(PuzzleRequest::fresh(seed));
                        next.set(AppState::Generating);
                    },
                );
                panel.spawn(theme::menu_button("Main Menu")).observe(
                    |_click: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
                        next.set(AppState::MainMenu);
                    },
                );
            });
        });
}
