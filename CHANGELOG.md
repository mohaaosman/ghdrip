# Changelog

All notable changes to this project are documented here.

## [Unreleased]

### Added

- Automated release builds for macOS and Linux.
- `--no-avatar` and `--no-color` flags.
- Animated contribution calendar with a year selector, configurable under `[contributions]`.
- `--year`, `--no-interactive`, `--no-contributions` and `--no-animation` flags.
- Clickable links in the bio (urls and @mentions), blog and Twitter fields.

## Changed

- Switched from a match to using clap for cli management.
- Renamed to ghdrip: binary `ghdrip`, token `GHDRIP_TOKEN`, config `~/.config/ghdrip/config.toml`.
- The year picker opens by default in a terminal instead of behind `--interactive`.

## Fixed

- Year picker no longer quits straight away when the terminal answers the avatar image commands.
- A missing or broken avatar no longer stops the profile from printing.

- Made "break" field be removed when it is the last line in the fields to maintain consistency of one line gap.

## [1.0.1] - 2026-09-22

This release was made as i messed up the first release and the ghfetch version is still v1.0.0

## [1.0.0] - 2026-09-22

### Added

- Initial release of ghfetch.
- Configurable fields and colors.
- Configurable terminal profile image.
- Kitty Graphics Protocol image rendering.
- GitHub token support.
- `--help` and `--version` options.

### Changed

- Improved GitHub API handling.
- Improved configuration handling.
