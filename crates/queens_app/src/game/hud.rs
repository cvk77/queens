//! The bars above and below the board: status, and the tools for solving.

use bevy::prelude::*;
use bevy::text::LineHeight;

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

/// Shows the most recent hint, and nothing when there is none.
#[derive(Component)]
pub(super) struct MessageLabel;

/// Acknowledges a copied seed, then clears itself.
#[derive(Component, Default)]
pub(super) struct SeedCopied {
    /// Counts down while the acknowledgement is up; `None` at rest.
    showing: Option<Timer>,
}

/// How long the acknowledgement stays up. Long enough to read, short enough
/// that it is gone before it becomes furniture.
const COPIED_SECONDS: f32 = 1.5;
/// Room set aside for the acknowledgement, wide enough for the longest word it
/// shows. Reserved up front rather than claimed on the click, and mirrored on
/// the other side of the bar, so a copy neither shifts the bar nor pulls it off
/// centre.
pub(super) const COPIED_WIDTH_PX: f32 = 70.0;

/// The commentary under the board.
const MESSAGE_FONT_PX: f32 = 16.0;
/// Pinned rather than left to the font's default so the reserved height below
/// is an exact multiple of it.
const MESSAGE_LINE_PX: f32 = 20.0;
/// How many lines the commentary always occupies. The longest explanation the
/// solver produces fits in three at any window width the game is playable at;
/// a narrower one wraps further and the text is centred over the overflow. The
/// slot holds that height while empty, so the board does not shift when a hint
/// appears.
const MESSAGE_LINES: f32 = 3.0;

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
        // Balances the room reserved for the copy acknowledgement on the far
        // side, so what the player can see stays centred in the bar.
        bar.spawn(Node {
            min_width: Val::Px(COPIED_WIDTH_PX),
            ..default()
        });
        bar.spawn(theme::text(
            format!("{size}x{size}   {rating}"),
            19.0,
            theme::TEXT,
        ));
        // The share code, so a puzzle worth keeping can be played again.
        // Clicking it copies the code: it is easy to mistype and there is a
        // field on the New Game screen waiting for it.
        bar.spawn((
            theme::text(format!("#{}", session.puzzle.seed()), 15.0, theme::TEXT_DIM),
            Node {
                margin: UiRect::left(Val::Px(10.0)),
                ..default()
            },
            TextLayout::no_wrap(),
        ))
        .observe(copy_seed)
        // Nothing else in the bar is clickable, so the label has to say it is.
        .observe(
            |over: On<Pointer<Over>>, mut labels: Query<&mut TextColor>| {
                if let Ok(mut color) = labels.get_mut(over.event_target()) {
                    color.0 = theme::TEXT;
                }
            },
        )
        .observe(|out: On<Pointer<Out>>, mut labels: Query<&mut TextColor>| {
            if let Ok(mut color) = labels.get_mut(out.event_target()) {
                color.0 = theme::TEXT_DIM;
            }
        });

        // Holds its width while empty, so acknowledging a copy cannot nudge
        // the rest of the bar sideways.
        bar.spawn((
            theme::text("", 15.0, theme::ACCENT),
            Node {
                min_width: Val::Px(COPIED_WIDTH_PX),
                // Keeps the word off the seed it belongs to.
                padding: UiRect::left(Val::Px(8.0)),
                ..default()
            },
            TextLayout::no_wrap(),
            SeedCopied::default(),
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

/// Puts the puzzle's share code on the clipboard.
///
/// The code alone: the leading `#` is decoration, and the share code field on
/// the New Game screen takes exactly this, size and difficulty included.
/// Shared with the victory overlay, which offers the same code to copy.
pub(super) fn copy_seed(
    _click: On<Pointer<Click>>,
    session: Res<Session>,
    mut clipboard: ResMut<Clipboard>,
    mut acknowledgement: Query<(&mut Text, &mut SeedCopied)>,
) {
    let share_code = session.puzzle.seed().to_string();
    let said = match clipboard.set_text(share_code) {
        Ok(()) => "copied",
        Err(error) => {
            // A desktop without a clipboard, or one that refused it, is not
            // worth a crash over a convenience.
            warn!("could not copy the seed: {error}");
            "failed"
        }
    };

    for (mut text, mut state) in &mut acknowledgement {
        text.0 = said.to_string();
        state.showing = Some(Timer::from_seconds(COPIED_SECONDS, TimerMode::Once));
    }
}

/// Takes the copy acknowledgement back down once it has had its moment.
pub fn clear_seed_acknowledgement(
    time: Res<Time>,
    mut labels: Query<(&mut Text, &mut SeedCopied)>,
) {
    for (mut text, mut state) in &mut labels {
        let Some(timer) = state.showing.as_mut() else {
            continue;
        };
        if timer.tick(time.delta()).just_finished() {
            text.0.clear();
            state.showing = None;
        }
    }
}

/// The toolbar below the board, plus the running commentary.
pub fn spawn_bottom_bar(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            width: Val::Percent(100.0),
            ..default()
        },))
        .with_children(|column| {
            // The commentary swings between nothing and three lines as hints
            // come and go. Its slot is a fixed height so the toolbar below it
            // and the board above stay put: a board that jumps every time the
            // player asks for a hint is a board they lose their place on.
            column
                .spawn(Node {
                    height: Val::Px(MESSAGE_LINE_PX * MESSAGE_LINES),
                    // Full sentences, kept off the window edges and centred
                    // over the toolbar.
                    width: Val::Percent(74.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|slot| {
                    slot.spawn((
                        Text::default(),
                        // Spelled out rather than built from `theme::text` so
                        // the line height is pinned: the slot above reserves a
                        // whole number of these.
                        TextFont {
                            font_size: FontSize::Px(MESSAGE_FONT_PX),
                            ..default()
                        },
                        LineHeight::Px(MESSAGE_LINE_PX),
                        TextColor(theme::TEXT_DIM),
                        MessageLabel,
                        TextLayout::justify(Justify::Center),
                    ));
                });

            column.spawn(theme::row(10.0)).with_children(|row| {
                row.spawn(theme::small_button("Hint")).observe(
                    |_click: On<Pointer<Click>>,
                     mut session: ResMut<Session>,
                     save: Res<SaveData>| {
                        session.request_hint(theme::region_names(save.settings.colourblind));
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
                None => (String::new(), theme::TEXT_DIM),
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

#[cfg(test)]
mod tests {
    use super::*;
    use queens_core::rating::ALL_RULES;

    /// Widest the message line is at the reference window: 74% of 1024px at
    /// roughly 9.6px per glyph in the built-in font.
    const CHARS_PER_LINE: usize = 78;

    /// The slot is a fixed height, so the wordiest sentence the solver can
    /// produce has to fit it rather than be cut off. Built by hand from the
    /// worst case — a full-size board, the longest colour names, and the
    /// largest set the solver will consider — rather than hunting for it in
    /// generated puzzles, which would only prove that today's seeds are tame.
    #[test]
    fn the_wordiest_hint_fits_the_slot() {
        use queens_core::logic::{Action, Step, Unit};
        use queens_core::{Coord, MAX_SIZE, RuleId};

        let names = theme::region_names(false);
        let longest = |family: fn(u8) -> Unit, count: usize| -> Vec<Unit> {
            let mut units: Vec<Unit> = (0..MAX_SIZE).map(family).collect();
            // The last indices carry the longest names and the two-digit
            // numbers, which is the worst case for either family.
            units.split_off(usize::from(MAX_SIZE) - count)
        };

        for rule in ALL_RULES {
            // Only a locked set reasons about more than one unit a side.
            let group = match rule {
                RuleId::SetElimination => queens_core::logic::MAX_SET_SIZE as usize,
                _ => 1,
            };
            for (subjects, objects) in [
                (longest(Unit::region, group), longest(Unit::column, group)),
                (longest(Unit::column, group), longest(Unit::region, group)),
                (longest(Unit::region, group), longest(Unit::row, group)),
                (longest(Unit::row, group), longest(Unit::region, group)),
            ] {
                let step = Step {
                    rule,
                    action: Action::Eliminate(vec![Coord::new(0, 0)]),
                    subjects,
                    objects,
                };
                let message = step.explain(names);
                let lines = message.len().div_ceil(CHARS_PER_LINE);
                assert!(
                    lines <= MESSAGE_LINES as usize,
                    "{rule:?} runs to {lines} lines ({} chars): {message}",
                    message.len()
                );
                assert!(message.is_ascii(), "{message}");
            }
        }
    }
}
