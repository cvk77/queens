//! Drawing the board: a `Display::Grid` of cell entities, and keeping them in
//! step with the session.

use bevy::prelude::*;
use queens_core::{ALL_SIDES, Coord, HintKind, Mark, Puzzle, Side};

use crate::persistence::SaveData;
use crate::session::Session;
use crate::states::PlayState;
use crate::theme;

/// Board width as a share of the smaller window dimension, leaving room for the
/// bars above and below.
const BOARD_VMIN: f32 = 66.0;
/// Bold line between two regions.
const REGION_BORDER_PX: f32 = 2.0;
/// Hairline between two cells of the same region.
const CELL_BORDER_PX: f32 = 1.0;
/// The frame drawn around the whole grid.
const BOARD_BORDER_PX: f32 = 3.0;
/// Space between the board and the numbers running alongside it.
const RULER_GAP_PX: f32 = 6.0;
/// Small enough to read as an annotation rather than part of the puzzle.
const RULER_FONT_PX: f32 = 13.0;

/// One square of the board.
#[derive(Component, Clone, Copy)]
pub struct Cell {
    pub coord: Coord,
}

/// The queen token inside a cell.
#[derive(Component)]
pub(crate) struct QueenMark;

/// The player's cross inside a cell.
#[derive(Component)]
pub(crate) struct CrossMark;

