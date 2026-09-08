//! The How to Play screen: the rules, the controls and the difficulty bands,
//! walked through a page at a time with hand-drawn example boards rather than
//! a played one.

use bevy::prelude::*;
use queens_core::rating::ALL_DIFFICULTIES;

use crate::game::board;
use crate::persistence::SaveData;
use crate::states::{AppState, HowToPlayPage};
use crate::theme;

pub struct HowToPlayPlugin;

impl Plugin for HowToPlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(HowToPlayPage::Goal), spawn_goal_page)
            .add_systems(OnEnter(HowToPlayPage::Touching), spawn_touching_page)
            .add_systems(OnEnter(HowToPlayPage::Controls), spawn_controls_page)
            .add_systems(OnEnter(HowToPlayPage::Hints), spawn_hints_page);
    }
}

/// The pages in the order they are presented, so "Back"/"Next" have something
/// to compute against.
const PAGES: [HowToPlayPage; 4] = [
    HowToPlayPage::Goal,
    HowToPlayPage::Touching,
    HowToPlayPage::Controls,
    HowToPlayPage::Hints,
];

fn page_index(page: HowToPlayPage) -> usize {
    PAGES
        .iter()
        .position(|&candidate| candidate == page)
        .expect("every page is listed in PAGES")
}

/// Every page's shell: a small "How to Play" label, the page's own title, a
/// panel the caller fills in, and the "Back"/"Next" row at the bottom.
fn spawn_page(
    commands: &mut Commands,
    page: HowToPlayPage,
    heading: &str,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    commands
        .spawn(theme::screen(DespawnOnExit(page)))
        .with_children(|screen| {
            screen.spawn(theme::label("How to Play"));
            screen.spawn(theme::title(heading));
            screen.spawn(theme::panel()).with_children(build);
            spawn_nav(screen, page);
        });
}

/// The "Back / page X of N / Next" row every page ends on. The first page's
/// "Back" and the last page's "Next" both return to the main menu instead.
fn spawn_nav(screen: &mut ChildSpawnerCommands, page: HowToPlayPage) {
    let index = page_index(page);

    screen
        .spawn(theme::row(theme::GRID * 3.0))
        .with_children(|row| {
            if index == 0 {
                row.spawn(theme::menu_button("Back to Menu"))
                    .observe(to_main_menu);
            } else {
                let previous = PAGES[index - 1];
                row.spawn(theme::menu_button("Back")).observe(
                    move |_click: On<Pointer<Click>>,
                          mut next: ResMut<NextState<HowToPlayPage>>| {
                        next.set(previous);
                    },
                );
            }

            row.spawn(theme::subtitle(format!("{} / {}", index + 1, PAGES.len())));

            if index + 1 == PAGES.len() {
                row.spawn(theme::accent_button("Done"))
                    .observe(to_main_menu);
            } else {
                let next_page = PAGES[index + 1];
                row.spawn(theme::accent_button("Next")).observe(
                    move |_click: On<Pointer<Click>>,
                          mut next: ResMut<NextState<HowToPlayPage>>| {
                        next.set(next_page);
                    },
                );
            }
        });
}

fn to_main_menu(_click: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::MainMenu);
}

// --- page 1: the goal -------------------------------------------------------

/// A hand-picked 5x5 layout: five contiguous regions, one queen in every row,
/// column and region, no two queens touching. It plays by every rule a
/// generated puzzle does; it is simply never asked to be unique.
const GOAL_REGIONS: [[u8; 5]; 5] = [
    [0, 0, 0, 1, 1],
    [0, 0, 2, 1, 1],
    [0, 2, 2, 1, 3],
    [4, 2, 2, 3, 3],
    [4, 4, 2, 3, 3],
];
const GOAL_QUEENS: [(usize, usize); 5] = [(0, 2), (1, 4), (2, 1), (3, 3), (4, 0)];

fn goal_board(colourblind: bool) -> Vec<Vec<DemoCell>> {
    GOAL_REGIONS
        .iter()
        .enumerate()
        .map(|(r, cols)| {
            cols.iter()
                .enumerate()
                .map(|(c, &region)| {
                    let colour = theme::region_colour(region, colourblind);
                    if GOAL_QUEENS.contains(&(r, c)) {
                        DemoCell::queen(colour)
                    } else {
                        DemoCell::empty(colour)
                    }
                })
                .collect()
        })
        .collect()
}

