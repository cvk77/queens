//! A scripted run of the game that screenshots each screen, for checking the
//! UI without a person at the keyboard.
//!
//! Enabled by setting `QUEENS_CAPTURE` to an output directory:
//!
//! ```text
//! QUEENS_CAPTURE=target/capture cargo run -p queens_app
//! ```
//!
//! It drives the app through the menu, a generated puzzle and a solved board,
//! writing a PNG at each step, then quits. Inert unless the variable is set, so
//! it costs a normal run nothing.
//!
//! The tail of the script feeds mouse gestures through Bevy's picking pipeline
//! and asserts what they did to the board, which covers input handling as a
//! player meets it rather than as a state machine. Failed checks make the
//! process exit non-zero.

use std::path::PathBuf;

use bevy::camera::{NormalizedRenderTarget, RenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PrimaryWindow, WindowRef};
use queens_core::{Coord, Difficulty, Mark, PuzzleSeed};

use crate::persistence::{InProgress, SaveData};
use crate::session::{PuzzleRequest, Session};
use crate::states::{AppState, PlayState};
use crate::theme;

/// The environment variable that turns this on and says where to write.
const CAPTURE_ENV: &str = "QUEENS_CAPTURE";

/// A step in the script: run `action` once, `seconds` after startup.
struct Beat {
    seconds: f32,
    action: fn(&mut Commands, &mut CaptureContext),
}

/// The bits of the world a beat is allowed to touch.
pub struct CaptureContext<'a> {
    pub directory: &'a PathBuf,
    pub next_app: &'a mut NextState<AppState>,
    pub next_play: &'a mut NextState<PlayState>,
    pub session: Option<&'a mut Session>,
    pub save: &'a mut SaveData,
    /// Screen position of the centre of every board cell, when a board is up.
    pub cell_centres: &'a [(Coord, Vec2)],
    /// How many queen or cross marks are actually being drawn.
    pub visible_marks: usize,
    /// A cell a beat wants later beats to work on.
    pub subject: &'a mut Option<Coord>,
    /// A board position a beat wants a later one to compare against.
    pub anchor: &'a mut Option<Vec2>,
    /// Pointer events to feed into the picking pipeline after this beat.
    pub pointer: Vec<(Vec2, PointerAction)>,
    /// Assertions this beat made, reported at the end of the run.
    pub checks: Vec<(String, bool)>,
    /// Set by the last beat; the system writes the exit message afterwards, so
    /// the context need not borrow a writer for the whole run.
    pub finished: bool,
}

impl CaptureContext<'_> {
    fn shoot(&self, commands: &mut Commands, name: &str) {
        let path = self.directory.join(format!("{name}.png"));
        info!("capture: writing {}", path.display());
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }

    /// Screen centre of a cell, if the board is on screen.
    fn centre_of(&self, cell: Coord) -> Option<Vec2> {
        self.cell_centres
            .iter()
            .find(|(coord, _)| *coord == cell)
            .map(|(_, centre)| *centre)
    }

    /// Records an assertion about the state the gestures left behind.
    fn check(&mut self, what: impl Into<String>, passed: bool) {
        self.checks.push((what.into(), passed));
    }
}

