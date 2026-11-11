# Release Guide

## Overview

Development happens directly on the `main` branch. During development, the version in `Cargo.toml` is set to `X.Y.Z-dev`. When ready to release, the `-dev` suffix is removed, the release is tagged, and the version is bumped to the next dev version.

Versioning: nfcd started at `0.1.0`, independent of nfd2nfc's version numbers. While on `0.x`, bump the minor version for breaking changes (e.g. config format) and major features (e.g. the menu bar app in `0.2.0`), and the patch version for everything else. `1.0.0` comes once the GUI lands and the config format is stable.

## Release Steps

### 1. Finalize the version

Remove the `-dev` suffix from the version in `Cargo.toml`, then commit:

```bash
# Edit Cargo.toml: "X.Y.Z-dev" → "X.Y.Z"
cargo check
git add Cargo.toml Cargo.lock
git commit -m "chore: release vX.Y.Z"
git push origin main
```

### 2. Tag and push

Pushing a `v*` tag triggers the automated release pipeline (GitHub Release, then the menu bar app build):

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

### 3. Write release notes

After the automated pipeline creates the GitHub Release, update the auto-generated release notes.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Available sections:

- **Added** — new features
- **Changed** — changes in existing functionality (improvements, refactoring, docs, etc.)
- **Deprecated** — soon-to-be removed features
- **Removed** — removed features
- **Fixed** — bug fixes
- **Security** — vulnerability fixes

Omit sections with no entries. Additional notes (e.g., git history rewrite warnings) can be written outside the sections.

```bash
gh release edit vX.Y.Z --notes "$(cat <<'EOF'
## What's Changed

### Added
- ...

### Changed
- ...

### Fixed
- ...

**Full Changelog**: https://github.com/chanungdev/nfcd/compare/vPREV...vX.Y.Z
EOF
)"
```

### 4. Set the next dev version

Bump to the next development version:

```bash
# Edit Cargo.toml: "X.Y.Z" → "X.Y.(Z+1)-dev"
cargo check
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to X.Y.(Z+1)-dev"
git push origin main
```

### 5. Update the Homebrew tap

The formula lives in [chanungdev/homebrew-tap](https://github.com/chanungdev/homebrew-tap) (`Formula/nfcd.rb`). Point it at the new tag:

```bash
curl -sL https://github.com/chanungdev/nfcd/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256
# In homebrew-tap: update `url` and `sha256` in Formula/nfcd.rb, then commit and push
git commit -am "nfcd X.Y.Z" && git push
```

Then point the cask at the app built for the same tag. Wait until the `gui` job of the Release workflow has uploaded `nfcd-app.zip`:

```bash
curl -sL https://github.com/chanungdev/nfcd/releases/download/vX.Y.Z/nfcd-app.zip | shasum -a 256
# In homebrew-tap: update `version` and `sha256` in Casks/nfcd-app.rb, then commit and push
git commit -am "nfcd-app X.Y.Z" && git push
```

Verify locally:

```bash
brew update && brew upgrade nfcd nfcd-app
brew test nfcd
```

The app version comes from the tag (`--config` in `release.yml`), so `nfcd-gui/` versions don't need bumping.

## Automated Pipeline

The `release.yml` workflow runs automatically when a `v*` tag is pushed. It creates a GitHub Release with auto-generated release notes, then builds a universal `nfcd.app` and uploads it as `nfcd-app.zip`. RC tags (e.g., `v0.2.0-rc.1`) are marked as prerelease.

## Post-release Checklist

- [ ] Verify the [GitHub Release](https://github.com/chanungdev/nfcd/releases) was created
- [ ] Write release notes (Step 3) and verify with `gh release view vX.Y.Z`
- [ ] Update the tap formula and cask (Step 5) and confirm: `brew update && brew info chanungdev/tap/nfcd chanungdev/tap/nfcd-app`
- [ ] Confirm installation works: `brew upgrade nfcd`

## Updating Screenshots

Screenshots are hosted as GitHub Release assets (not tracked in git).
Local working files are in the `assets/` directory (gitignored).

```bash
# Update specific images (overwrites existing)
gh release upload assets assets/home.png assets/config.png --clobber

# Or update all screenshots at once
gh release upload assets assets/*.png --clobber
```
