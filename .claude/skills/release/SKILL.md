---
name: release
description: Use when the user asks to cut, tag, bump or ship a release (e.g. "bump to 0.2.5 and release", "tag this", "ship a new version") — bumps the workspace version, verifies, commits, dry-runs packaging, then tags and pushes in the right order.
---

# Release

Only run this when the user explicitly asks for a release and, if they didn't
already state a version, ask them for one — don't guess a semver bump size
(patch vs. minor vs. major). Pushing a tag is a public, hard-to-reverse action
that kicks off CI's build and release jobs, not something to do speculatively.

## 1. Bump the version

The whole workspace shares one version, in the root `Cargo.toml`:

```toml
[workspace.package]
version = "X.Y.Z"
```

Every crate (`queens_core`, `queens_cli`, `queens_app`) inherits it via
`version.workspace = true` — there is nothing else to edit.

```bash
cargo check --workspace   # refreshes the versions Cargo.lock records
```

## 2. Run the local gate

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All three must pass — fix and re-run rather than tagging around a failure.
If this release also carries generator/solver or UI changes that haven't
been verified yet, follow `CLAUDE.md`'s "Verifying changes" section first
(the `queens-gen bench` audit, or the scripted `QUEENS_CAPTURE` run) — a
release is exactly the wrong moment to skip that.

## 3. Commit

Two commits, matching this repo's history (check `git log --oneline` if
unsure of message style):

1. Any feature or fix work, with its own message — commit this *before* the
   version bump if it isn't committed already. Don't fold it into the bump.
2. The version bump alone, touching only `Cargo.toml` and `Cargo.lock`:

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: release X.Y.Z"
```

It needs to be its own commit because that is what the tag in step 5 points
at — mixing in unrelated changes makes the tag point at more than the release.

## 4. Dry-run the packaging build

`.github/workflows/ci.yml`'s `build` job — the release binaries, and macOS
signing/notarizing — only ever runs for a `v*` tag or a manual dispatch, so a
packaging mistake otherwise surfaces for the first time *at* the tag, which is
the most annoying possible moment to find it. Push the release commit, then
trigger the workflow by hand before tagging:

```bash
git push
gh workflow run ci.yml --ref master
gh run watch     # or: gh run list --workflow=ci.yml
```

Confirm `build` is green on all three platforms (macOS, Linux, Windows)
before moving on. (For a pull request that touches `packaging/` or the
workflow itself, adding a `packaging` label runs the same job on the PR, so
that class of mistake need not wait for release day at all.)

The `release` job, which actually publishes a GitHub Release, stays gated on
the tag itself, so this dry run cannot publish anything by accident. If `gh`
isn't authenticated for Actions here, use the "Run workflow" button on the CI
workflow's page instead.

## 5. Tag and push

Annotated tags, `vX.Y.Z`, pointing at the release commit:

```bash
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

This is the step that actually publishes the release — don't take it before
step 4 is green.

## If a mistake surfaces

Don't rewrite pushed history. This repo's own log shows the pattern to
follow — a wrong version bump (`chore: release 0.3.0`) was caught and fixed
with a new commit rather than an amend (`chore: correct the release to
0.2.4, not 0.3.0`). Do the same: fix forward with a new commit. If the tag
itself was already pushed with the wrong version, only delete and recreate
it (`git push origin :refs/tags/vX.Y.Z`) after checking nothing has already
consumed it — a download, a dependent build, a released binary.