/// The script. Timings leave room for a puzzle to generate and for the UI to
/// settle before each shot.
const SCRIPT: &[Beat] = &[
    Beat {
        seconds: 1.5,
        action: |commands, ctx| ctx.shoot(commands, "1-main-menu"),
    },
    Beat {
        seconds: 2.0,
        action: |_commands, ctx| ctx.next_app.set(AppState::NewGame),
    },
    Beat {
        seconds: 3.0,
        action: |commands, ctx| ctx.shoot(commands, "2-new-game"),
    },
    Beat {
        seconds: 3.5,
        action: |commands, ctx| {
            // A fixed seed keeps the captured board comparable between runs.
            ctx.save.settings.size = 8;
            ctx.save.settings.difficulty = Difficulty::Medium;
            commands.insert_resource(PuzzleRequest::fresh(PuzzleSeed::new(
                8,
                Difficulty::Medium,
                20_260_905,
            )));
            ctx.next_app.set(AppState::Generating);
        },
    },
    Beat {
        seconds: 5.5,
        action: |commands, ctx| ctx.shoot(commands, "3-fresh-board"),
    },
    Beat {
        seconds: 6.0,
        action: |_commands, ctx| {
            // Place a few correct queens plus one that conflicts, so the shot
            // shows marks, auto-crosses and the conflict highlight at once.
            let Some(session) = ctx.session.as_deref_mut() else {
                return;
            };
            let solution: Vec<_> = session.puzzle.solution_cells().take(3).collect();
            for cell in solution {
                session.set_mark(cell, Mark::Queen, true);
            }
            let clash = session
                .puzzle
                .cells()
                .find(|&c| c.row == 0 && !session.puzzle.is_solution_cell(c));
            if let Some(clash) = clash {
                session.set_mark(clash, Mark::Queen, false);
            }
        },
    },
    Beat {
        seconds: 6.6,
        action: |commands, ctx| ctx.shoot(commands, "4-marks-and-conflict"),
    },
    // Note where the board is before asking for a hint. A hint runs to two or
    // three lines where the rating line is one, and the board must not shift
    // under the player to make room for it.
    Beat {
        seconds: 6.9,
        action: |_commands, ctx| {
            *ctx.anchor = ctx.centre_of(Coord::new(0, 0));
        },
    },
    Beat {
        seconds: 7.0,
        action: |_commands, ctx| {
            if let Some(session) = ctx.session.as_deref_mut() {
                session.reset();
                session.request_hint(theme::region_names(ctx.save.settings.colourblind));
            }
        },
    },
    Beat {
        seconds: 7.5,
        action: |_commands, ctx| {
            let before = *ctx.anchor;
            let after = ctx.centre_of(Coord::new(0, 0));
            ctx.check(
                format!("a hint leaves the board where it was ({before:?} -> {after:?})"),
                before.is_some() && before == after,
            );
        },
    },
    Beat {
        seconds: 7.6,
        action: |commands, ctx| ctx.shoot(commands, "5-hint"),
    },
    // Pausing must not double as a free look at the board, so put some marks
    // down before stopping the clock.
    Beat {
        seconds: 7.7,
        action: |_commands, ctx| {
            let Some(session) = ctx.session.as_deref_mut() else {
                return;
            };
            let solution: Vec<_> = session.puzzle.solution_cells().take(3).collect();
            for cell in solution {
                session.set_mark(cell, Mark::Queen, true);
            }
        },
    },
    Beat {
        seconds: 7.85,
        action: |_commands, ctx| {
            let drawn = ctx.visible_marks;
            ctx.check(
                format!("marks are drawn while playing ({drawn} on screen)"),
                drawn > 0,
            );
        },
    },
    Beat {
        seconds: 7.9,
        action: |_commands, ctx| ctx.next_play.set(PlayState::Paused),
    },
    Beat {
        seconds: 8.5,
        action: |commands, ctx| {
            let drawn = ctx.visible_marks;
            ctx.check(
                format!("pausing hides every queen and cross ({drawn} still on screen)"),
                drawn == 0,
            );
            ctx.shoot(commands, "6-paused");
        },
    },
    Beat {
        seconds: 8.9,
        action: |_commands, ctx| ctx.next_play.set(PlayState::Active),
    },
    Beat {
        seconds: 9.1,
        action: |_commands, ctx| {
            let drawn = ctx.visible_marks;
            ctx.check(
                format!("marks come back on resuming ({drawn} on screen)"),
                drawn > 0,
            );
        },
    },
    Beat {
        seconds: 9.3,
        action: |_commands, ctx| {
            // Finish the puzzle outright to reach the victory panel.
            let Some(session) = ctx.session.as_deref_mut() else {
                return;
            };
            let solution: Vec<_> = session.puzzle.solution_cells().collect();
            for cell in solution {
                session.set_mark(cell, Mark::Queen, false);
            }
        },
    },
    Beat {
        seconds: 10.1,
        action: |commands, ctx| ctx.shoot(commands, "7-victory"),
    },
    Beat {
        seconds: 10.5,
        action: |_commands, ctx| ctx.next_app.set(AppState::Stats),
    },
    Beat {
        seconds: 11.3,
        action: |commands, ctx| ctx.shoot(commands, "8-stats"),
    },
    Beat {
        seconds: 11.9,
        action: |_commands, ctx| ctx.next_app.set(AppState::Settings),
    },
    Beat {
        seconds: 12.7,
        action: |commands, ctx| ctx.shoot(commands, "9-settings"),
    },
    // Plant a saved game and pick it up again, which exercises the whole
    // resume path: the seed alone has to rebuild the identical puzzle.
    Beat {
        seconds: 13.3,
        action: |_commands, ctx| {
            let mut marks = vec![Mark::Empty; 8 * 8];
            for mark in marks.iter_mut().take(5) {
                *mark = Mark::Cross;
            }
            marks[8 + 6] = Mark::Queen;
            ctx.save.in_progress = Some(InProgress {
                seed: PuzzleSeed::new(8, Difficulty::Medium, 20_260_905),
                marks,
                elapsed: 92.0,
                auto_crossed: Vec::new(),
                // Non-zero, so the resume has something to carry across.
                hints_used: 2,
            });
            ctx.next_app.set(AppState::MainMenu);
        },
    },
    Beat {
        seconds: 14.1,
        action: |commands, ctx| ctx.shoot(commands, "10-continue-offered"),
    },
    Beat {
        seconds: 14.5,
        action: |commands, ctx| {
            let Some(saved) = ctx.save.in_progress.clone() else {
                return;
            };
            commands.insert_resource(PuzzleRequest::resume(&saved));
            ctx.next_app.set(AppState::Generating);
        },
    },
    Beat {
        seconds: 16.0,
        action: |commands, ctx| ctx.shoot(commands, "11-resumed"),
    },
    // Hints spent on a puzzle belong to the puzzle, not the sitting: putting it
    // down and picking it up again must not hand the player a clean sheet.
    Beat {
        seconds: 16.2,
        action: |_commands, ctx| {
            let carried = ctx.session.as_deref().map(|session| session.hints_used);
            ctx.check(
                format!("a resumed game keeps the hints it has spent ({carried:?})"),
                carried == Some(2),
            );
        },
    },
    // A double-click with a pixel of movement inside each press, which Bevy
    // reports as a drag. Both clicks have to land for the cell to reach a
    // queen.
    Beat {
        seconds: 16.6,
        action: |_commands, ctx| {
            let Some(session) = ctx.session.as_deref_mut() else {
                return;
            };
            session.reset();
            let subject = session.puzzle.solution_cells().next();
            *ctx.subject = subject;
            // Park the pointer on the cell so picking sees it as hovered.
            if let Some(centre) = subject.and_then(|cell| ctx.centre_of(cell)) {
                ctx.pointer
                    .push((centre, PointerAction::Move { delta: Vec2::ZERO }));
            }
        },
    },
    Beat {
        seconds: 16.75,
        action: |_commands, ctx| press(ctx, Vec2::ZERO),
    },
    Beat {
        seconds: 16.85,
        action: |_commands, ctx| jitter(ctx),
    },
    Beat {
        seconds: 16.95,
        action: |_commands, ctx| release(ctx, JITTER),
    },
    Beat {
        seconds: 17.1,
        action: |_commands, ctx| press(ctx, JITTER),
    },
    Beat {
        seconds: 17.2,
        action: |_commands, ctx| jitter(ctx),
    },
    Beat {
        seconds: 17.3,
        action: |_commands, ctx| release(ctx, JITTER),
    },
    Beat {
        seconds: 17.6,
        action: |commands, ctx| {
            let subject = *ctx.subject;
            let mark = subject.and_then(|cell| {
                ctx.session
                    .as_deref()
                    .map(|session| session.board.get(cell))
            });
            ctx.check(
                format!("a twitchy double-click places a queen (cell held {mark:?})"),
                mark == Some(Mark::Queen),
            );
            ctx.shoot(commands, "12-double-click");
        },
    },
    // A sweep across several cells still has to paint every one of them.
    Beat {
        seconds: 17.8,
        action: |_commands, ctx| {
            let Some(session) = ctx.session.as_deref_mut() else {
                return;
            };
            session.reset();
            *ctx.subject = Some(Coord::new(7, 0));
            if let Some(centre) = ctx.centre_of(Coord::new(7, 0)) {
                ctx.pointer
                    .push((centre, PointerAction::Move { delta: Vec2::ZERO }));
            }
        },
    },
    Beat {
        seconds: 17.95,
        action: |_commands, ctx| press(ctx, Vec2::ZERO),
    },
    // Drag along the bottom row, a cell at a time.
    Beat {
        seconds: 18.05,
        action: |_commands, ctx| slide_to(ctx, Coord::new(7, 1)),
    },
    Beat {
        seconds: 18.15,
        action: |_commands, ctx| slide_to(ctx, Coord::new(7, 2)),
    },
    Beat {
        seconds: 18.25,
        action: |_commands, ctx| slide_to(ctx, Coord::new(7, 3)),
    },
    Beat {
        seconds: 18.35,
        action: |_commands, ctx| {
            if let Some(centre) = ctx.centre_of(Coord::new(7, 3)) {
                ctx.pointer
                    .push((centre, PointerAction::Release(PointerButton::Primary)));
            }
        },
    },
    Beat {
        seconds: 18.6,
        action: |commands, ctx| {
            let swept: Vec<Mark> = ctx
                .session
                .as_deref()
                .map(|session| {
                    (0..4)
                        .map(|col| session.board.get(Coord::new(7, col)))
                        .collect()
                })
                .unwrap_or_default();
            ctx.check(
                format!("a sweep crosses off every cell it crosses (row held {swept:?})"),
                swept == vec![Mark::Cross; 4],
            );
            ctx.shoot(commands, "13-sweep");
        },
    },
    Beat {
        seconds: 19.0,
        action: |_commands, ctx| ctx.finished = true,
    },
];

