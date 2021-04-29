# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2021-04-19

- First release

## [0.2.0] - 2021-04-28

### Added

- Validator identity
- Validator inclusion rate
- Validator average reward points
- Validator reward staked flag
- Skip eras already synced
- Subscribe for Staking::EraPayout event
- CI release in github actions
- Use version script to bump new versions based on [bump-version.sh](https://gist.github.com/paulormart/e8c8e659f78d0ef6a497f22b41d814f9)

## [0.2.1] - 2021-04-29

### Added

- Await for Redis to be ready during restarts
