//! The main menu and the screens reached from it.

use bevy::prelude::*;
use queens_core::{MAX_SIZE, MIN_SIZE, PuzzleSeed, rating::ALL_DIFFICULTIES};

use crate::persistence::SaveData;
use crate::session::PuzzleRequest;
use crate::states::AppState;
use crate::theme;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SeedInput>()
            .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(OnEnter(AppState::NewGame), spawn_new_game)
            .add_systems(OnEnter(AppState::Stats), spawn_stats)
            .add_systems(OnEnter(AppState::Settings), spawn_settings)
            .add_systems(
                Update,
                (
                    highlight_size_options,
                    highlight_difficulty_options,
                    type_seed,
                    show_seed_input,
                )
                    .run_if(in_state(AppState::NewGame)),
            )
            .add_systems(
                Update,
                highlight_toggles.run_if(in_state(AppState::Settings)),
            );
    }
}

/// Sends the player to a screen. Spelled out once so every button reads the
/// same way.
fn go_to(state: AppState) -> impl Fn(On<Pointer<Click>>, ResMut<NextState<AppState>>) {
    move |_click, mut next| next.set(state)
}

// --- main menu -------------------------------------------------------------

fn spawn_main_menu(mut commands: Commands, save: Res<SaveData>) {
    let resumable = save.in_progress.clone();

    commands
        .spawn(theme::screen(DespawnOnExit(AppState::MainMenu)))
        .with_children(|screen| {
            screen.spawn(theme::title("QUEENS"));
            screen.spawn(theme::subtitle(
                "One queen per row, per column and per colour - and no two may touch.",
            ));

            screen.spawn(theme::panel()).with_children(|panel| {
                if let Some(saved) = resumable {
                    let label = format!(
                        "Continue - {0}x{0} {1}",
                        saved.seed.size, saved.seed.difficulty
                    );
                    panel.spawn(theme::accent_button(&label)).observe(
                        move |_click: On<Pointer<Click>>,
                              mut commands: Commands,
                              mut next: ResMut<NextState<AppState>>| {
                            commands.insert_resource(PuzzleRequest::resume(&saved));
                            next.set(AppState::Generating);
                        },
                    );
                }

                panel
                    .spawn(theme::menu_button("New Game"))
                    .observe(go_to(AppState::NewGame));
                panel
                    .spawn(theme::menu_button("Statistics"))
                    .observe(go_to(AppState::Stats));
                panel
                    .spawn(theme::menu_button("Settings"))
                    .observe(go_to(AppState::Settings));
                panel.spawn(theme::menu_button("Quit")).observe(
                    |_click: On<Pointer<Click>>,
                     save: Res<SaveData>,
                     mut exit: MessageWriter<AppExit>| {
                        crate::persistence::flush(&save);
                        exit.write(AppExit::Success);
                    },
                );
            });
        });
}

// --- new game --------------------------------------------------------------

/// Tags a board-size button with the size it selects.
#[derive(Component, Clone, Copy)]
struct SizeOption(u8);

/// Tags a difficulty button with the band it selects.
#[derive(Component, Clone, Copy)]
struct DifficultyOption(queens_core::Difficulty);