/// Builds the board — the ruler of row and column numbers, the grid, and every
/// cell inside it — as a child of `parent`.
///
/// The whole thing is one `Display::Grid` so the rulers cannot drift out of
/// step with the cells: `auto` tracks take their size from the board node
/// itself.
pub fn spawn_grid(parent: &mut ChildSpawnerCommands, session: &Session, save: &SaveData) {
    let puzzle = &session.puzzle;
    let size = puzzle.size();

    parent
        .spawn(Node {
            display: Display::Grid,
            grid_template_columns: vec![GridTrack::auto(), GridTrack::auto()],
            grid_template_rows: vec![GridTrack::auto(), GridTrack::auto()],
            column_gap: Val::Px(RULER_GAP_PX),
            row_gap: Val::Px(RULER_GAP_PX),
            ..default()
        })
        .with_children(|frame| {
            // The corner between the two rulers stays empty.
            frame.spawn(Node {
                grid_column: GridPlacement::start(1),
                grid_row: GridPlacement::start(1),
                ..default()
            });
            frame.spawn(ruler(size, false));
            frame.spawn(ruler(size, true));

            frame
                .spawn((
                    Node {
                        grid_column: GridPlacement::start(2),
                        grid_row: GridPlacement::start(2),
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::flex(u16::from(size), 1.0),
                        grid_template_rows: RepeatedGridTrack::flex(u16::from(size), 1.0),
                        width: Val::VMin(BOARD_VMIN),
                        height: Val::VMin(BOARD_VMIN),
                        border: UiRect::all(Val::Px(BOARD_BORDER_PX)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(theme::REGION_EDGE),
                    BorderColor::all(theme::REGION_EDGE),
                ))
                .with_children(|grid| {
                    for coord in puzzle.cells() {
                        spawn_cell(grid, puzzle, coord, save);
                    }
                });
        });
}

/// A strip of 1-based numbers running alongside the board, so a hint naming
/// "row 3" or "column 8" points somewhere the player can actually count to.
///
/// The board's own border sits inside its width, so the strip is padded by that
/// much to keep each number centred on its line.
fn ruler(size: u8, vertical: bool) -> impl Bundle {
    let tracks = RepeatedGridTrack::flex(u16::from(size), 1.0);
    let border = Val::Px(BOARD_BORDER_PX);
    let (node, labels) = if vertical {
        (
            Node {
                grid_column: GridPlacement::start(1),
                grid_row: GridPlacement::start(2),
                display: Display::Grid,
                grid_template_rows: tracks,
                justify_items: JustifyItems::End,
                align_items: AlignItems::Center,
                padding: UiRect::vertical(border),
                ..default()
            },
            size,
        )
    } else {
        (
            Node {
                grid_column: GridPlacement::start(2),
                grid_row: GridPlacement::start(1),
                display: Display::Grid,
                grid_template_columns: tracks,
                justify_items: JustifyItems::Center,
                align_items: AlignItems::End,
                padding: UiRect::horizontal(border),
                ..default()
            },
            size,
        )
    };

    (
        node,
        Children::spawn(SpawnIter(
            (1..=labels).map(|n| theme::text(n.to_string(), RULER_FONT_PX, theme::TEXT_DIM)),
        )),
    )
}

fn spawn_cell(grid: &mut ChildSpawnerCommands, puzzle: &Puzzle, coord: Coord, save: &SaveData) {
    // Each cell draws its own borders, so a region boundary ends up drawn from
    // both sides — which is exactly the heavy divider the puzzle needs.
    let width = |side: Side| {
        Val::Px(if puzzle.is_region_boundary(coord, side) {
            REGION_BORDER_PX
        } else {
            CELL_BORDER_PX
        })
    };
    let color = |side: Side| {
        if puzzle.is_region_boundary(coord, side) {
            theme::REGION_EDGE
        } else {
            theme::CELL_EDGE
        }
    };
    let [top, right, bottom, left] = ALL_SIDES;

    grid.spawn((
        Cell { coord },
        Button,
        Node {
            border: UiRect {
                top: width(top),
                right: width(right),
                bottom: width(bottom),
                left: width(left),
            },
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(theme::region_colour(
            puzzle.region_at(coord),
            save.settings.colourblind,
        )),
        BorderColor {
            top: color(top),
            right: color(right),
            bottom: color(bottom),
            left: color(left),
        },
        // Kept on every cell rather than inserted on demand: toggling a colour
        // is far cheaper than adding and removing a component each move.
        Outline {
            width: Val::Px(3.0),
            offset: Val::Px(-3.0),
            color: Color::NONE,
        },
        children![queen_mark(), cross_mark()],
    ))
    .observe(super::interaction::on_cell_click)
    .observe(super::interaction::on_cell_drag_start)
    .observe(super::interaction::on_cell_drag_enter)
    .observe(super::interaction::on_cell_drag_end);
}

/// The queen: a dark disc with a light crown set into it, drawn from plain UI
/// nodes so the game needs no art assets and no font that happens to carry a
/// chess glyph. The disc is what keeps the crown legible whichever region
/// colour the queen lands on.
fn queen_mark() -> impl Bundle {
    (
        QueenMark,
        Node {
            display: Display::None,
            width: Val::Percent(68.0),
            height: Val::Percent(68.0),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::MAX,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(theme::QUEEN_BODY),
        BorderColor::all(theme::QUEEN_RING),
        children![crown()],
    )
}

/// Where the band's top edge sits, as a share of the crown's box. Every point
/// is centred on this line, so the band hides their side corners and each one
/// is left as a clean triangle, notched from its neighbours.
const BAND_TOP: f32 = 48.0;
/// How far down the band reaches from [`BAND_TOP`].
const BAND_HEIGHT: f32 = 30.0;

/// The crown, in its own square box so every piece can be placed as a share of
/// it and the whole thing scales with the cell.
///
/// The three points are squares turned on end. Children are painted in order,
/// so the band goes on top of their lower halves and leaves just the tips
/// standing — which is a lot less fiddly than trying to describe the same
/// silhouette as one shape.
fn crown() -> impl Bundle {
    (
        // Sized so the widest part of the crown still clears the disc's ring.
        Node {
            width: Val::Percent(78.0),
            height: Val::Percent(78.0),
            ..default()
        },
        children![
            crown_point(20.0, 28.0),
            crown_point(50.0, 37.0),
            crown_point(80.0, 28.0),
            crown_band(),
            crown_gem(),
        ],
    )
}

/// One point of the crown: a square standing on a corner, given the `x` its tip
/// lines up with and the side of the square. The taller centre point is simply
/// the larger square — a square on its corner stands `side * √2 / 2` tall, so
/// the sides are what set how far each tip clears the band.
fn crown_point(x: f32, side: f32) -> impl Bundle {
    let half = side / 2.0;
    (
        // Rotation is about the node's centre, so placing the square centred on
        // the band's top edge is what puts its side corners out of sight.
        crown_piece(x - half, BAND_TOP - half, side, side, BorderRadius::ZERO),
        UiTransform::from_rotation(Rot2::degrees(45.0)),
    )
}

/// The band the points rise out of.
fn crown_band() -> impl Bundle {
    crown_piece(
        0.0,
        BAND_TOP,
        100.0,
        BAND_HEIGHT,
        BorderRadius::bottom(Val::Percent(28.0)),
    )
}

/// The gem set into the middle of the band.
fn crown_gem() -> impl Bundle {
    let side = 18.0;
    let top = BAND_TOP + (BAND_HEIGHT - side) / 2.0;
    tinted_piece(
        50.0 - side / 2.0,
        top,
        side,
        side,
        BorderRadius::MAX,
        theme::QUEEN_GEM,
    )
}

/// A rectangle of the crown, positioned as a share of the crown's box. Absolute
/// positioning throughout: the pieces are meant to overlap, which is the one
/// thing flow layout will not do.
fn crown_piece(
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    border_radius: BorderRadius,
) -> impl Bundle {
    tinted_piece(left, top, width, height, border_radius, theme::QUEEN_CROWN)
}

/// [`crown_piece`] in a colour of its own, for the gem — the one piece that is
/// not part of the crown's silhouette.
fn tinted_piece(
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    border_radius: BorderRadius,
    color: Color,
) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(left),
            top: Val::Percent(top),
            width: Val::Percent(width),
            height: Val::Percent(height),
            border_radius,
            ..default()
        },
        BackgroundColor(color),
    )
}

/// The player's cross: two bars laid over each other. Rotating bars rather than
/// tilting a glyph gives both strokes the same weight and properly rounded
/// ends, and it scales with the cell without any per-board font sizing.
fn cross_mark() -> impl Bundle {
    (
        CrossMark,
        Node {
            display: Display::None,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        children![cross_bar(45.0), cross_bar(-45.0)],
    )
}

/// One stroke of the cross, inset so that it sits centred in the cell.
fn cross_bar(degrees: f32) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(24.0),
            top: Val::Percent(45.0),
            width: Val::Percent(52.0),
            height: Val::Percent(10.0),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(theme::CROSS),
        UiTransform::from_rotation(Rot2::degrees(degrees)),
    )
}

