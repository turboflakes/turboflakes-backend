# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.18] - 2021-09-15

### Change
  
- Update substrate_ext dependency
- Fix 'EraPaid' on-chain event

## [0.7.17] - 2021-09-10

### Change
  
- Update parity-scale-codec dependency
- Fix dead code warning
- Reverse changelog file

## [0.7.16] - 2021-09-09

### Change
  
- Update substrate_ext dependency

## [0.7.15] - 2021-08-19

### Change
  
- Update substrate_ext dependency

## [0.7.14] - 2021-07-24

### Change
  
- Update substrate_ext dependency

## [0.7.13] - 2021-07-18

### Add

- Cache points collected per validaotr per era

### Change
  
- Update substrate_ext dependency

## [0.7.12] - 2021-06-27

### Change
  
- Update substrate dependencies

## [0.7.10] - 2021-05-30

### Change
  
- Fix create or await for substrate client to be ready when setting connection

## [0.7.9] - 2021-05-29

### Change
  
- Fix event subscription related to substrate-subxt dependencies

## [0.7.8] - 2021-05-27

### Change
  
- Review log messages

## [0.7.7] - 2021-05-25

### Change
  
- Fix uncomment generate board

## [0.7.6] - 2021-05-25

### Change
  
- Revert and just use the maximun and minimum to calculate the partial scores.

## [0.7.4] - 2021-05-25

### Change
  
- To optimize the scores use 95% confidence interval calculation to obtain the maximum and minimum during normalization

## [0.7.3] - 2021-05-25

### Change
  
- Fix validator rank response. Send status information

## [0.7.1] - 2021-05-25

### Change
  
- Fix nominations score by reverse normalization value

## [0.7.0] - 2021-05-24

### Change
  
- Fix do not process new events if cache is syncing

### Add

- Include nominations and total stake to the score calculation

## [0.6.2] - 2021-05-24

### Change
  
- Fix cache timeout
- Check if cache keys are available in rank endpoint

## [0.6.1] - 2021-05-24

### Change
  
- Fix commission score

## [0.6.0] - 2021-05-23

### Add
  
- Cache scores and limits
- Add scores and limits to Rank endpoint response

## [0.5.6] - 2021-05-17

### Change
  
- Fix cargo dependencies

## [0.5.5] - 2021-05-17

### Change
  
- Only make a full sync at era payout.
- Only generate leaderboard when cache syncing is finished.
- Draft leaderboard stats counter

## [0.5.4] - 2021-05-16

### Change
  
- Handle validator rank in a better way. If board not available wait.

## [0.5.3] - 2021-05-15

### Change
  
- Change CORS

## [0.5.0] - 2021-05-14

### Added
  
- Add validator rank endpoint to get the rank of a validator address in a specific board
- Change error response messages

## [0.4.7] - 2021-05-11

### Changed

- Fix subscribe new session events typo

## [0.4.6] - 2021-05-11

### Changed

- Fix uncomment history code :)

## [0.4.5] - 2021-05-11

### Changed

- Fix cache empty value result

## [0.4.4] - 2021-05-10

### Changed

- Only sync validators with bonded controller

## [0.4.3] - 2021-05-10

### Changed

- Fix dependencies

## [0.4.2] - 2021-05-10

### Changed

- Sync all nominators and respective stake for all validators
- Subscribe new session events to sync validators and nominators

## [0.4.1] - 2021-05-08

### Changed

- Fix dependencies

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

## [0.3.1] - 2021-05-04

### Changed

- Fix mean for an empty list

## [0.3.2] - 2021-05-05

### Changed

- Fix inclusion calculation

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

## [0.2.2] - 2021-04-29

### Added

- Await for Redis to be ready during restarts

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

## [0.1.0] - 2021-04-19

- First release
