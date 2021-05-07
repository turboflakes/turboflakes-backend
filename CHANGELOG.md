# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Generate boards from user defined weights
- Change active and all validators set to sorted set
- Add stakers clipped
- Add number of stakers and clipped stakers

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

## [0.2.2] - 2021-04-29

### Added

- Await for Redis to be ready during restarts

## [0.3.0] - 2021-05-04

### Changed

- Fix inclusion rate and average reward points for a validator
- Setup Cors and configurable allowed origin header
- Fix closed subcriptions
- Change health endpoint remove version parameter
- Change all cache keys to an Enum with all possible variants
- Restart subscription and history on error

### Added

- Add Index endpoint with service information
- Add query parameters to validator endpoint

## [0.3.1] - 2021-05-04

### Changed

- Fix mean for an empty list

## [0.3.2] - 2021-05-05

### Changed

- Fix inclusion calculation
