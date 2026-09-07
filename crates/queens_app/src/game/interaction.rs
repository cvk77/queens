//! Turning clicks, drags and keys into moves.

use bevy::prelude::*;
use queens_core::Mark;

use crate::persistence::SaveData;
use crate::session::Session;
use crate::states::PlayState;

use super::board::Cell;

/// Tracks a drag across the board, so a player can sweep out a run of crosses
/// instead of clicking each cell.
///
/// # Why a sweep waits
/// Bevy starts a drag on the first pixel of movement while a button is held,
/// with no distance threshold, so nearly every real click reports a `DragStart`
/// as well. A stroke therefore records its intent and touches nothing until the
/// pointer leaves the cell it began on, which keeps a wiggle inside one cell
/// indistinguishable from a plain click.
///
/// Acting on `DragStart` directly would collide with the click cycle, since the
/// two disagree about what a crossed cell becomes next: a sweep clears it,
/// while a click advances it to a queen.
#[derive(Resource, Default)]
pub struct PaintStroke {
    stroke: Option<Stroke>,
}

struct Stroke {
    /// The cell the pointer went down on.
    origin: Entity,
    /// What the sweep lays down, decided from `origin` at press time.
    mark: Mark,
    /// Set once the pointer has left `origin`, which is the moment this stops
    /// being a click and becomes a sweep.
    painted: bool,
}

/// Cells a sweep wants marked.
struct Paint {
    mark: Mark,
    /// The cell the pointer just entered.
    cell: Entity,
    /// The cell the sweep began on, handed back exactly once — it was left
    /// untouched until the pointer proved this was a sweep and not a click.
    origin: Option<Entity>,
}

impl PaintStroke {
    /// A press on `origin` has begun moving. Nothing is painted yet.
    fn begin(&mut self, origin: Entity, mark: Mark) {
        self.stroke = Some(Stroke {
            origin,
            mark,
            painted: false,
        });
    }

    /// The pointer entered `cell`. Returns what to paint, if this is a sweep.
    fn extend(&mut self, cell: Entity) -> Option<Paint> {
        let stroke = self.stroke.as_mut()?;
        if cell == stroke.origin {
            // Back where it started; the origin is already handled.
            return None;
        }
        // Hand back the origin on the first cell the sweep reaches, then never
        // again.
        let origin = (!stroke.painted).then_some(stroke.origin);
        stroke.painted = true;
        Some(Paint {
            mark: stroke.mark,
            cell,
            origin,
        })
    }

    /// Whether the gesture that is ending painted, and forgets it either way.
    ///
    /// A sweep that happens to end where it began also reports a click, and
    /// that click is not a click. Bevy emits `Click` before `DragEnd`, so the
    /// stroke is still here to be asked. Clearing it here as well as on
    /// `DragEnd` means a gesture that never gets its `DragEnd` — a cancelled
    /// drag — cannot leave a verdict behind to swallow a later click.
    fn take_painted(&mut self) -> bool {
        self.stroke.take().is_some_and(|stroke| stroke.painted)
    }

    /// The gesture is over.
    fn end(&mut self) {
        self.stroke = None;
    }
}

/// Left click cycles empty → cross → queen; right click drops or lifts a queen
/// directly, which is quicker once the crosses are in.
pub fn on_cell_click(
    click: On<Pointer<Click>>,
    cells: Query<&Cell>,
    play_state: Res<State<PlayState>>,
    save: Res<SaveData>,
    mut stroke: ResMut<PaintStroke>,
    mut session: ResMut<Session>,
) {
    if *play_state.get() != PlayState::Active {
        return;
    }
    // Ask the stroke before anything else, so the verdict is always consumed.
    if stroke.take_painted() {
        return;
    }
    let Ok(cell) = cells.get(click.event_target()) else {
        return;
    };

    let current = session.board.get(cell.coord);
    let next = match click.button {
        PointerButton::Secondary => {
            if current == Mark::Queen {
                Mark::Empty
            } else {
                Mark::Queen
            }
        }
        _ => current.cycled(),
    };
    session.set_mark(cell.coord, next, save.settings.auto_cross);
}

/// Notes what a sweep from this cell would lay down, without touching the
/// board — see [`PaintStroke`] for why it waits.
pub fn on_cell_drag_start(
    drag: On<Pointer<DragStart>>,
    cells: Query<&Cell>,
    play_state: Res<State<PlayState>>,
    session: Res<Session>,
    mut stroke: ResMut<PaintStroke>,
) {
    if *play_state.get() != PlayState::Active || drag.button != PointerButton::Primary {
        return;
    }
    let target = drag.event_target();
    let Ok(cell) = cells.get(target) else {
        return;
    };

    // Sweeping from an empty cell lays crosses; sweeping from a crossed one
    // rubs them out.
    let mark = match session.board.get(cell.coord) {
        Mark::Cross => Mark::Empty,
        _ => Mark::Cross,
    };
    stroke.begin(target, mark);
}

