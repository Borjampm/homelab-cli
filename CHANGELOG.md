# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.10](https://github.com/Borjampm/homelab-cli/compare/v0.1.9...v0.1.10) - 2026-03-10

### Added

- add --in flag to run commands in a specific directory

### Other

- Merge pull request #16 from Borjampm/add-in-directory-flag
- rename --in flag to --dir for clarity

## [0.1.9](https://github.com/Borjampm/homelab-cli/compare/v0.1.8...v0.1.9) - 2026-03-04

### Added

- add port management commands and auto-check in run

### Fixed

- ensure Docker lab is running before SSH commands in port tests

### Other

- replace atexit Docker teardown with justfile test recipe
- simplify port module by removing duplication
- add unit tests for extract_users_block and parse_process_entry
- break down parse_ss_output into smaller functions

## [0.1.8](https://github.com/Borjampm/homelab-cli/compare/v0.1.7...v0.1.8) - 2026-03-04

### Added

- add --include flag to override .gitignore exclusions in sync

### Other

- Merge pull request #12 from Borjampm/add-include-flag-to-sync
- address PR review comments

## [0.1.7](https://github.com/Borjampm/homelab-cli/compare/v0.1.6...v0.1.7) - 2026-03-03

### Fixed

- wrap remote commands in interactive shell to source .bashrc

## [0.1.6](https://github.com/Borjampm/homelab-cli/compare/v0.1.5...v0.1.6) - 2026-03-03

### Fixed

- kill port forward tunnels when setup command fails

## [0.1.5](https://github.com/Borjampm/homelab-cli/compare/v0.1.4...v0.1.5) - 2026-03-03

### Added

- add --setup flag to run command for pre-command setup steps

## [0.1.4](https://github.com/Borjampm/homelab-cli/compare/v0.1.3...v0.1.4) - 2026-03-03

### Other

- add development flow to CLAUDE.md

## [0.1.3](https://github.com/Borjampm/homelab-cli/compare/v0.1.2...v0.1.3) - 2026-03-03

### Other

- add help text to all CLI commands and flags

## [0.1.2](https://github.com/Borjampm/homelab-cli/compare/v0.1.1...v0.1.2) - 2026-03-03

### Fixed

- use cargo-dist generated release workflow

## [0.1.1](https://github.com/Borjampm/homelab-cli/compare/v0.1.0...v0.1.1) - 2026-03-03

### Fixed

- drop windows target and fix macos-13 runner in release workflow

## [0.1.0](https://github.com/Borjampm/homelab-cli/releases/tag/v0.1.0) - 2026-03-03

### Added

- add release pipeline with release-plz and cargo-dist

### Other

- start