/// Redraws marks, conflicts and hints whenever the session or the palette
/// changes.
///
/// Marks are shown by switching a node between `Display::None` and `Flex`
/// rather than by toggling `Visibility`: a hidden node still takes up layout
/// space, which would knock the visible mark off centre.
pub fn refresh_board(
    session: Res<Session>,
    save: Res<SaveData>,
    play_state: Res<State<PlayState>>,
    mut cells: Query<(&Cell, &Children, &mut Outline, &mut BackgroundColor)>,
    mut marks: Query<(&mut Node, Has<QueenMark>, Has<CrossMark>)>,
) {
    if !session.is_changed() && !save.is_changed() && !play_state.is_changed() {
        return;
    }

    // Pausing stops the clock, so it must not double as a way to study the
    // board. The regions stay, since they are the puzzle itself, but every mark
    // the player has made is hidden until they resume.
    let concealed = *play_state.get() == PlayState::Paused;

    let hint = session.hint.as_ref();
    // A deduction usually clears several cells at once; showing them all is
    // what makes the sentence explaining it check out.
    let hint_cells = hint.map(|hint| hint.cells()).unwrap_or_default();
    // Colour says what the hint wants: green to place a queen, red for a mark
    // that does not belong, blue for a cell to cross off.
    let hint_colour = match hint.map(|hint| &hint.kind) {
        Some(HintKind::Place(_)) => theme::SUCCESS,
        Some(HintKind::IncorrectQueen(_) | HintKind::IncorrectCross(_)) => theme::DANGER,
        _ => theme::ACCENT,
    };

    for (cell, children, mut outline, mut background) in &mut cells {
        let mark = session.board.get(cell.coord);

        outline.color = if concealed {
            Color::NONE
        } else if session.conflicts.contains(&cell.coord) {
            theme::DANGER
        } else if hint_cells.contains(&cell.coord) {
            hint_colour
        } else {
            Color::NONE
        };

        background.0 = theme::region_colour(
            session.puzzle.region_at(cell.coord),
            save.settings.colourblind,
        );

        for &child in children {
            let Ok((mut node, is_queen, is_cross)) = marks.get_mut(child) else {
                continue;
            };
            let shown = !concealed
                && ((is_queen && mark == Mark::Queen) || (is_cross && mark == Mark::Cross));
            let wanted = if shown { Display::Flex } else { Display::None };
            if node.display != wanted {
                node.display = wanted;
            }
        }
    }
}