/// Continues a sweep onto each cell the pointer crosses.
pub fn on_cell_drag_enter(
    drag: On<Pointer<DragEnter>>,
    cells: Query<&Cell>,
    play_state: Res<State<PlayState>>,
    mut stroke: ResMut<PaintStroke>,
    mut session: ResMut<Session>,
) {
    if *play_state.get() != PlayState::Active {
        return;
    }
    let target = drag.event_target();
    if !cells.contains(target) {
        return;
    }
    let Some(paint) = stroke.extend(target) else {
        return;
    };

    for entity in paint.origin.into_iter().chain([paint.cell]) {
        let Ok(cell) = cells.get(entity) else {
            continue;
        };
        // Never let a sweep disturb a queen the player put down deliberately.
        if session.board.get(cell.coord) == Mark::Queen {
            continue;
        }
        // A sweep only lays or clears crosses, so auto-cross has no part in it.
        session.set_mark(cell.coord, paint.mark, false);
    }
}

/// Ends a sweep.
pub fn on_cell_drag_end(_drag: On<Pointer<DragEnd>>, mut stroke: ResMut<PaintStroke>) {
    stroke.end();
}

/// Keyboard shortcuts for the toolbar.
pub fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    play_state: Res<State<PlayState>>,
    mut next_play: ResMut<NextState<PlayState>>,
    mut session: ResMut<Session>,
    save: Res<SaveData>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        match play_state.get() {
            PlayState::Active => next_play.set(PlayState::Paused),
            PlayState::Paused => next_play.set(PlayState::Active),
            PlayState::Won => {}
        }
        return;
    }

    if *play_state.get() != PlayState::Active {
        return;
    }

    let control = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);

    if control && keys.just_pressed(KeyCode::KeyZ) {
        if shift {
            session.redo();
        } else {
            session.undo();
        }
    } else if control && keys.just_pressed(KeyCode::KeyY) {
        session.redo();
    } else if keys.just_pressed(KeyCode::KeyH) {
        session.request_hint(crate::theme::region_names(save.settings.colourblind));
    } else if keys.just_pressed(KeyCode::KeyR) && control {
        session.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three distinct entity ids to stand in for cells.
    fn cells() -> (Entity, Entity, Entity) {
        let mut world = World::new();
        (
            world.spawn_empty().id(),
            world.spawn_empty().id(),
            world.spawn_empty().id(),
        )
    }

    /// Bevy reports a drag for the smallest twitch during a click, so a
    /// gesture that never leaves its cell has to paint nothing and let its
    /// click through.
    #[test]
    fn a_press_that_never_leaves_its_cell_is_a_click() {
        let (a, ..) = cells();
        let mut stroke = PaintStroke::default();

        stroke.begin(a, Mark::Cross);
        // Bevy sends Drag events throughout; none of them leave the cell.
        assert!(stroke.extend(a).is_none(), "a wiggle must not paint");
        assert!(
            !stroke.take_painted(),
            "a wiggle must not swallow its own click"
        );
    }

    /// A fast double click is just two of those gestures in a row, so both
    /// clicks have to survive — that is what carries a cell empty → cross →
    /// queen.
    #[test]
    fn repeated_twitchy_presses_all_stay_clicks() {
        let (a, ..) = cells();
        let mut stroke = PaintStroke::default();

        for _ in 0..4 {
            stroke.begin(a, Mark::Cross);
            assert!(!stroke.take_painted());
            stroke.end();
        }
    }

    #[test]
    fn leaving_the_cell_starts_a_sweep_and_paints_the_origin_too() {
        let (a, b, c) = cells();
        let mut stroke = PaintStroke::default();
        stroke.begin(a, Mark::Cross);

        // The first cell the sweep reaches also settles the one it began on,
        // which was deliberately left alone until now.
        let first = stroke.extend(b).expect("leaving the cell is a sweep");
        assert_eq!(first.mark, Mark::Cross);
        assert_eq!(first.cell, b);
        assert_eq!(first.origin, Some(a));

        // The origin is handed back only once.
        let second = stroke.extend(c).expect("the sweep continues");
        assert_eq!(second.cell, c);
        assert_eq!(second.origin, None);
    }

    #[test]
    fn a_sweep_swallows_the_click_it_ends_on() {
        let (a, b, _) = cells();
        let mut stroke = PaintStroke::default();

        stroke.begin(a, Mark::Cross);
        stroke.extend(b);
        // Sweeping back and releasing on the starting cell reports a click,
        // which belongs to the sweep rather than to the player.
        assert!(stroke.extend(a).is_none(), "the origin is already painted");
        assert!(stroke.take_painted());
    }

    #[test]
    fn an_erasing_sweep_carries_its_own_mark() {
        let (a, b, _) = cells();
        let mut stroke = PaintStroke::default();

        stroke.begin(a, Mark::Empty);
        let paint = stroke.extend(b).unwrap();
        assert_eq!(paint.mark, Mark::Empty);
    }

    /// A cancelled drag never gets its `DragEnd`. Its verdict must not linger
    /// and swallow an unrelated click later on.
    #[test]
    fn a_stale_stroke_cannot_swallow_a_later_click() {
        let (a, b, _) = cells();
        let mut stroke = PaintStroke::default();

        stroke.begin(a, Mark::Cross);
        stroke.extend(b);
        assert!(stroke.take_painted(), "the sweep's own click is swallowed");
        // No DragEnd arrives, and the next click is a genuine one.
        assert!(!stroke.take_painted(), "the verdict must not persist");
    }

    #[test]
    fn events_without_a_stroke_are_harmless() {
        let (a, ..) = cells();
        let mut stroke = PaintStroke::default();

        assert!(stroke.extend(a).is_none());
        assert!(!stroke.take_painted());
        stroke.end();
        assert!(!stroke.take_painted());
    }
}
