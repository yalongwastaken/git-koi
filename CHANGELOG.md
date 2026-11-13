# Changelog

Notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A pond around the koi: each of the last seven days with an issue opened floats
  a lily pad, and each with a pull request opened settles a rock, sized by the
  day's count. `git-koi collect` records the week's counts in snapshot version
  4, `git-koi render` draws rocks under the koi and lily pads over it, and the
  render caption names them. Older snapshots still render, without a pond.
- Water: a wake of ripples trails the tail with each stroke and drifts back and
  fades through the glide, as much as the day's contributions earn, faintly
  while the koi rests or feeds and never in deep rest; and marks seeded by the
  account shimmer across the water while it swims, as many as the week's
  contributions earn. The still shows neither.

## [1.0.2] - 2026-10-31

### Fixed

- `git-koi collect` requests again any contribution days GitHub leaves out of an
  answer, instead of failing with "missing contribution date". GitHub left out
  days for some accounts whose history crosses a February 29 (#52).

## [1.0.1] - 2026-10-28

### Changed

- The Action is listed as `git-profile-koi`, since a GitHub user already holds
  the name `git-koi`, and its description fits the Marketplace limit.

## [1.0.0] - 2026-10-28

### Added

- `git-koi collect`, which gathers an account's public snapshot through the
  GitHub CLI, and `--help` and `--version` on the command.
- `git-koi render`, which draws the day's koi as a looping GIF, a still PNG, and
  a preview page, in six bodies.
- A GitHub Action that downloads the release binary and runs both inside a
  profile repository's workflow.
- The `git-koi` library: account history, koi development, behavior, motion,
  markings, rendering, and encoding.

[Unreleased]: https://github.com/yalongwastaken/git-koi/compare/v1.0.2...HEAD
[1.0.2]: https://github.com/yalongwastaken/git-koi/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/yalongwastaken/git-koi/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/yalongwastaken/git-koi/releases/tag/v1.0.0
