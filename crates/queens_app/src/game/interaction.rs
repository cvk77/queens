//! Turning clicks, drags and keys into moves.

use bevy::input::keyboard::Key;
use bevy::prelude::*;
use queens_core::{Coord, Mark};

use crate::audio::Sound;
use crate::persistence::SaveData;
use crate::session::Session;
use crate::states::PlayState;

use super::board::Cell;

/// How far the pointer has to travel from the press before the gesture is a
/// sweep rather than a click.
///
/// A finger flattening on a trackpad drags the pointer a pixel or two, and a
/// tap that lands near a cell edge carries it into the neighbour. Bevy only
/// sends `Click` to an entity the pointer is still over, so without a
/// threshold that tap is lost twice over: no click, and a stripe of crosses
/// the player never asked for.
const SWEEP_THRESHOLD_PX: f32 = 6.0;

/// Tracks a drag across the board, so a player can sweep out a run of crosses
/// instead of clicking each cell.
///
/// # Why a sweep waits
/// Bevy starts a drag on the first pixel of movement while a button is held,
/// with no distance threshold, so nearly every real click reports a `DragStart`
/// as well. A stroke therefore records its intent and touches nothing until the
/// pointer has both left the cell it began on and travelled
/// [`SWEEP_THRESHOLD_PX`], which keeps a wobble indistinguishable from a plain
/// click wherever on the cell it lands.
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
    /// Where the press landed, which is what [`SWEEP_THRESHOLD_PX`] is
    /// measured from.
    pressed_at: Vec2,
    /// A cell the pointer wandered into without travelling far enough to be a
    /// sweep. It joins the sweep if one starts, and is forgotten otherwise.
    pending: Option<Entity>,
    /// Set once the pointer has earned a sweep, which is the moment this stops
    /// being a click.
    painted: bool,
    /// Set once this gesture's click has been dealt with, so ending it cannot
    /// place a second mark.
    clicked: bool,
}

/// Cells a sweep wants marked.
struct Paint {
    mark: Mark,
    /// The cell the pointer just entered.
    cell: Entity,
    /// The cell the sweep began on, handed back exactly once — it was left
    /// untouched until the pointer proved this was a sweep and not a click.
    origin: Option<Entity>,
    /// A cell the pointer crossed before the sweep was certain, handed back
    /// with the origin so the run has no gap in it.
    pending: Option<Entity>,
}

impl PaintStroke {
    /// A press on `origin` has begun moving. Nothing is painted yet.
    fn begin(&mut self, origin: Entity, mark: Mark, pressed_at: Vec2) {
        self.stroke = Some(Stroke {
            origin,
            mark,
            pressed_at,
            pending: None,
            painted: false,
            clicked: false,
        });
    }

    /// The pointer entered `cell` at `at`. Returns what to paint, if this is a
    /// sweep.
    fn extend(&mut self, cell: Entity, at: Vec2) -> Option<Paint> {
        let stroke = self.stroke.as_mut()?;
        if cell == stroke.origin {
            // Back where it started; the origin is already handled.
            return None;
        }
        if !stroke.painted && at.distance(stroke.pressed_at) < SWEEP_THRESHOLD_PX {
            // A wobble over the edge, not a sweep. Remember where it went, in
            // case the stroke goes on to become one.
            stroke.pending = Some(cell);
            return None;
        }
        // Hand back the cells the sweep skipped while it was still deciding,
        // on the first cell it reaches and never again.
        let origin = (!stroke.painted).then_some(stroke.origin);
        let pending = stroke.pending.take();
        stroke.painted = true;
        Some(Paint {
            mark: stroke.mark,
            cell,
            origin,
            pending,
        })
    }

    /// Whether the click that is arriving belongs to the player.
    ///
    /// A sweep that happens to end where it began also reports a click, and
    /// that click is not a click. Bevy emits `Click` before `DragEnd`, so the
    /// stroke is still here to be asked. A painted stroke is taken along with
    /// its verdict, so a gesture that never gets its `DragEnd` — a cancelled
    /// drag — cannot leave one behind to swallow a later click.
    fn click(&mut self) -> bool {
        match self.stroke.as_mut() {
            Some(stroke) if stroke.painted => {
                self.stroke = None;
                false
            }
            Some(stroke) => {
                stroke.clicked = true;
                true
            }
            None => true,
        }
    }