/// How far the pointer twitches during a press. One pixel is enough for Bevy
/// to call it a drag.
const JITTER: Vec2 = Vec2::new(1.0, 1.0);

fn press(ctx: &mut CaptureContext, offset: Vec2) {
    let Some(centre) = ctx.subject.and_then(|cell| ctx.centre_of(cell)) else {
        return;
    };
    ctx.pointer.push((
        centre + offset,
        PointerAction::Press(PointerButton::Primary),
    ));
}

fn jitter(ctx: &mut CaptureContext) {
    let Some(centre) = ctx.subject.and_then(|cell| ctx.centre_of(cell)) else {
        return;
    };
    ctx.pointer
        .push((centre + JITTER, PointerAction::Move { delta: JITTER }));
}

fn release(ctx: &mut CaptureContext, offset: Vec2) {
    let Some(centre) = ctx.subject.and_then(|cell| ctx.centre_of(cell)) else {
        return;
    };
    ctx.pointer.push((
        centre + offset,
        PointerAction::Release(PointerButton::Primary),
    ));
}

/// Moves the pointer onto another cell while a button is held.
fn slide_to(ctx: &mut CaptureContext, cell: Coord) {
    let Some(from) = ctx.subject.and_then(|cell| ctx.centre_of(cell)) else {
        return;
    };
    let Some(to) = ctx.centre_of(cell) else {
        return;
    };
    ctx.pointer
        .push((to, PointerAction::Move { delta: to - from }));
    *ctx.subject = Some(cell);
}

