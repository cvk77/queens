//! Checks once at startup whether GitHub has a newer release than this
//! build, so the main menu can mention it without the game ever reaching the
//! network again once that check is done.

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
#[cfg(not(target_arch = "wasm32"))]
use serde::Deserialize;

pub struct UpdateCheckPlugin;

impl Plugin for UpdateCheckPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LatestRelease>();
        // A browser build is served by itch, which is always current, so the
        // resource stays at its default and the menu simply never has
        // anything to announce.
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Startup, start_check)
            .add_systems(Update, finish_check.run_if(resource_exists::<CheckTask>));
    }
}

/// The tag of a newer release than this build, once the check has finished.
/// `None` before it finishes, if it fails, and if this build is already
/// current - the main menu only has something to say when there is a newer
/// tag than the one it was built from.
#[derive(Resource, Default)]
pub struct LatestRelease(pub Option<String>);

/// The in-flight check. Removed once it resolves, so [`finish_check`] only
/// runs while there is something to poll.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct CheckTask(Task<Option<String>>);

/// GitHub's API stays within its generous unauthenticated rate limit for a
/// once-per-launch check, and needs no token to read a public repo's
/// releases.
#[cfg(not(target_arch = "wasm32"))]
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/cvk77/queens/releases/latest";

/// Long enough for a slow connection, short enough that a single stuck check
/// does not hold an async-compute thread for the rest of the session.
#[cfg(not(target_arch = "wasm32"))]
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(not(target_arch = "wasm32"))]
fn start_check(mut commands: Commands) {
    let task = AsyncComputeTaskPool::get().spawn(async { fetch_latest_tag() });
    commands.insert_resource(CheckTask(task));
}

#[cfg(not(target_arch = "wasm32"))]
fn finish_check(
    mut commands: Commands,
    mut task: ResMut<CheckTask>,
    mut latest: ResMut<LatestRelease>,
) {
    let Some(newer) = block_on(poll_once(&mut task.0)) else {
        return;
    };
    latest.0 = newer;
    commands.remove_resource::<CheckTask>();
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

/// GitHub's tag for the latest release, if it names a version newer than
/// this build. Any failure - offline, GitHub unreachable, a malformed
/// response - is silently treated as "nothing to report": this check is a
/// courtesy, never something the player has to wait on or see fail.
#[cfg(not(target_arch = "wasm32"))]
fn fetch_latest_tag() -> Option<String> {
    let release: Release = ureq::get(LATEST_RELEASE_URL)
        .header("User-Agent", "queens-update-check")
        .config()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;

    let candidate = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    is_newer(candidate, env!("CARGO_PKG_VERSION")).then_some(release.tag_name)
}

/// Compares plain `major.minor.patch` triples, which is all this project's
/// own version numbers ever are.
#[cfg(not(target_arch = "wasm32"))]
fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_semver(candidate), parse_semver(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_semver(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((major, minor, patch))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn a_higher_patch_counts_as_newer() {
        assert!(is_newer("0.2.6", "0.2.5"));
    }

    #[test]
    fn a_higher_minor_outranks_a_higher_patch_on_the_current_side() {
        assert!(is_newer("0.3.0", "0.2.9"));
    }

    #[test]
    fn an_equal_version_is_not_newer() {
        assert!(!is_newer("0.2.5", "0.2.5"));
    }

    #[test]
    fn an_older_version_is_not_newer() {
        assert!(!is_newer("0.2.4", "0.2.5"));
    }

    #[test]
    fn an_unparseable_tag_is_never_newer() {
        assert!(!is_newer("not-a-version", "0.2.5"));
        assert!(!is_newer("0.2.5", "not-a-version"));
    }

    #[test]
    fn a_tag_with_extra_parts_is_rejected_rather_than_truncated() {
        assert!(!is_newer("0.2.6-rc1", "0.2.5"));
    }
}