    /// The gesture is over. Returns the cell owed a click.
    ///
    /// Bevy sends `Click` only to an entity the pointer is still over, so a
    /// tap that slid onto a neighbour before releasing produces none at all.
    /// `DragEnd` is reported against the cell that was pressed either way,
    /// which makes it the one dependable end of a gesture.
    fn finish(&mut self) -> Option<Entity> {
        let stroke = self.stroke.take()?;
        (!stroke.painted && !stroke.clicked).then_some(stroke.origin)
    }
}

/// Left click cycles empty → cross → queen; right click drops or lifts a queen
/// directly, which is quicker once the crosses are in.
fn mark_clicked(
    session: &mut Session,
    sounds: &mut MessageWriter<Sound>,
    coord: Coord,
    button: PointerButton,
    auto_cross: bool,
) {
    let current = session.board.get(coord);
    let next = match button {
        PointerButton::Secondary => {
            if current == Mark::Queen {
                Mark::Empty
            } else {
                Mark::Queen
            }
        }
        _ => current.cycled(),
    };
    if let Some(sound) = Sound::for_change(current, next) {
        sounds.write(sound);
    }
    session.set_mark(coord, next, auto_cross);
}

/// Takes a click on a cell, unless a sweep has claimed it.
pub fn on_cell_click(
    click: On<Pointer<Click>>,
    cells: Query<&Cell>,
    play_state: Res<State<PlayState>>,
    save: Res<SaveData>,
    mut stroke: ResMut<PaintStroke>,
    mut session: ResMut<Session>,
    mut sounds: MessageWriter<Sound>,
) {
    if *play_state.get() != PlayState::Active {
        return;
    }
    // Ask the stroke before anything else, so the verdict is always consumed.
    if !stroke.click() {
        return;
    }
    let Ok(cell) = cells.get(click.event_target()) else {
        return;
    };

    mark_clicked(
        &mut session,
        &mut sounds,
        cell.coord,
        click.button,
        save.settings.auto_cross,
    );
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
    stroke.begin(target, mark, drag.pointer_location.position);
}

/// Continues a sweep onto each cell the pointer crosses.
pub fn on_cell_drag_enter(
    drag: On<Pointer<DragEnter>>,
    cells: Query<&Cell>,
    play_state: Res<State<PlayState>>,
    mut stroke: ResMut<PaintStroke>,
    mut session: ResMut<Session>,
    mut sounds: MessageWriter<Sound>,
) {
    if *play_state.get() != PlayState::Active {
        return;
    }
    let target = drag.event_target();
    if !cells.contains(target) {
        return;
    }
    let Some(paint) = stroke.extend(target, drag.pointer_location.position) else {
        return;
    };

    for entity in paint
        .origin
        .into_iter()
        .chain(paint.pending)
        .chain([paint.cell])
    {
        let Ok(cell) = cells.get(entity) else {
            continue;
        };
        let current = session.board.get(cell.coord);
        // Never let a sweep disturb a queen the player put down deliberately.
        if current == Mark::Queen {
            continue;
        }
        // One cue per cell the sweep actually changes. Thinning that run to a
        // tick rate is `audio.rs`'s business, not this loop's.
        if let Some(sound) = Sound::for_change(current, paint.mark) {
            sounds.write(sound);
        }
        // A sweep only lays or clears crosses, so auto-cross has no part in it.
        session.set_mark(cell.coord, paint.mark, false);
    }
}

/// Ends a gesture, and takes the click Bevy did not report.
///
/// A tap that drifted off its cell before the button came up never produces a
/// `Click`, so the cell it began on is marked from here instead.
pub fn on_cell_drag_end(
    drag: On<Pointer<DragEnd>>,
    cells: Query<&Cell>,
    play_state: Res<State<PlayState>>,
    save: Res<SaveData>,
    mut stroke: ResMut<PaintStroke>,
    mut session: ResMut<Session>,
    mut sounds: MessageWriter<Sound>,
) {
    // Always end the stroke, whatever the state of play, so nothing of this
    // gesture is left to confuse the next one.
    let Some(origin) = stroke.finish() else {
        return;
    };
    if *play_state.get() != PlayState::Active {
        return;
    }
    let Ok(cell) = cells.get(origin) else {
        return;
    };

    mark_clicked(
        &mut session,
        &mut sounds,
        cell.coord,
        drag.button,
        save.settings.auto_cross,
    );
}

