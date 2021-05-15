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

## [0.4.0] - 2021-05-08

### Added

- Generate boards from user defined weights
- Add stakers clipped
- Add number of stakers and clipped stakers
- Add number of valid judgements
- Add number of sub-accounts

### Changed

- Change active and all validators set to sorted set
- Rename mean by average or short avg
- Name generic board names
- Normalize average reward points based on historic era points
- Change Weights up to 8 characteristics

## [0.4.1] - 2021-05-08

### Changed

- Fix dependencies

## [0.4.2] - 2021-05-10

### Changed

- Sync all nominators and respective stake for all validators
- Subscribe new session events to sync validators and nominators

## [0.4.3] - 2021-05-10

### Changed

- Fix dependencies

## [0.4.4] - 2021-05-10

### Changed

- Only sync validators with bonded controller

## [0.4.5] - 2021-05-11

### Changed

- Fix cache empty value result

## [0.4.6] - 2021-05-11

### Changed

- Fix uncomment history code :)

## [0.4.7] - 2021-05-11

### Changed

- Fix subscribe new session events typo

## [0.5.0] - 2021-05-14

### Added
  
- Add validator rank endpoint to get the rank of a validator address in a specific board
- Change error response messages

## [0.5.3] - 2021-05-15

### Change
  
- Change CORS