fn spawn_goal_page(mut commands: Commands, save: Res<SaveData>) {
    let layout = goal_board(save.settings.colourblind);
    spawn_page(&mut commands, HowToPlayPage::Goal, "The Goal", |panel| {
        panel.spawn(theme::subtitle(
            "An n x n board is divided into n coloured regions. Place n queens so that \
             every row, every column and every region holds exactly one.",
        ));
        spawn_demo_board(panel, &layout);
        panel.spawn(theme::subtitle(
            "Every row, every column and every region above holds exactly one queen.",
        ));
    });
}

// --- page 2: no touching -----------------------------------------------------

fn spawn_touching_page(mut commands: Commands) {
    let tile = theme::BUTTON;
    spawn_page(
        &mut commands,
        HowToPlayPage::Touching,
        "No Touching",
        |panel| {
            panel.spawn(theme::subtitle(
                "Two queens can never touch, not even diagonally - but that is the only \
                 restriction. Two queens on the same diagonal are perfectly legal as long \
                 as they are two or more rows apart.",
            ));
            panel
                .spawn(theme::row(theme::GRID * 5.0))
                .with_children(|row| {
                    spawn_labelled_diagram(
                        row,
                        "Legal",
                        theme::SUCCESS,
                        &[
                            vec![
                                DemoCell::queen(tile),
                                DemoCell::empty(tile),
                                DemoCell::empty(tile),
                            ],
                            vec![
                                DemoCell::empty(tile),
                                DemoCell::empty(tile),
                                DemoCell::queen(tile),
                            ],
                        ],
                    );
                    spawn_labelled_diagram(
                        row,
                        "Illegal",
                        theme::DANGER,
                        &[
                            vec![
                                DemoCell::conflicted_queen(tile),
                                DemoCell::empty(tile),
                                DemoCell::empty(tile),
                            ],
                            vec![
                                DemoCell::empty(tile),
                                DemoCell::conflicted_queen(tile),
                                DemoCell::empty(tile),
                            ],
                        ],
                    );
                });
            panel.spawn(theme::subtitle(
                "A queen that breaks a rule is outlined in red the moment you place it.",
            ));
        },
    );
}

fn spawn_labelled_diagram(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    label_colour: Color,
    rows: &[Vec<DemoCell>],
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(theme::GRID),
            ..default()
        })
        .with_children(|column| {
            spawn_demo_board(column, rows);
            column.spawn(theme::text(label.to_uppercase(), 14.0, label_colour));
        });
}

// --- page 3: controls --------------------------------------------------------

/// Matches the modifier `game::interaction::keyboard_shortcuts` actually
/// listens for: Command on macOS, Control everywhere else.
#[cfg(target_os = "macos")]
const UNDO_REDO_KEYS: &str = "Cmd+Z / Cmd+Y";
#[cfg(not(target_os = "macos"))]
const UNDO_REDO_KEYS: &str = "Ctrl+Z / Ctrl+Y";

#[cfg(target_os = "macos")]
const CLEAR_KEY: &str = "Cmd+R";
#[cfg(not(target_os = "macos"))]
const CLEAR_KEY: &str = "Ctrl+R";

const CONTROLS: [(&str, &str); 6] = [
    ("Right click", "Place or lift a queen directly"),
    (
        "Left drag",
        "Sweep crosses, or rub them out from a crossed cell",
    ),
    ("Esc", "Pause and resume"),
    ("H", "Hint"),
    (UNDO_REDO_KEYS, "Undo / redo"),
    (CLEAR_KEY, "Clear the board"),
];

const NAME_COLUMN_PX: f32 = 190.0;

fn spawn_controls_page(mut commands: Commands) {
    let tile = theme::BUTTON;
    spawn_page(
        &mut commands,
        HowToPlayPage::Controls,
        "Controls",
        |panel| {
            panel.spawn(theme::subtitle(
                "Left click cycles a cell: empty, cross, queen, empty.",
            ));
            panel
                .spawn(theme::row(theme::GRID * 4.0))
                .with_children(|row| {
                    for (mark, caption) in [
                        (DemoMark::None, "Empty"),
                        (DemoMark::Cross, "Cross"),
                        (DemoMark::Queen, "Queen"),
                    ] {
                        row.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(theme::GRID),
                            ..default()
                        })
                        .with_children(|column| {
                            spawn_demo_board(
                                column,
                                &[vec![DemoCell {
                                    region: tile,
                                    mark,
                                    conflict: false,
                                }]],
                            );
                            column.spawn(theme::subtitle(caption));
                        });
                    }
                });

            spawn_reference_table(panel, NAME_COLUMN_PX, &CONTROLS);
        },
    );
}

