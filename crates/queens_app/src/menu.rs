//! The main menu and the screens reached from it.

use bevy::prelude::*;
use queens_core::{MAX_SIZE, MIN_SIZE, PuzzleSeed, rating::ALL_DIFFICULTIES};

use crate::persistence::SaveData;
use crate::session::PuzzleRequest;
use crate::states::AppState;
use crate::theme;
use crate::update_check::LatestRelease;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SeedInput>()
            .init_resource::<ShareCodeInput>()
            .init_resource::<FocusedField>()
            .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(
                Update,
                refresh_update_notice.run_if(in_state(AppState::MainMenu)),
            )
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
                    type_share_code,
                    show_share_code_input,
                    dim_seed_clear_button,
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

fn spawn_main_menu(mut commands: Commands, save: Res<SaveData>, latest: Res<LatestRelease>) {
    let resumable = save.in_progress.clone();

    commands
        .spawn(theme::screen(DespawnOnExit(AppState::MainMenu)))
        .with_children(|screen| {
            screen.spawn(theme::hero("Queens"));

            // Plain column, not `theme::panel()`: four buttons need no card
            // to read as a group, and a card here would draw a border round
            // the first thing the player sees.
            screen
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(theme::GRID * 2.0),
                    ..default()
                })
                .with_children(|panel| {
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
                        .spawn(theme::menu_button("How to Play"))
                        .observe(go_to(AppState::HowToPlay));
                    panel
                        .spawn(theme::menu_button("Statistics"))
                        .observe(go_to(AppState::Stats));
                    panel
                        .spawn(theme::menu_button("Settings"))
                        .observe(go_to(AppState::Settings));
                    // A browser tab is closed by the browser, and quitting the
                    // app there would only leave the player looking at a dead
                    // canvas with no way back.
                    #[cfg(not(target_arch = "wasm32"))]
                    panel.spawn(theme::menu_button("Quit")).observe(
                        |_click: On<Pointer<Click>>,
                         save: Res<SaveData>,
                         mut exit: MessageWriter<AppExit>| {
                            crate::persistence::flush(&save);
                            exit.write(AppExit::Success);
                        },
                    );
                });

            screen.spawn(update_notice_line(latest.0.as_deref()));
            screen.spawn(theme::footnote(COPYRIGHT));
        });
}

/// Marks the text that names a newer release than this build, once
/// [`crate::update_check::UpdateCheckPlugin`] finds one.
#[derive(Component)]
struct UpdateNotice;

/// A quiet line above the copyright line, empty until the update check finds
/// a newer release. Present and reserved from the start either way, so its
/// text changing later never shifts the copyright line beneath it.
fn update_notice_line(newer: Option<&str>) -> impl Bundle {
    let message = newer
        .map(|tag| format!("A new version is available: {tag}"))
        .unwrap_or_default();
    (
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(theme::GRID * 5.5),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            theme::text(message, 13.0, theme::ACCENT),
            UpdateNotice,
            TextLayout::no_wrap(),
        )],
    )
}

/// Fills the notice in once the background check finds a newer release,
/// covering the one case [`spawn_main_menu`] cannot: the check finishing
/// while the player is already looking at this screen.
fn refresh_update_notice(
    latest: Res<LatestRelease>,
    mut notices: Query<&mut Text, With<UpdateNotice>>,
) {
    if !latest.is_changed() {
        return;
    }
    let Some(tag) = &latest.0 else {
        return;
    };
    let message = format!("A new version is available: {tag}");
    for mut text in &mut notices {
        text.0 = message.clone();
    }
}

/// The build and who owns it, pinned to the bottom of the main menu.
///
/// The version comes from the workspace manifest, so a beta tester reporting a
/// bug can read off which build they are on. The embedded Space Grotesk carries
/// a normal Latin set, so the copyright sign and the umlaut render as
/// themselves rather than the ASCII stand-ins the old built-in font needed.
const COPYRIGHT: &str = concat!(
    "Queens Puzzle ",
    env!("CARGO_PKG_VERSION"),
    ", \u{a9} 2026 Christoph von Kr\u{fc}chten"
);

// --- new game --------------------------------------------------------------

/// Tags a board-size button with the size it selects.
#[derive(Component, Clone, Copy)]
struct SizeOption(u8);

/// Tags a difficulty button with the band it selects.
#[derive(Component, Clone, Copy)]
struct DifficultyOption(queens_core::Difficulty);