#[derive(Resource)]
struct CaptureRun {
    directory: PathBuf,
    elapsed: f32,
    next_beat: usize,
    /// A cell later beats work on, carried between them.
    subject: Option<Coord>,
    /// Where the board sat when a beat last noted it down, for checking that
    /// something which changes the text around it did not move it.
    anchor: Option<Vec2>,
    /// Every assertion the gesture beats made.
    checks: Vec<(String, bool)>,
}

/// Adds the capture script when `QUEENS_CAPTURE` is set.
pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        let Ok(directory) = std::env::var(CAPTURE_ENV) else {
            return;
        };
        let directory = PathBuf::from(directory);
        if let Err(error) = std::fs::create_dir_all(&directory) {
            error!("capture: cannot create {}: {error}", directory.display());
            return;
        }

        info!("capture: writing screenshots to {}", directory.display());
        app.insert_resource(CaptureRun {
            directory,
            elapsed: 0.0,
            next_beat: 0,
            subject: None,
            anchor: None,
            checks: Vec::new(),
        })
        .add_systems(Update, run_script);
    }
}

// A Bevy system takes what it needs; splitting this one up would only move the
// same parameters somewhere else.
#[allow(clippy::too_many_arguments)]
fn run_script(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<CaptureRun>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_play: ResMut<NextState<PlayState>>,
    mut session: Option<ResMut<Session>>,
    mut save: ResMut<SaveData>,
    cells: Query<(&crate::game::board::Cell, &UiGlobalTransform)>,
    marks: Query<
        &Node,
        Or<(
            With<crate::game::board::QueenMark>,
            With<crate::game::board::CrossMark>,
        )>,
    >,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    mut pointer_input: MessageWriter<PointerInput>,
    mut exit: MessageWriter<AppExit>,
) {
    run.elapsed += time.delta_secs();
    let directory = run.directory.clone();
    // Where every cell sits on screen, so a beat can aim the pointer at one.
    let centres: Vec<(Coord, Vec2)> = cells
        .iter()
        .map(|(cell, transform)| (cell.coord, transform.translation))
        .collect();

    let visible_marks = marks
        .iter()
        .filter(|node| node.display != Display::None)
        .count();

    let mut subject = run.subject;
    let mut anchor = run.anchor;
    let mut finished = false;
    let mut pointer = Vec::new();
    let mut checks = Vec::new();

    // At most one beat per frame, even when several have fallen due after a
    // slow frame. A beat that asserts something reads state gathered at the top
    // of its own frame, so sharing a frame with the beat that set that state up
    // would have it looking at the position from before. Beats that pile up are
    // simply played out on consecutive frames.
    if run.next_beat < SCRIPT.len() && SCRIPT[run.next_beat].seconds <= run.elapsed {
        let beat = &SCRIPT[run.next_beat];
        run.next_beat += 1;

        let mut context = CaptureContext {
            directory: &directory,
            next_app: &mut next_app,
            next_play: &mut next_play,
            session: session.as_deref_mut(),
            save: &mut save,
            cell_centres: &centres,
            visible_marks,
            subject: &mut subject,
            anchor: &mut anchor,
            pointer: Vec::new(),
            checks: Vec::new(),
            finished: false,
        };
        (beat.action)(&mut commands, &mut context);
        finished |= context.finished;
        pointer.extend(context.pointer);
        checks.extend(context.checks);
    }

    run.subject = subject;
    run.anchor = anchor;
    run.checks.extend(checks);

    // Feed the gestures into the same pipeline the mouse uses.
    if let Ok(window) = primary_window.single()
        && let Some(target) = RenderTarget::Window(WindowRef::Primary).normalize(Some(window))
    {
        for (position, action) in pointer {
            pointer_input.write(PointerInput::new(
                PointerId::Mouse,
                Location {
                    target: target.clone(),
                    position,
                },
                action,
            ));
        }
    }

    if finished {
        let failures = report(&run.checks);
        exit.write(if failures == 0 {
            AppExit::Success
        } else {
            AppExit::from_code(1)
        });
    }
}

/// Prints every assertion and returns how many failed.
fn report(checks: &[(String, bool)]) -> usize {
    let failures = checks.iter().filter(|(_, passed)| !passed).count();
    for (what, passed) in checks {
        if *passed {
            info!("capture: PASS {what}");
        } else {
            error!("capture: FAIL {what}");
        }
    }
    info!(
        "capture: {} of {} checks passed",
        checks.len() - failures,
        checks.len()
    );
    failures
}

/// Never rendered, but the `Coord` type has to be in scope for `centres`.
const _: fn() = || {
    let _ = NormalizedRenderTarget::Window;
};
