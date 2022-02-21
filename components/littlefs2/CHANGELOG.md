# Change Log

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## [Unreleased]

## [v0.2.2] - 2021-03-20

### Changed
- Added `remove_dir_all_when`, allowing to filter "rm -rf <path>"

## [v0.2.1] - 2021-02-26

### Changed
- PathBuf::from errors on embedded nuls, and prevents ending
  with nuls
- get rid of ufmt (oversight in 0.2 release)
- get rid of dead code (oversight in 0.2 release)

## [v0.2.0] - 2021-02-02

### Changed

- [breaking-change] The version of the `generic-array` dependency has been
  bumped to v0.14.2 (now that `heapless` v0.6.0` is out).

## [v0.1.1] - 2021-02-11

### Fixed

- `std`-triggering regression
