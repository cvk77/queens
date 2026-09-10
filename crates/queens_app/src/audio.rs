//! The game's four sounds.
//!
//! The WAV files are compiled into the binary, as the font is: the game ships
//! as one executable with nothing beside it, and the web build as one wasm
//! next to one page. An `assets/` directory would have to be carried into the
//! macOS bundle, the release archives and the trunk build before a single cue
//! played.
//!
//! A cue is asked for by writing a [`Sound`] message rather than played on
//! the spot, which gives one system the final say over what reaches the
//! speakers. That is where the settings toggle is honoured and where the
//! retrigger floor in [`far_enough_apart`] lives: a drag-sweep marks a cell
//! every frame or two, and without a floor those ticks fuse into a buzz.

use bevy::audio::Volume;
use bevy::prelude::*;
use queens_core::Mark;

use crate::persistence::SaveData;

/// Loads the cues and plays the ones the game asks for.
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Sound>()
            .add_systems(Startup, load_cues)
            .add_systems(Update, play_requested.run_if(resource_exists::<Cues>));
    }
}

// --- what the game asks for ------------------------------------------------

/// A cue the game wants heard.
#[derive(Message, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sound {
    /// A cross going down.
    Cross,
    /// A queen going down.
    Queen,
    /// A mark coming off again.
    Erase,
    /// The board solved.
    Victory,
}

impl Sound {
    /// The cue a cell changing from `previous` to `next` should make.
    ///
    /// A cell set to what it already holds changes nothing, and a sweep
    /// crosses plenty of those, so it stays silent.
    pub fn for_change(previous: Mark, next: Mark) -> Option<Self> {
        if previous == next {
            return None;
        }
        Some(match next {
            Mark::Cross => Self::Cross,
            Mark::Queen => Self::Queen,
            Mark::Empty => Self::Erase,
        })
    }

    /// Whether this is a cue a player can trigger as fast as they can move,
    /// and so one that shares the retrigger floor. The fanfare happens once
    /// and is never in that company.
    fn comes_from_the_board(self) -> bool {
        !matches!(self, Self::Victory)
    }

    /// How loud this cue plays.
    ///
    /// The four recordings sit about ten decibels apart in peak level — the
    /// cross's tick is the hottest of them, and the shortest — so the balance
    /// between them is set here rather than in the files.
    fn volume(self) -> f32 {
        match self {
            Self::Cross => 0.5,
            Self::Queen => 1.0,
            Self::Erase => 1.0,
            Self::Victory => 0.9,
        }
    }
}

/// The shortest gap between two cues from the board.
///
/// Between "two deliberate clicks" and "one frame of a sweep": fast enough
/// that nothing a player does on purpose is swallowed, slow enough that a
/// sweep ticks rather than buzzes.
const RETRIGGER_SECONDS: f32 = 0.045;

/// Whether a cue at `now` is far enough behind the last one to be heard as a
/// tick of its own.
fn far_enough_apart(now: f32, last: Option<f32>) -> bool {
    last.is_none_or(|last| now - last >= RETRIGGER_SECONDS)
}

/// Spawns a one-shot player per cue asked for, subject to the settings and
/// the retrigger floor.
///
/// The players despawn themselves when they finish, so nothing accumulates.
fn play_requested(
    mut commands: Commands,
    mut requested: MessageReader<Sound>,
    cues: Res<Cues>,
    save: Res<SaveData>,
    time: Res<Time>,
    mut last_from_the_board: Local<Option<f32>>,
) {
    if !save.settings.sound {
        // Drained rather than left to pile up, so turning sound back on does
        // not replay what happened while it was off.
        requested.clear();
        return;
    }
    let now = time.elapsed_secs();
    for &sound in requested.read() {
        if sound.comes_from_the_board() {
            if !far_enough_apart(now, *last_from_the_board) {
                continue;
            }
            *last_from_the_board = Some(now);
        }
        commands.spawn((
            AudioPlayer::new(cues.handle(sound)),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(sound.volume())),
        ));
    }
}

// --- the recordings --------------------------------------------------------

/// 16-bit PCM at 44.1 kHz, which is what `bevy/wav` decodes. Nothing here is
/// long enough for a compressed format to be worth a decoder.
const CROSS_WAV: &[u8] = include_bytes!("../assets/sounds/cross.wav");
const QUEEN_WAV: &[u8] = include_bytes!("../assets/sounds/queen.wav");
const ERASE_WAV: &[u8] = include_bytes!("../assets/sounds/erase.wav");
const VICTORY_WAV: &[u8] = include_bytes!("../assets/sounds/victory.wav");