/// The platform's own modifier for undo, redo and clear: Command on macOS,
/// Control everywhere else, matching what every other application on each
/// platform already uses.
#[cfg(target_os = "macos")]
const MODIFIER: [KeyCode; 2] = [KeyCode::SuperLeft, KeyCode::SuperRight];
#[cfg(not(target_os = "macos"))]
const MODIFIER: [KeyCode; 2] = [KeyCode::ControlLeft, KeyCode::ControlRight];

/// Whether `letter` was pressed this frame, going by the character the key
/// actually produces rather than its physical position.
///
/// [`KeyCode`] names a key by where it sits on a US layout, which is wrong for
/// a letter shortcut on a keyboard that puts a different letter there — a
/// German QWERTZ keyboard swaps Y and Z, so `KeyCode::KeyZ` is the key labelled
/// Y and undo fires on the wrong one. The logical [`Key`] is what the layout
/// says the key means, so this instead asks whether the pressed key produced
/// `letter` at all.
fn letter_just_pressed(keys: &ButtonInput<Key>, letter: char) -> bool {
    keys.get_just_pressed().any(|key| {
        let Key::Character(text) = key else {
            return false;
        };
        let mut chars = text.chars();
        matches!((chars.next(), chars.next()), (Some(c), None) if c.eq_ignore_ascii_case(&letter))
    })
}