fn spawn_new_game(mut commands: Commands) {
    commands
        .spawn(theme::screen(DespawnOnExit(AppState::NewGame)))
        .with_children(|screen| {
            screen.spawn(theme::title("New Game"));

            screen.spawn(theme::panel()).with_children(|panel| {
                panel.spawn(theme::text("Board size", 20.0, theme::TEXT));
                panel.spawn(theme::row(8.0)).with_children(|row| {
                    for size in MIN_SIZE..=MAX_SIZE {
                        row.spawn((theme::small_button(&size.to_string()), SizeOption(size)))
                            .observe(
                                move |_click: On<Pointer<Click>>, mut save: ResMut<SaveData>| {
                                    save.settings.size = size;
                                },
                            );
                    }
                });

                panel.spawn((
                    theme::text("Difficulty", 20.0, theme::TEXT),
                    Node {
                        margin: UiRect::top(Val::Px(12.0)),
                        ..default()
                    },
                ));
                panel.spawn(theme::row(8.0)).with_children(|row| {
                    for difficulty in ALL_DIFFICULTIES {
                        row.spawn((
                            theme::small_button(difficulty.name()),
                            DifficultyOption(difficulty),
                        ))
                        .observe(
                            move |_click: On<Pointer<Click>>, mut save: ResMut<SaveData>| {
                                save.settings.difficulty = difficulty;
                            },
                        );
                    }
                });

                panel.spawn((
                    theme::subtitle(
                        "Difficulty is what the puzzle demands of you, not how big it is:\n\
                         Easy needs only forced cells, Expert needs proof by contradiction.",
                    ),
                    Node {
                        margin: UiRect::vertical(Val::Px(10.0)),
                        ..default()
                    },
                ));

                panel.spawn((
                    theme::text("Seed", 20.0, theme::TEXT),
                    Node {
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                ));
                panel.spawn(theme::row(10.0)).with_children(|row| {
                    row.spawn(seed_field());
                    row.spawn(theme::small_button("Clear")).observe(
                        |_click: On<Pointer<Click>>, mut typed: ResMut<SeedInput>| {
                            typed.digits.clear();
                        },
                    );
                });
                panel.spawn(theme::subtitle(
                    "Type a seed to replay an exact puzzle, or leave it blank for a new one.",
                ));
            });

            screen.spawn(theme::row(12.0)).with_children(|row| {
                row.spawn(theme::menu_button("Back"))
                    .observe(go_to(AppState::MainMenu));
                row.spawn(theme::accent_button("Start")).observe(
                    |_click: On<Pointer<Click>>,
                     mut commands: Commands,
                     save: Res<SaveData>,
                     typed: Res<SeedInput>,
                     mut next: ResMut<NextState<AppState>>| {
                        let (size, difficulty) = (save.settings.size, save.settings.difficulty);
                        let seed = match typed.seed() {
                            Some(seed) => PuzzleSeed::new(size, difficulty, seed),
                            None => PuzzleSeed::random(size, difficulty),
                        };
                        commands.insert_resource(PuzzleRequest::fresh(seed));
                        next.set(AppState::Generating);
                    },
                );
            });
        });
}

// --- typing a seed ---------------------------------------------------------

/// A seed the player is typing on the New Game screen. Empty means "surprise
/// me".
///
/// Rolled by hand rather than with a text widget because the field only ever
/// holds digits, which makes the whole of it a dozen lines and keeps the screen
/// free of focus rules.
#[derive(Resource, Default)]
pub struct SeedInput {
    digits: String,
}

/// Enough for any `u64` seed, including the long ones older saves hold.
const MAX_SEED_DIGITS: usize = 19;

impl SeedInput {
    /// The seed to use, or `None` to pick a fresh one.
    fn seed(&self) -> Option<u64> {
        self.digits.parse().ok()
    }

    fn push_digit(&mut self, digit: char) {
        if self.digits.len() < MAX_SEED_DIGITS {
            self.digits.push(digit);
        }
    }

    fn backspace(&mut self) {
        self.digits.pop();
    }
}

/// The text inside the seed field.
#[derive(Component)]
struct SeedText;

fn seed_field() -> impl Bundle {
    (
        Node {
            width: Val::Px(200.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(theme::BACKGROUND),
        BorderColor::all(theme::PANEL_EDGE),
        children![(
            theme::text("random", 19.0, theme::TEXT_DIM),
            SeedText,
            TextLayout::no_wrap(),
        )],
    )
}

/// Collects digits while the New Game screen is up. There is nothing else on
/// the screen to type into, so the field needs no focus of its own.
fn type_seed(keys: Res<ButtonInput<KeyCode>>, mut typed: ResMut<SeedInput>) {
    const DIGITS: [(KeyCode, char); 20] = [
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
        (KeyCode::Numpad0, '0'),
        (KeyCode::Numpad1, '1'),
        (KeyCode::Numpad2, '2'),
        (KeyCode::Numpad3, '3'),
        (KeyCode::Numpad4, '4'),
        (KeyCode::Numpad5, '5'),
        (KeyCode::Numpad6, '6'),
        (KeyCode::Numpad7, '7'),
        (KeyCode::Numpad8, '8'),
        (KeyCode::Numpad9, '9'),
    ];

    if keys.just_pressed(KeyCode::Backspace) || keys.just_pressed(KeyCode::Delete) {
        typed.backspace();
    }
    for (key, digit) in DIGITS {
        if keys.just_pressed(key) {
            typed.push_digit(digit);
        }
    }
}

fn show_seed_input(
    typed: Res<SeedInput>,
    mut fields: Query<(&mut Text, &mut TextColor), With<SeedText>>,
) {
    if !typed.is_changed() {
        return;
    }
    for (mut label, mut color) in &mut fields {
        if typed.digits.is_empty() {
            label.0 = "random".to_string();
            color.0 = theme::TEXT_DIM;
        } else {
            label.0 = typed.digits.clone();
            color.0 = theme::TEXT;
        }
    }
}

fn highlight_size_options(
    save: Res<SaveData>,
    mut options: Query<(&SizeOption, &mut theme::ButtonTint)>,
) {
    for (option, mut tint) in &mut options {
        let base = selected_tint(option.0 == save.settings.size);
        if tint.base != base {
            tint.base = base;
        }
    }
}

fn highlight_difficulty_options(
    save: Res<SaveData>,
    mut options: Query<(&DifficultyOption, &mut theme::ButtonTint)>,
) {
    for (option, mut tint) in &mut options {
        let base = selected_tint(option.0 == save.settings.difficulty);
        if tint.base != base {
            tint.base = base;
        }
    }
}

fn selected_tint(selected: bool) -> Color {
    if selected {
        theme::ACCENT
    } else {
        theme::BUTTON
    }
}

// --- statistics ------------------------------------------------------------

fn spawn_stats(mut commands: Commands, save: Res<SaveData>) {
    let rows: Vec<(String, String, String, String)> = ALL_DIFFICULTIES
        .into_iter()
        .map(|difficulty| {
            let stats = save.stats_for(difficulty);
            (
                difficulty.name().to_string(),
                format!("{} / {}", stats.solved, stats.started),
                stats
                    .best_seconds
                    .map(theme::format_time)
                    .unwrap_or_else(|| "-".to_string()),
                stats
                    .average_seconds()
                    .map(theme::format_time)
                    .unwrap_or_else(|| "-".to_string()),
            )
        })
        .collect();

    commands
        .spawn(theme::screen(DespawnOnExit(AppState::Stats)))
        .with_children(|screen| {
            screen.spawn(theme::title("Statistics"));

            screen.spawn(theme::panel()).with_children(|panel| {
                panel.spawn(stats_row(
                    "Difficulty",
                    "Solved",
                    "Best",
                    "Average",
                    theme::TEXT_DIM,
                ));
                for (name, solved, best, average) in rows {
                    panel.spawn(stats_row(&name, &solved, &best, &average, theme::TEXT));
                }
            });

            screen
                .spawn(theme::menu_button("Back"))
                .observe(go_to(AppState::MainMenu));
        });
}

/// One line of the statistics table, with fixed column widths so the numbers
/// line up.
fn stats_row(
    difficulty: &str,
    solved: &str,
    best: &str,
    average: &str,
    color: Color,
) -> impl Bundle {
    /// A fixed box holding the text, rather than sizing the text node itself:
    /// glyph metrics differ between "0 / 0" and "Easy", and letting those drive
    /// the box leaves the columns sitting at different heights.
    fn column(content: &str, width: f32, color: Color) -> impl Bundle {
        (
            Node {
                width: Val::Px(width),
                height: Val::Px(28.0),
                align_items: AlignItems::Center,
                ..default()
            },
            children![(
                theme::text(content.to_string(), 18.0, color),
                TextLayout::no_wrap(),
            )],
        )
    }

    (
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        },
        children![
            column(difficulty, 110.0, color),
            column(solved, 90.0, color),
            column(best, 90.0, color),
            column(average, 90.0, color),
        ],
    )
}

// --- settings --------------------------------------------------------------

/// Which preference a toggle button controls.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Toggle {
    AutoCross,
    Colourblind,
}

fn spawn_settings(mut commands: Commands) {
    commands
        .spawn(theme::screen(DespawnOnExit(AppState::Settings)))
        .with_children(|screen| {
            screen.spawn(theme::title("Settings"));

            screen.spawn(theme::panel()).with_children(|panel| {
                spawn_toggle(
                    panel,
                    Toggle::AutoCross,
                    "Auto-cross",
                    "Cross off the cells a queen rules out as soon as you place it.",
                );
                spawn_toggle(
                    panel,
                    Toggle::Colourblind,
                    "Colourblind",
                    "Use region colours chosen to stay separable.",
                );
            });

            screen
                .spawn(theme::menu_button("Back"))
                .observe(go_to(AppState::MainMenu));
        });
}

fn spawn_toggle(panel: &mut ChildSpawnerCommands, toggle: Toggle, label: &str, description: &str) {
    panel
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            children![theme::subtitle(description.to_string())],
        ))
        .with_children(|group| {
            group.spawn((theme::menu_button(label), toggle)).observe(
                move |_click: On<Pointer<Click>>, mut save: ResMut<SaveData>| match toggle {
                    Toggle::AutoCross => {
                        save.settings.auto_cross = !save.settings.auto_cross;
                    }
                    Toggle::Colourblind => {
                        save.settings.colourblind = !save.settings.colourblind;
                    }
                },
            );
        });
}