/// The four cues, as assets to play.
#[derive(Resource)]
struct Cues {
    cross: Handle<AudioSource>,
    queen: Handle<AudioSource>,
    erase: Handle<AudioSource>,
    victory: Handle<AudioSource>,
}

impl Cues {
    fn handle(&self, sound: Sound) -> Handle<AudioSource> {
        match sound {
            Sound::Cross => self.cross.clone(),
            Sound::Queen => self.queen.clone(),
            Sound::Erase => self.erase.clone(),
            Sound::Victory => self.victory.clone(),
        }
    }
}

/// Wraps the embedded bytes as assets.
///
/// Added directly rather than through the asset server: the bytes are already
/// here, and there is no file for it to go looking for.
fn load_cues(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    commands.insert_resource(Cues {
        cross: sources.add(cue(CROSS_WAV)),
        queen: sources.add(cue(QUEEN_WAV)),
        erase: sources.add(cue(ERASE_WAV)),
        victory: sources.add(cue(VICTORY_WAV)),
    });
}

fn cue(wav: &'static [u8]) -> AudioSource {
    AudioSource { bytes: wav.into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::audio::{Decodable, Source};

    /// Every cue, named, so a property can be asserted of all four at once.
    const CUES: [(&str, &[u8]); 4] = [
        ("cross", CROSS_WAV),
        ("queen", QUEEN_WAV),
        ("erase", ERASE_WAV),
        ("victory", VICTORY_WAV),
    ];

    /// A file rodio cannot decode panics inside the audio thread the first
    /// time the cue is played, which is a long way from where it could be
    /// noticed. Replacing one of these files has to fail here instead.
    #[test]
    fn every_cue_decodes_to_audible_samples() {
        for (name, wav) in CUES {
            let mut decoder = cue(wav).decoder();
            let channels = decoder.channels().get();
            let rate = decoder.sample_rate().get();
            assert!(
                (1..=2).contains(&channels),
                "{name} has {channels} channels"
            );

            let samples = decoder.by_ref().count();
            assert!(samples > 0, "{name} decodes to nothing");
            let seconds = samples as f32 / rate as f32 / f32::from(channels);
            // A cue is a cue: long enough to hear, short enough that the next
            // one is not queueing up behind it.
            assert!(
                (0.005..3.0).contains(&seconds),
                "{name} runs {seconds}s at {rate}Hz"
            );
        }
    }

    #[test]
    fn a_cell_that_does_not_change_makes_no_sound() {
        assert_eq!(Sound::for_change(Mark::Cross, Mark::Cross), None);
        assert_eq!(Sound::for_change(Mark::Empty, Mark::Empty), None);
    }

    #[test]
    fn each_mark_has_its_own_cue() {
        assert_eq!(
            Sound::for_change(Mark::Empty, Mark::Cross),
            Some(Sound::Cross)
        );
        assert_eq!(
            Sound::for_change(Mark::Cross, Mark::Queen),
            Some(Sound::Queen)
        );
        assert_eq!(
            Sound::for_change(Mark::Queen, Mark::Empty),
            Some(Sound::Erase)
        );
    }

    /// A sweep marks a cell every frame or two, and every one of those asks
    /// for a tick. They have to come out as a run rather than as a buzz.
    #[test]
    fn a_sweep_ticks_rather_than_buzzing() {
        let mut last = None;
        let mut heard = 0;
        // A cell every 16ms: a brisk sweep at sixty frames a second.
        for frame in 0..60 {
            let now = frame as f32 * 0.016;
            if far_enough_apart(now, last) {
                heard += 1;
                last = Some(now);
            }
        }
        assert_eq!(heard, 20, "one tick per 45ms of the second swept");
    }

    #[test]
    fn the_first_cue_is_never_held_back() {
        assert!(far_enough_apart(0.0, None));
        assert!(far_enough_apart(12_345.0, None));
    }

    /// A sweep hands back the cell it started on together with the one it
    /// reached, so two cells can be marked in one frame. That is one tick.
    #[test]
    fn cues_in_the_same_frame_collapse_into_one() {
        assert!(!far_enough_apart(4.0, Some(4.0)));
    }
}
