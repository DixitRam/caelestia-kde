# Releases

A release is built once and promoted, never rebuilt per stage. The build graph and the
publish step are separate workflows so the graph can run anywhere:

| Workflow | Runs on | Publishes |
| --- | --- | --- |
| [release-assets.yml](../.github/workflows/release-assets.yml) | anything, including a pull request | nothing - uploads workflow artifacts |
| [version-release.yml](../.github/workflows/version-release.yml) | push to `main`, dispatch, or a call | a tag and a GitHub release |
| [release-rehearsal.yml](../.github/workflows/release-rehearsal.yml) | weekly schedule and dispatch | a `rehearsal-<sha>` prerelease |

> Paths above are relative to this file's parent, which is `docs/`.

## Channels

`version-release.yml` takes a `channel` input:

- **`rehearsal`** - builds and verifies the tree under test, then publishes it as a
  prerelease under `rehearsal-<short-sha>`. Never marked latest. This is the simulated push
  to `main`.
- **`rc`** - publishes the version as `rc-v<version>`, also a prerelease. Use it to look at
  the assets and the generated notes before promoting.
- **`stable`** - the push to `main`: tag `v<version>`, marked latest. On success it deletes
  the `rc-v<version>` it was promoted from and opens the sync PR back to `dev`.

A dispatch with `publish: false` runs the whole build graph and every artifact check but
creates no tag and no release.

### Why the tag names matter

The shell's update checker walks local `refs/tags/v*` and treats the newest as the release
([UpdateChecker.qml](../shell/services/UpdateChecker.qml)). Anything that must not be
offered to users therefore has to avoid the `v` prefix, which is why a rehearsal tag is
`rehearsal-<sha>` and a candidate is `rc-v<version>`. `git describe --match 'v*'` in the
release notes and in the shell's CMake version detection rely on the same rule.

## The order a release happens in

1. `release-assets.yml` builds the shell archive, the AUR source tarball, the vendored crate
   tree and both installer binaries. Each is uploaded as a workflow artifact; the run has no
   write permission.
2. `verify-shell-archive` extracts the archive, lays it out the way the installer deploys it
   (`etc/xdg/quickshell/caelestia`, `usr/lib/qt6/qml`, `usr/lib/caelestia`) and runs the
   headless runtime smoke against those bytes. A build tree passing the smoke says nothing
   about the archive.
3. `verify-pkgbuild` (opt-in, and always on a rehearsal) builds the AUR package from the
   tarballs this run produced, using the PKGBUILD's `_source_url`/`_vendor_url` overrides.
4. Only then does `publish` run, behind the `release` environment. It creates the release as
   a **draft**, uploads every asset, and makes it visible last. A failed build can no longer
   leave a live release with missing files.

## Rehearsing a release

```bash
# Build, verify and publish a prerelease for dev's tip
gh workflow run release-rehearsal.yml -f ref=dev -f publish=true

# Build and verify only, for any branch
gh workflow run version-release.yml -f channel=rehearsal -f ref=my-branch -f publish=false
```

To install what a rehearsal published, point the launcher at it. Both overrides are unset in
every normal install:

```bash
CAELESTIA_RELEASES_BASE=https://github.com/<owner>/<repo>/releases/download \
CAELESTIA_RELEASE_TAG=rehearsal-<sha> ./install.sh
```

`CAELESTIA_RELEASES_BASE` moves the download root (a fork's releases), and
`CAELESTIA_RELEASE_TAG` names the tag to read instead of the one in `.github/version.env`,
which is what lets a rehearsal checkout use the prebuilt shell path at all.

## Promoting

1. Bump `.github/version.env` (and `installer/data/tui.version` if the installer changed).
2. Dispatch `version-release.yml` with `channel=rc` on the release branch and check the
   draft.
3. Merge to `main`. The push runs the same graph with `channel=stable`.

`verify-pkgbuild` warns when the PKGBUILD's `pkgver` has not been bumped to match
`.github/version.env`; it builds a copy carrying the tree's version so the source build is
still exercised.