/// A column of [`reference_row`]s that all line up: `align_items: Stretch`
/// widens every row to the column's own width, which is otherwise set by its
/// widest row, so a short row's name still starts at the same left edge as a
/// long one instead of being centred under it independently.
fn spawn_reference_table(panel: &mut ChildSpawnerCommands, name_width: f32, rows: &[(&str, &str)]) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            row_gap: Val::Px(theme::GRID),
            ..default()
        })
        .with_children(|table| {
            for (name, description) in rows {
                table.spawn(reference_row(name, name_width, description));
            }
        });
}

/// A fixed-width name in its own column, so every row's description starts at
/// the same place regardless of how long the name beside it is.
fn reference_row(name: &str, name_width: f32, description: &str) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(theme::GRID * 2.0),
            ..default()
        },
        children![
            (
                Node {
                    width: Val::Px(name_width),
                    ..default()
                },
                children![(theme::text(name, 16.0, theme::TEXT), TextLayout::no_wrap())],
            ),
            theme::subtitle(description),
        ],
    )
}

// --- page 4: hints and difficulty -------------------------------------------

/// The hardest step each band demands, in [`ALL_DIFFICULTIES`] order.
const DIFFICULTY_STEPS: [&str; 4] = [
    "A row, column or region with only one cell left",
    "Confinement, and cells ruled out by every option a region has",
    "A locked set: two or three rows sharing exactly that many columns",
    "Proof by contradiction",
];

const DIFFICULTY_COLUMN_PX: f32 = 100.0;

fn spawn_hints_page(mut commands: Commands) {
    spawn_page(
        &mut commands,
        HowToPlayPage::Hints,
        "Hints and Difficulty",
        |panel| {
            panel.spawn(theme::subtitle(
                "Hints come from the same deductive solver that rates the puzzles, so a \
                 hint is always a step you could have taken yourself.",
            ));
            panel.spawn((
                theme::label("Difficulty"),
                Node {
                    margin: UiRect::top(Val::Px(theme::GRID)),
                    ..default()
                },
            ));
            let bands: Vec<(&str, &str)> = ALL_DIFFICULTIES
                .into_iter()
                .zip(DIFFICULTY_STEPS)
                .map(|(difficulty, step)| (difficulty.name(), step))
                .collect();
            spawn_reference_table(panel, DIFFICULTY_COLUMN_PX, &bands);
        },
    );
}

// --- illustrative boards -----------------------------------------------------

/// One square of a hand-drawn diagram: a region colour, what occupies it, and
/// whether that occupant is shown breaking a rule.
#[derive(Clone, Copy)]
struct DemoCell {
    region: Color,
    mark: DemoMark,
    conflict: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum DemoMark {
    None,
    Queen,
    Cross,
}

impl DemoCell {
    fn empty(region: Color) -> Self {
        Self {
            region,
            mark: DemoMark::None,
            conflict: false,
        }
    }

    fn queen(region: Color) -> Self {
        Self {
            region,
            mark: DemoMark::Queen,
            conflict: false,
        }
    }

    fn conflicted_queen(region: Color) -> Self {
        Self {
            region,
            mark: DemoMark::Queen,
            conflict: true,
        }
    }
}

const DEMO_CELL_PX: f32 = 52.0;

/// Renders a small, static board for illustration. Unlike [`board::spawn_grid`]
/// it takes no [`queens_core::Puzzle`] and needs none, since every diagram
/// here shows a hand-picked layout rather than a played one.
fn spawn_demo_board(parent: &mut ChildSpawnerCommands, rows: &[Vec<DemoCell>]) {
    let columns = rows.first().map_or(0, Vec::len) as u16;
    parent
        .spawn((
            Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::px(columns, DEMO_CELL_PX),
                grid_template_rows: RepeatedGridTrack::px(rows.len() as u16, DEMO_CELL_PX),
                border: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(theme::REGION_EDGE),
            BorderColor::all(theme::REGION_EDGE),
        ))
        .with_children(|grid| {
            for cell in rows.iter().flatten() {
                spawn_demo_cell(grid, *cell);
            }
        });
}

fn spawn_demo_cell(grid: &mut ChildSpawnerCommands, cell: DemoCell) {
    grid.spawn((
        Node {
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(cell.region),
        BorderColor::all(theme::CELL_EDGE),
        Outline {
            width: Val::Px(3.0),
            offset: Val::Px(-3.0),
            color: if cell.conflict {
                theme::DANGER
            } else {
                Color::NONE
            },
        },
    ))
    .with_children(|node| match cell.mark {
        DemoMark::Queen => {
            node.spawn(board::queen_token());
        }
        DemoMark::Cross => {
            node.spawn(board::cross_token());
        }
        DemoMark::None => {}
    });
}