/// Keyboard shortcuts for the toolbar.
pub fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    letters: Res<ButtonInput<Key>>,
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

    let modifier = keys.any_pressed(MODIFIER);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);

    if modifier && letter_just_pressed(&letters, 'z') {
        if shift {
            session.redo();
        } else {
            session.undo();
        }
    } else if modifier && letter_just_pressed(&letters, 'y') {
        session.redo();
    } else if letter_just_pressed(&letters, 'h') {
        session.request_hint(crate::theme::region_names(save.settings.colourblind));
    } else if modifier && letter_just_pressed(&letters, 'r') {
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

    /// Where the press landed, and a point well clear of it.
    const PRESS: Vec2 = Vec2::new(100.0, 100.0);
    const FAR: Vec2 = Vec2::new(160.0, 100.0);
    /// Inside the threshold: the drift of a trackpad tap.
    const DRIFT: Vec2 = Vec2::new(103.0, 100.0);

    /// Bevy reports a drag for the smallest twitch during a click, so a
    /// gesture that never leaves its cell has to paint nothing and let its
    /// click through.
    #[test]
    fn a_press_that_never_leaves_its_cell_is_a_click() {
        let (a, ..) = cells();
        let mut stroke = PaintStroke::default();

        stroke.begin(a, Mark::Cross, PRESS);
        // Bevy sends Drag events throughout; none of them leave the cell.
        assert!(stroke.extend(a, DRIFT).is_none(), "a wiggle must not paint");
        assert!(stroke.click(), "a wiggle must not swallow its own click");
    }

    /// A tap near a cell edge slides into the neighbour as the finger
    /// flattens, and Bevy then reports no click at all. The cell that was
    /// pressed still has to take the mark, or the tap is simply lost.
    #[test]
    fn a_tap_that_drifts_onto_a_neighbour_still_counts_as_a_click() {
        let (a, b, _) = cells();
        let mut stroke = PaintStroke::default();

        stroke.begin(a, Mark::Cross, PRESS);
        assert!(
            stroke.extend(b, DRIFT).is_none(),
            "a drift of a few pixels is not a sweep"
        );
        assert_eq!(
            stroke.finish(),
            Some(a),
            "the cell that was pressed is owed the click"
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
            stroke.begin(a, Mark::Cross, PRESS);
            assert!(stroke.click());
            assert_eq!(stroke.finish(), None, "the click already landed");
        }
    }

    #[test]
    fn leaving_the_cell_starts_a_sweep_and_paints_the_origin_too() {
        let (a, b, c) = cells();
        let mut stroke = PaintStroke::default();
        stroke.begin(a, Mark::Cross, PRESS);

        // The first cell the sweep reaches also settles the one it began on,
        // which was deliberately left alone until now.
        let first = stroke.extend(b, FAR).expect("leaving the cell is a sweep");
        assert_eq!(first.mark, Mark::Cross);
        assert_eq!(first.cell, b);
        assert_eq!(first.origin, Some(a));

        // The origin is handed back only once.
        let second = stroke.extend(c, FAR).expect("the sweep continues");
        assert_eq!(second.cell, c);
        assert_eq!(second.origin, None);
    }

    /// A sweep that starts near an edge crosses its neighbour before it has
    /// travelled far enough to be a sweep. That cell is part of the run and
    /// must not be left blank behind the stroke.
    #[test]
    fn a_sweep_catches_up_the_cell_it_wobbled_through() {
        let (a, b, c) = cells();
        let mut stroke = PaintStroke::default();
        stroke.begin(a, Mark::Cross, PRESS);

        assert!(stroke.extend(b, DRIFT).is_none());
        let paint = stroke.extend(c, FAR).expect("this is a sweep now");
        assert_eq!(paint.origin, Some(a));
        assert_eq!(paint.pending, Some(b), "the cell passed through in doubt");
        assert_eq!(paint.cell, c);

        // Both are handed back exactly once.
        let next = stroke.extend(b, FAR).expect("the sweep continues");
        assert_eq!(next.origin, None);
        assert_eq!(next.pending, None);
    }

    #[test]
    fn a_sweep_swallows_the_click_it_ends_on() {
        let (a, b, _) = cells();
        let mut stroke = PaintStroke::default();

        stroke.begin(a, Mark::Cross, PRESS);
        stroke.extend(b, FAR);
        // Sweeping back and releasing on the starting cell reports a click,
        // which belongs to the sweep rather than to the player.
        assert!(
            stroke.extend(a, PRESS).is_none(),
            "the origin is already painted"
        );
        assert!(!stroke.click());
    }

    #[test]
    fn an_erasing_sweep_carries_its_own_mark() {
        let (a, b, _) = cells();
        let mut stroke = PaintStroke::default();

        stroke.begin(a, Mark::Empty, PRESS);
        let paint = stroke.extend(b, FAR).unwrap();
        assert_eq!(paint.mark, Mark::Empty);
    }

    /// A cancelled drag never gets its `DragEnd`. Its verdict must not linger
    /// and swallow an unrelated click later on.
    #[test]
    fn a_stale_stroke_cannot_swallow_a_later_click() {
        let (a, b, _) = cells();
        let mut stroke = PaintStroke::default();

        stroke.begin(a, Mark::Cross, PRESS);
        stroke.extend(b, FAR);
        assert!(!stroke.click(), "the sweep's own click is swallowed");
        // No DragEnd arrives, and the next click is a genuine one.
        assert!(stroke.click(), "the verdict must not persist");
    }

    /// A sweep has already marked everything it touched, so ending one owes
    /// the board nothing further.
    #[test]
    fn a_sweep_is_not_owed_a_click_when_it_ends() {
        let (a, b, _) = cells();
        let mut stroke = PaintStroke::default();

        stroke.begin(a, Mark::Cross, PRESS);
        stroke.extend(b, FAR);
        assert_eq!(stroke.finish(), None);
    }

    #[test]
    fn events_without_a_stroke_are_harmless() {
        let (a, ..) = cells();
        let mut stroke = PaintStroke::default();

        assert!(stroke.extend(a, FAR).is_none());
        assert!(stroke.click());
        assert_eq!(stroke.finish(), None);
    }

    /// A German keyboard's Y key sits where a US layout's Z does and vice
    /// versa: `KeyCode::KeyZ` would fire on the wrong one, which is exactly
    /// the bug this match-by-character approach exists to avoid.
    #[test]
    fn a_letter_shortcut_follows_what_the_key_produces_not_where_it_sits() {
        let mut letters = ButtonInput::<Key>::default();
        letters.press(Key::Character("y".into()));

        assert!(letter_just_pressed(&letters, 'y'));
        assert!(!letter_just_pressed(&letters, 'z'));
    }

    /// Shift held for redo (`Ctrl+Shift+Z`) turns the produced character
    /// upper case, so the match has to ignore case.
    #[test]
    fn a_letter_shortcut_ignores_case() {
        let mut letters = ButtonInput::<Key>::default();
        letters.press(Key::Character("Z".into()));

        assert!(letter_just_pressed(&letters, 'z'));
    }

    #[test]
    fn a_letter_shortcut_does_not_match_a_different_key() {
        let mut letters = ButtonInput::<Key>::default();
        letters.press(Key::Character("a".into()));

        assert!(!letter_just_pressed(&letters, 'z'));
    }

    /// A modifier or arrow key carries no character at all, and must not be
    /// mistaken for one.
    #[test]
    fn a_non_character_key_never_matches_a_letter() {
        let mut letters = ButtonInput::<Key>::default();
        letters.press(Key::Control);

        assert!(!letter_just_pressed(&letters, 'z'));
    }
}