/// Shows a toggle's state in its colour and its label.
fn highlight_toggles(
    save: Res<SaveData>,
    mut toggles: Query<(&Toggle, &mut theme::ButtonTint, &Children)>,
    mut labels: Query<&mut Text>,
) {
    for (toggle, mut tint, children) in &mut toggles {
        let on = match toggle {
            Toggle::AutoCross => save.settings.auto_cross,
            Toggle::Colourblind => save.settings.colourblind,
        };
        let base = selected_tint(on);
        if tint.base != base {
            tint.base = base;
        }

        let name = match toggle {
            Toggle::AutoCross => "Auto-cross",
            Toggle::Colourblind => "Colourblind",
        };
        let wanted = format!("{name}: {}", if on { "on" } else { "off" });
        for &child in children {
            if let Ok(mut label) = labels.get_mut(child)
                && label.0 != wanted
            {
                label.0 = wanted.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_seed_field_means_pick_one() {
        assert_eq!(SeedInput::default().seed(), None);
    }

    #[test]
    fn typed_digits_become_the_seed() {
        let mut typed = SeedInput::default();
        for digit in "20260905".chars() {
            typed.push_digit(digit);
        }
        assert_eq!(typed.seed(), Some(20_260_905));

        typed.backspace();
        assert_eq!(typed.seed(), Some(2_026_090));
    }

    /// The digit cap has to sit below the point where a seed stops fitting in a
    /// `u64`, or a full field would parse as nothing and silently start a
    /// random puzzle instead.
    #[test]
    fn a_completely_full_field_still_parses() {
        let mut typed = SeedInput::default();
        for _ in 0..MAX_SEED_DIGITS * 2 {
            typed.push_digit('9');
        }
        assert_eq!(typed.digits.len(), MAX_SEED_DIGITS, "capped");
        assert!(typed.seed().is_some(), "{} overflows u64", typed.digits);
    }

    #[test]
    fn backspacing_an_empty_field_is_harmless() {
        let mut typed = SeedInput::default();
        typed.backspace();
        assert_eq!(typed.seed(), None);
    }
}
