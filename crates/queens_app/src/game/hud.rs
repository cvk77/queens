//! The bars above and below the board: status, and the tools for solving.

use bevy::prelude::*;

use crate::persistence::SaveData;
use crate::session::{PuzzleRequest, Session};
use crate::states::{AppState, PlayState};
use crate::theme;

/// Shows elapsed play time.
#[derive(Component)]
pub(super) struct TimerLabel;

/// Shows how many queens are down out of how many are needed.
#[derive(Component)]
pub(super) struct QueensLabel;

/// Shows the most recent hint, or the puzzle's rating when there is none.
#[derive(Component)]
pub(super) struct MessageLabel;

/// Greyed out when there is nothing to undo or redo.
#[derive(Component, Clone, Copy)]
pub(super) enum HistoryButton {
    Undo,
    Redo,
}

/// The status bar above the board.
pub fn spawn_top_bar(parent: &mut ChildSpawnerCommands, session: &Session) {
    let size = session.puzzle.size();
    let rating = session.puzzle.rating().difficulty;

    parent.spawn(bar()).with_children(|bar| {
        bar.spawn(theme::small_button("< Menu")).observe(
            |_click: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
                next.set(AppState::MainMenu);
            },
        );

        bar.spawn(spacer());
        bar.spawn(theme::text(
            format!("{size}x{size}   {rating}"),
            19.0,
            theme::TEXT,
        ));
        // The seed, so a puzzle worth keeping can be written down and typed
        // back in on the New Game screen.
        bar.spawn((
            theme::text(
                format!("#{}", session.puzzle.seed().seed),
                15.0,
                theme::TEXT_DIM,
            ),
            Node {
                margin: UiRect::left(Val::Px(10.0)),
                ..default()
            },
            TextLayout::no_wrap(),
        ));
        bar.spawn(spacer());

        bar.spawn((theme::text("0:00", 19.0, theme::TEXT), TimerLabel));
        bar.spawn((
            theme::text("", 19.0, theme::TEXT_DIM),
            QueensLabel,
            Node {
                min_width: Val::Px(130.0),
                ..default()
            },
            // The counter must stay on one line; wrapping it to "Queens" over
            // "8/8" pushes the bar out of alignment.
            TextLayout::no_wrap(),
        ));
    });
}

/// The toolbar below the board, plus the running commentary.
pub fn spawn_bottom_bar(parent: &mut ChildSpawnerCommands, session: &Session) {
    let hardest = session.puzzle.rating().hardest_rule;

    parent
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            width: Val::Percent(100.0),
            ..default()
        },))
        .with_children(|column| {
            column.spawn((
                theme::text(
                    format!("Hardest step needed: {}", hardest.name()),
                    16.0,
                    theme::TEXT_DIM,
                ),
                MessageLabel,
                // Hint explanations are full sentences; keep them off the
                // window edges and centred over the toolbar.
                Node {
                    max_width: Val::Percent(74.0),
                    ..default()
                },
                TextLayout::justify(Justify::Center),
            ));

            column.spawn(theme::row(10.0)).with_children(|row| {
                row.spawn(theme::small_button("Hint")).observe(
                    |_click: On<Pointer<Click>>, mut session: ResMut<Session>| {
                        session.request_hint();
                    },
                );
                row.spawn((theme::small_button("Undo"), HistoryButton::Undo))
                    .observe(|_click: On<Pointer<Click>>, mut session: ResMut<Session>| {
                        session.undo();
                    });
                row.spawn((theme::small_button("Redo"), HistoryButton::Redo))
                    .observe(|_click: On<Pointer<Click>>, mut session: ResMut<Session>| {
                        session.redo();
                    });
                row.spawn(theme::small_button("Reset")).observe(
                    |_click: On<Pointer<Click>>, mut session: ResMut<Session>| {
                        session.reset();
                    },
                );
                row.spawn(theme::small_button("New")).observe(
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
                row.spawn(theme::small_button("Pause")).observe(
                    |_click: On<Pointer<Click>>, mut next: ResMut<NextState<PlayState>>| {
                        next.set(PlayState::Paused);
                    },
                );
            });
        });
}

fn bar() -> impl Bundle {
    Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(14.0),
        padding: UiRect::horizontal(Val::Px(8.0)),
        ..default()
    }
}

/// Pushes the items on either side of it apart.
fn spacer() -> impl Bundle {
    Node {
        flex_grow: 1.0,
        ..default()
    }
}

/// Keeps the clock, the counter and the message line current.
pub fn refresh_hud(
    session: Res<Session>,
    mut timer: Query<
        &mut Text,
        (
            With<TimerLabel>,
            Without<QueensLabel>,
            Without<MessageLabel>,
        ),
    >,
    mut queens: Query<&mut Text, (With<QueensLabel>, Without<MessageLabel>)>,
    mut message: Query<(&mut Text, &mut TextColor), With<MessageLabel>>,
    mut history: Query<(&HistoryButton, &mut theme::ButtonTint)>,
) {
    for mut label in &mut timer {
        let formatted = theme::format_time(session.elapsed);
        if label.0 != formatted {
            label.0 = formatted;
        }
    }

    if session.is_changed() {
        let placed = session.queens_placed();
        let needed = session.puzzle.size();
        for mut label in &mut queens {
            label.0 = format!("Queens {placed}/{needed}");
        }

        for (mut label, mut color) in &mut message {
            let (text, tint) = match &session.hint {
                Some(hint) => (hint.message.clone(), theme::ACCENT),
                None if !session.conflicts.is_empty() => {
                    ("Some queens are in conflict.".to_string(), theme::DANGER)
                }
                None => (
                    format!(
                        "Hardest step needed: {}",
                        session.puzzle.rating().hardest_rule.name()
                    ),
                    theme::TEXT_DIM,
                ),
            };
            label.0 = text;
            color.0 = tint;
        }

        for (button, mut tint) in &mut history {
            let enabled = match button {
                HistoryButton::Undo => session.can_undo(),
                HistoryButton::Redo => session.can_redo(),
            };
            let base = if enabled { theme::BUTTON } else { theme::PANEL };
            if tint.base != base {
                tint.base = base;
            }
        }
    }
}