/// Which of the two typed fields on the New Game screen keystrokes go to.
///
/// Two fields listen for characters now, where the screen used to have only
/// one; without this, a digit typed while composing a share code would land
/// in the seed field too, since nothing else told it not to.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default)]
enum FocusedField {
    #[default]
    ShareCode,
    Seed,
}

/// Points typed characters at one of the two fields. Mirrors [`go_to`].
fn focus_on(field: FocusedField) -> impl Fn(On<Pointer<Click>>, ResMut<FocusedField>) {
    move |_click, mut focus| *focus = field
}

/// Tags the Seed row's "Clear" button, so it can be dimmed along with the
/// field it clears while a share code holds.
#[derive(Component)]
struct SeedClearButton;

fn spawn_new_game(mut commands: Commands) {
    commands
        .spawn(theme::screen(DespawnOnExit(AppState::NewGame)))
        .with_children(|screen| {
            screen.spawn(theme::title("New Game"));

            screen.spawn(theme::panel()).with_children(|panel| {
                panel.spawn(theme::label("Share code"));
                panel.spawn(theme::row(theme::GRID)).with_children(|row| {
                    row.spawn(share_code_field())
                        .observe(focus_on(FocusedField::ShareCode));
                    row.spawn(theme::small_button("Clear")).observe(
                        |_click: On<Pointer<Click>>, mut share: ResMut<ShareCodeInput>| {
                            share.text.clear();
                        },
                    );
                });
                panel.spawn((
                    theme::subtitle(
                        "Paste a share code to replay someone else's puzzle exactly - \
                         it picks the size and difficulty for you.",
                    ),
                    // Extra room below, on top of the panel's own row gap: the
                    // share code is a shortcut that bypasses everything below
                    // it, so it reads better as its own section up top.
                    Node {
                        margin: UiRect::bottom(Val::Px(theme::GRID * 3.0)),
                        ..default()
                    },
                ));

                panel.spawn(theme::label("Board size"));
                panel.spawn(theme::row(theme::GRID)).with_children(|row| {
                    for size in MIN_SIZE..=MAX_SIZE {
                        row.spawn((theme::small_button(&size.to_string()), SizeOption(size)))
                            .observe(
                                move |_click: On<Pointer<Click>>,
                                      mut save: ResMut<SaveData>,
                                      share: Res<ShareCodeInput>| {
                                    if share.decoded().is_none() {
                                        save.settings.size = size;
                                    }
                                },
                            );
                    }
                });

                panel.spawn((
                    theme::label("Difficulty"),
                    Node {
                        margin: UiRect::top(Val::Px(theme::GRID)),
                        ..default()
                    },
                ));
                panel.spawn(theme::row(theme::GRID)).with_children(|row| {
                    for difficulty in ALL_DIFFICULTIES {
                        row.spawn((
                            theme::small_button(difficulty.name()),
                            DifficultyOption(difficulty),
                        ))
                        .observe(
                            move |_click: On<Pointer<Click>>,
                                  mut save: ResMut<SaveData>,
                                  share: Res<ShareCodeInput>| {
                                if share.decoded().is_none() {
                                    save.settings.difficulty = difficulty;
                                }
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
                    theme::label("Seed"),
                    Node {
                        margin: UiRect::top(Val::Px(theme::GRID * 0.5)),
                        ..default()
                    },
                ));
                panel.spawn(theme::row(theme::GRID)).with_children(|row| {
                    row.spawn(seed_field())
                        .observe(focus_on(FocusedField::Seed));
                    row.spawn((theme::small_button("Clear"), SeedClearButton))
                        .observe(
                            |_click: On<Pointer<Click>>,
                             mut typed: ResMut<SeedInput>,
                             share: Res<ShareCodeInput>| {
                                if share.decoded().is_none() {
                                    typed.digits.clear();
                                }
                            },
                        );
                });
                panel.spawn(theme::subtitle(
                    "Type a seed to replay an exact puzzle, or leave it blank for a new one. \
                     Ignored while a share code is set above.",
                ));
            });

            screen.spawn(theme::row(12.0)).with_children(|row| {
                row.spawn(theme::menu_button("Back"))
                    .observe(go_to(AppState::MainMenu));
                row.spawn(theme::accent_button("Start"))
                    .observe(start_puzzle);
            });
        });
}

/// Starts the puzzle the New Game screen currently describes.
fn start_puzzle(
    _click: On<Pointer<Click>>,
    mut commands: Commands,
    mut save: ResMut<SaveData>,
    typed: Res<SeedInput>,
    share: Res<ShareCodeInput>,
    mut next: ResMut<NextState<AppState>>,
) {
    let seed = resolve_puzzle(&mut save.settings, typed.seed(), share.decoded());
    commands.insert_resource(PuzzleRequest::fresh(seed));
    next.set(AppState::Generating);
}

/// Picks the puzzle the New Game screen's fields describe: a decoded share
/// code wins outright; otherwise a typed seed combines with `settings`'
/// size and difficulty, or a fresh seed is picked.
///
/// A share code's size and difficulty are written back into `settings` too,
/// even though its buttons were never clicked: "New Puzzle" on the victory
/// screen and the toolbar's "New" both continue from `settings`, and should
/// carry on with the puzzle just played rather than snap back to whatever
/// was last manually selected.
fn resolve_puzzle(
    settings: &mut crate::persistence::Settings,
    typed_seed: Option<u64>,
    share_code: Option<PuzzleSeed>,
) -> PuzzleSeed {
    match share_code {
        Some(seed) => {
            settings.size = seed.size;
            settings.difficulty = seed.difficulty;
            seed
        }
        None => match typed_seed {
            Some(seed) => PuzzleSeed::new(settings.size, settings.difficulty, seed),
            None => PuzzleSeed::random(settings.size, settings.difficulty),
        },
    }
}

// --- typing a seed ---------------------------------------------------------

/// A seed the player is typing on the New Game screen. Empty means "surprise
/// me".
///
/// Rolled by hand rather than with a text widget because the field only ever
/// holds digits, which makes the whole of it a dozen lines.
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

    /// Replaces the field with the digits found in `text`, dropping anything
    /// else a paste might carry along, such as the leading `#` a copied seed
    /// is shown with.
    fn set_digits(&mut self, text: &str) {
        self.digits = text
            .chars()
            .filter(char::is_ascii_digit)
            .take(MAX_SEED_DIGITS)
            .collect();
    }
}

/// The text inside the seed field.
#[derive(Component)]
struct SeedText;

/// The seed field's own box, so its border can be dimmed while a share code
/// holds it inert, and accented while it holds the keystrokes.
#[derive(Component)]
struct SeedFieldBox;

fn seed_field() -> impl Bundle {
    (
        SeedFieldBox,
        Node {
            width: Val::Px(200.0),
            padding: UiRect::axes(Val::Px(theme::GRID * 1.5), Val::Px(theme::GRID)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(theme::GRID)),
            ..default()
        },
        BackgroundColor(theme::BACKGROUND),
        children![(
            theme::text("random", 19.0, theme::TEXT_DIM),
            SeedText,
            TextLayout::no_wrap(),
        )],
    )
}

/// Collects digits while the New Game screen is up and this field holds
/// focus.
fn type_seed(
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: ResMut<SeedInput>,
    mut clipboard: ResMut<Clipboard>,
    share: Res<ShareCodeInput>,
    focus: Res<FocusedField>,
) {
    if *focus != FocusedField::Seed || share.decoded().is_some() {
        return;
    }

    let paste = keys.just_pressed(KeyCode::KeyV)
        && (keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
            || keys.any_pressed([KeyCode::SuperLeft, KeyCode::SuperRight]));
    if paste {
        if let Some(Ok(text)) = clipboard.fetch_text().poll_result() {
            typed.set_digits(&text);
        }
        return;
    }

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
    share: Res<ShareCodeInput>,
    focus: Res<FocusedField>,
    mut fields: Query<(&mut Text, &mut TextColor), With<SeedText>>,
    mut boxes: Query<&mut BackgroundColor, With<SeedFieldBox>>,
) {
    if !typed.is_changed() && !share.is_changed() && !focus.is_changed() {
        return;
    }
    let locked = share.decoded().is_some();
    for (mut label, mut color) in &mut fields {
        if typed.digits.is_empty() {
            label.0 = "random".to_string();
            color.0 = theme::TEXT_DIM;
        } else {
            label.0 = typed.digits.clone();
            color.0 = if locked { theme::TEXT_DIM } else { theme::TEXT };
        }
    }
    let fill = field_fill(*focus == FocusedField::Seed, locked);
    for mut background in &mut boxes {
        *background = BackgroundColor(fill);
    }
}

// --- typing a share code -----------------------------------------------------

/// A share code the player is typing or has pasted on the New Game screen.
/// Empty, or not yet a complete code, leaves size, difficulty and the seed
/// field to manual selection.
///
/// Rolled by hand like [`SeedInput`], for the same reason: the field only
/// ever holds the characters a share code can, which keeps it small.
#[derive(Resource, Default)]
pub struct ShareCodeInput {
    text: String,
}

/// Two digits for the largest board size, one difficulty letter, and enough
/// digits for any `u64` seed.
const MAX_SHARE_CODE_LEN: usize = 2 + 1 + MAX_SEED_DIGITS;

impl ShareCodeInput {
    /// The puzzle it decodes to, or `None` while the field is empty or
    /// incomplete.
    ///
    /// Visible to [`crate::capture`], which sets a code directly to screenshot
    /// the locked New Game screen without driving real keystrokes.
    pub(crate) fn decoded(&self) -> Option<PuzzleSeed> {
        PuzzleSeed::parse(&self.text)
    }

    fn push_char(&mut self, c: char) {
        if self.text.len() < MAX_SHARE_CODE_LEN {
            self.text.push(c);
        }
    }

    fn backspace(&mut self) {
        self.text.pop();
    }

    /// Replaces the field with the alphanumerics found in `text`, upper-cased
    /// and capped, dropping anything else a paste might carry along.
    pub(crate) fn set_text(&mut self, text: &str) {
        self.text = text
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
            .take(MAX_SHARE_CODE_LEN)
            .collect();
    }
}

/// The text inside the share code field.
#[derive(Component)]
struct ShareCodeText;

/// The share code field's own box, so its border can show which field holds
/// focus.
#[derive(Component)]
struct ShareCodeFieldBox;

fn share_code_field() -> impl Bundle {
    (
        ShareCodeFieldBox,
        Node {
            width: Val::Px(200.0),
            padding: UiRect::axes(Val::Px(theme::GRID * 1.5), Val::Px(theme::GRID)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(theme::GRID)),
            ..default()
        },
        BackgroundColor(theme::BACKGROUND),
        children![(
            theme::text("none", 19.0, theme::TEXT_DIM),
            ShareCodeText,
            TextLayout::no_wrap(),
        )],
    )
}

/// Collects a share code while the New Game screen is up and this field holds
/// focus.
fn type_share_code(
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: ResMut<ShareCodeInput>,
    mut clipboard: ResMut<Clipboard>,
    focus: Res<FocusedField>,
) {
    if *focus != FocusedField::ShareCode {
        return;
    }

    let paste = keys.just_pressed(KeyCode::KeyV)
        && (keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
            || keys.any_pressed([KeyCode::SuperLeft, KeyCode::SuperRight]));
    if paste {
        if let Some(Ok(text)) = clipboard.fetch_text().poll_result() {
            typed.set_text(&text);
        }
        return;
    }

    const CHARS: [(KeyCode, char); 14] = [
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
        (KeyCode::KeyE, 'E'),
        (KeyCode::KeyM, 'M'),
        (KeyCode::KeyH, 'H'),
        (KeyCode::KeyX, 'X'),
    ];

    if keys.just_pressed(KeyCode::Backspace) || keys.just_pressed(KeyCode::Delete) {
        typed.backspace();
    }
    for (key, c) in CHARS {
        if keys.just_pressed(key) {
            typed.push_char(c);
        }
    }
}

fn show_share_code_input(
    typed: Res<ShareCodeInput>,
    focus: Res<FocusedField>,
    mut fields: Query<&mut Text, With<ShareCodeText>>,
    mut boxes: Query<&mut BackgroundColor, With<ShareCodeFieldBox>>,
) {
    if !typed.is_changed() && !focus.is_changed() {
        return;
    }
    for mut label in &mut fields {
        label.0 = if typed.text.is_empty() {
            "none".to_string()
        } else {
            typed.text.clone()
        };
    }
    // Never locked/disabled itself, unlike the seed field.
    let fill = field_fill(*focus == FocusedField::ShareCode, false);
    for mut background in &mut boxes {
        *background = BackgroundColor(fill);
    }
}

/// A field's fill: dimmed while a share code holds it inert, lit with the
/// accent while it holds the player's keystrokes, and the plain background
/// otherwise. A flat fill rather than a focus ring around it.
fn field_fill(focused: bool, locked: bool) -> Color {
    if locked {
        theme::DISABLED
    } else if focused {
        theme::FIELD_FOCUS
    } else {
        theme::BACKGROUND
    }
}

/// Dims a button label the same way its background dims, so a locked row
/// does not read as fully lit but for one highlighted option.
fn label_tint(selected: bool, locked: bool) -> Color {
    if !selected && locked {
        theme::TEXT_DIM
    } else {
        theme::TEXT
    }
}

/// Dims the Seed row's own "Clear" button while a share code makes it a
/// no-op, matching the field it belongs to.
fn dim_seed_clear_button(
    share: Res<ShareCodeInput>,
    mut buttons: Query<(&mut theme::ButtonTint, &Children), With<SeedClearButton>>,
    mut labels: Query<&mut TextColor>,
) {
    if !share.is_changed() {
        return;
    }
    let locked = share.decoded().is_some();
    let base = if locked {
        theme::DISABLED
    } else {
        theme::BUTTON
    };
    let text_color = if locked { theme::TEXT_DIM } else { theme::TEXT };
    for (mut tint, children) in &mut buttons {
        if tint.base != base {
            tint.base = base;
        }
        dim_label(children, &mut labels, text_color);
    }
}

fn highlight_size_options(
    save: Res<SaveData>,
    share: Res<ShareCodeInput>,
    mut options: Query<(&SizeOption, &mut theme::ButtonTint, &Children)>,
    mut labels: Query<&mut TextColor>,
) {
    let locked = share.decoded();
    let selected_size = locked.map_or(save.settings.size, |seed| seed.size);
    for (option, mut tint, children) in &mut options {
        let selected = option.0 == selected_size;
        let base = option_tint(selected, locked.is_some());
        if tint.base != base {
            tint.base = base;
        }
        dim_label(
            children,
            &mut labels,
            label_tint(selected, locked.is_some()),
        );
    }
}

fn highlight_difficulty_options(
    save: Res<SaveData>,
    share: Res<ShareCodeInput>,
    mut options: Query<(&DifficultyOption, &mut theme::ButtonTint, &Children)>,
    mut labels: Query<&mut TextColor>,
) {
    let locked = share.decoded();
    let selected_difficulty = locked.map_or(save.settings.difficulty, |seed| seed.difficulty);
    for (option, mut tint, children) in &mut options {
        let selected = option.0 == selected_difficulty;
        let base = option_tint(selected, locked.is_some());
        if tint.base != base {
            tint.base = base;
        }
        dim_label(
            children,
            &mut labels,
            label_tint(selected, locked.is_some()),
        );
    }
}

/// Sets every text child's colour, for the buttons whose label is a single
/// child with no marker of its own to query by.
fn dim_label(children: &Children, labels: &mut Query<&mut TextColor>, color: Color) {
    for &child in children {
        if let Ok(mut label_color) = labels.get_mut(child)
            && label_color.0 != color
        {
            label_color.0 = color;
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

/// Like [`selected_tint`], but dims every option that is not the one a share
/// code decoded to, since none of them can be clicked while it holds.
fn option_tint(selected: bool, locked: bool) -> Color {
    match (selected, locked) {
        (true, _) => theme::ACCENT,
        (false, true) => theme::DISABLED,
        (false, false) => theme::BUTTON,
    }
}

// --- statistics ------------------------------------------------------------

fn spawn_stats(mut commands: Commands, save: Res<SaveData>) {
    let rows: Vec<[String; 5]> = ALL_DIFFICULTIES
        .into_iter()
        .map(|difficulty| {
            let stats = save.stats_for(difficulty);
            [
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
                hints_cell(stats),
            ]
        })
        .collect();

    commands
        .spawn(theme::screen(DespawnOnExit(AppState::Stats)))
        .with_children(|screen| {
            screen.spawn(theme::title("Statistics"));

            screen.spawn(theme::panel()).with_children(|panel| {
                panel.spawn(stats_header_row([
                    "Difficulty",
                    "Solved",
                    "Best",
                    "Average",
                    "Hints",
                ]));
                for row in &rows {
                    panel.spawn(stats_row(std::array::from_fn(|i| row[i].as_str())));
                }
            });

            // Which puzzles the hint count covers, which a column heading has
            // no room to say.
            screen.spawn(theme::subtitle(
                "Hints counts those spent on the puzzles you went on to solve.",
            ));

            screen
                .spawn(theme::menu_button("Back"))
                .observe(go_to(AppState::MainMenu));
        });
}

/// The hints column. A dash rather than a zero where nothing has been solved,
/// matching the time columns: no solves is not the same as a clean sheet.
fn hints_cell(stats: &crate::persistence::DifficultyStats) -> String {
    if stats.solved == 0 {
        return "-".to_string();
    }
    stats.hints_used.to_string()
}

/// Wide enough for the longest cell each column can hold.
const STATS_WIDTHS: [f32; 5] = [110.0, 90.0, 90.0, 90.0, 90.0];

/// A fixed box holding the text, rather than sizing the text node itself:
/// glyph metrics differ between "0 / 0" and "Easy", and letting those drive
/// the box leaves the columns sitting at different heights.
fn stats_column(content: impl Bundle, width: f32) -> impl Bundle {
    (
        Node {
            width: Val::Px(width),
            height: Val::Px(theme::GRID * 3.5),
            align_items: AlignItems::Center,
            ..default()
        },
        children![(content, TextLayout::no_wrap())],
    )
}

fn stats_row_node() -> impl Bundle {
    Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(theme::GRID),
        ..default()
    }
}

/// The column headings, as structural labels rather than data.
fn stats_header_row(cells: [&str; 5]) -> impl Bundle {
    (
        stats_row_node(),
        Children::spawn(SpawnIter(
            cells
                .into_iter()
                .zip(STATS_WIDTHS)
                .map(|(content, width)| stats_column(theme::label(content), width))
                .collect::<Vec<_>>()
                .into_iter(),
        )),
    )
}

/// One line of the statistics table: the difficulty's name, then four numbers
/// set with tabular figures so every column lines up.
fn stats_row(cells: [&str; 5]) -> impl Bundle {
    let [name, rest @ ..] = cells;
    (
        stats_row_node(),
        Children::spawn((
            Spawn(stats_column(
                theme::text(name.to_string(), 18.0, theme::TEXT),
                STATS_WIDTHS[0],
            )),
            SpawnIter(
                rest.into_iter()
                    .zip(&STATS_WIDTHS[1..])
                    .map(|(content, &width)| {
                        stats_column(
                            theme::numeric(content.to_string(), 18.0, theme::TEXT),
                            width,
                        )
                    })
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
        )),
    )
}

// --- settings --------------------------------------------------------------

/// Which preference a toggle button controls.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Toggle {
    AutoCross,
    Colourblind,
    Sound,
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
                spawn_toggle(
                    panel,
                    Toggle::Sound,
                    "Sound",
                    "Play a cue as you mark the board, and on a solve.",
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
                    Toggle::Sound => {
                        save.settings.sound = !save.settings.sound;
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
            Toggle::Sound => save.settings.sound,
        };
        let base = selected_tint(on);
        if tint.base != base {
            tint.base = base;
        }

        let name = match toggle {
            Toggle::AutoCross => "Auto-cross",
            Toggle::Colourblind => "Colourblind",
            Toggle::Sound => "Sound",
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

    /// The version number is what a bug report actually needs out of this
    /// line, however the name around it is spelled.
    #[test]
    fn the_copyright_line_carries_the_version() {
        assert!(COPYRIGHT.contains(env!("CARGO_PKG_VERSION")));
    }

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

    /// A pasted seed is copied with a leading `#`, and may land on top of
    /// digits the player already typed; both have to be handled.
    #[test]
    fn pasting_replaces_the_field_with_the_digits_it_carries() {
        let mut typed = SeedInput::default();
        typed.push_digit('1');
        typed.set_digits("#20260905");
        assert_eq!(typed.seed(), Some(20_260_905));
    }

    #[test]
    fn a_pasted_seed_is_capped_like_a_typed_one() {
        let mut typed = SeedInput::default();
        typed.set_digits(&"9".repeat(MAX_SEED_DIGITS * 2));
        assert_eq!(typed.digits.len(), MAX_SEED_DIGITS, "capped");
    }

    #[test]
    fn an_empty_share_code_field_decodes_to_nothing() {
        assert_eq!(ShareCodeInput::default().decoded(), None);
    }

    #[test]
    fn a_full_share_code_decodes_to_its_puzzle_seed() {
        let mut typed = ShareCodeInput::default();
        typed.set_text("10H392854");
        assert_eq!(
            typed.decoded(),
            Some(PuzzleSeed::new(10, queens_core::Difficulty::Hard, 392_854))
        );
    }

    /// While the code is still being typed or pasted, size and difficulty
    /// stay on manual selection rather than locking onto a partial guess.
    #[test]
    fn an_incomplete_share_code_does_not_decode() {
        let mut typed = ShareCodeInput::default();
        typed.set_text("10H");
        assert_eq!(typed.decoded(), None);
    }

    /// A share code is meant to be pasted, and a clumsy paste may carry stray
    /// punctuation or the wrong case along with it.
    #[test]
    fn pasting_strips_punctuation_and_upper_cases_the_code() {
        let mut typed = ShareCodeInput::default();
        typed.set_text("10h-392854");
        assert_eq!(
            typed.decoded(),
            Some(PuzzleSeed::new(10, queens_core::Difficulty::Hard, 392_854))
        );
    }

    #[test]
    fn a_pasted_share_code_is_capped() {
        let mut typed = ShareCodeInput::default();
        typed.set_text(&"9".repeat(MAX_SHARE_CODE_LEN * 2));
        assert_eq!(typed.text.len(), MAX_SHARE_CODE_LEN, "capped");
    }

    /// The two fields used to be driven by two systems that both watched
    /// every digit key, gated only by whether a code had *already* fully
    /// decoded. Typing a digit that completed a code (the last key of "5E1")
    /// landed in both fields on the same frame, since the seed field's system
    /// still saw the pre-keystroke, not-yet-decoded state: the size digit and
    /// the seed digit both leaked in, turning a seed of `1` into `51`. Real
    /// focus, checked by both systems before either touches its own field,
    /// removes the shared state that raced.
    #[test]
    fn typing_a_share_code_never_touches_the_seed_field() {
        use bevy::ecs::system::RunSystemOnce;

        let mut world = World::new();
        world.init_resource::<SeedInput>();
        world.init_resource::<ShareCodeInput>();
        world.init_resource::<FocusedField>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<Clipboard>();

        // "5", "E", "1" pressed one at a time, each its own frame: both
        // systems run every frame, exactly as they do in the real Update
        // schedule.
        for key in [KeyCode::Digit5, KeyCode::KeyE, KeyCode::Digit1] {
            world.resource_mut::<ButtonInput<KeyCode>>().press(key);
            world.run_system_once(type_seed).unwrap();
            world.run_system_once(type_share_code).unwrap();
            world.resource_mut::<ButtonInput<KeyCode>>().clear();
        }

        assert_eq!(world.resource::<SeedInput>().digits, "");
        assert_eq!(
            world.resource::<ShareCodeInput>().decoded(),
            Some(PuzzleSeed::new(5, queens_core::Difficulty::Easy, 1))
        );
    }

    /// Starting a share-code puzzle bypasses the size/difficulty buttons, but
    /// must still leave `settings` describing the puzzle that was just
    /// started, or "New Puzzle" on the victory screen and the toolbar's "New"
    /// would snap back to whatever was last manually selected instead of
    /// continuing from it.
    #[test]
    fn starting_a_share_code_puzzle_updates_settings_to_match() {
        let mut settings = crate::persistence::Settings::default();
        let code = PuzzleSeed::new(5, queens_core::Difficulty::Easy, 1);

        let started = resolve_puzzle(&mut settings, None, Some(code));

        assert_eq!(started, code);
        assert_eq!(settings.size, 5);
        assert_eq!(settings.difficulty, queens_core::Difficulty::Easy);
    }

    #[test]
    fn a_typed_seed_combines_with_the_current_settings() {
        let mut settings = crate::persistence::Settings {
            size: 9,
            difficulty: queens_core::Difficulty::Hard,
            ..crate::persistence::Settings::default()
        };

        let started = resolve_puzzle(&mut settings, Some(42), None);

        assert_eq!(
            started,
            PuzzleSeed::new(9, queens_core::Difficulty::Hard, 42)
        );
    }

    /// With neither a share code nor a typed seed, a fresh puzzle still has
    /// to match the current settings, not just any size and difficulty.
    #[test]
    fn no_seed_at_all_still_matches_the_current_settings() {
        let mut settings = crate::persistence::Settings {
            size: 11,
            difficulty: queens_core::Difficulty::Expert,
            ..crate::persistence::Settings::default()
        };

        let started = resolve_puzzle(&mut settings, None, None);

        assert_eq!(started.size, 11);
        assert_eq!(started.difficulty, queens_core::Difficulty::Expert);
    }
}
