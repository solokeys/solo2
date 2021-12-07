# Maintenance Guide

This guide explains how to keep this repository up to date and how to make releases.

## Updating the Repository

### Upstream Changes

Regularly check the [solo2](https://github.com/solokeys/solo2) for relevant changes and merge them into this repository.  Before pushing the merge, make sure to run the build so that the `Cargo.lock` file is updated.  If it changed, add it to the merge commit.

### Dependencies

As we have a `Cargo.lock` file with fixed dependency versions, we don’t automatically pull in new dependency versions.  To update the version of a specific dependency, run `cargo update -p <name>`.  To update all dependencies, run `cargo update`.

## Releasing

### Creating Releases

To release a new version of the firmware, perform the following steps:
1. Update the version counter in `runners/lpc55/Cargo.toml`.
2. Run the firmware build.
3. Update the changelog.
4. Update the `runners/lpc55/config/commands.bd` file:
   - The `productVersion` and `componentVersion` should be set to the current firmware version.
   - The `version_check sec` and `version_check nsec` values are derived from the firmware version.  You can find the correct values in the `build_constants.rs` file in the Rust output directory or you can calculate them by hand using the algorithm shown in `runners/lpc55/build.rs` (variable `version_to_check`).
5. Commit all changed files and create a signed tag with a `v` prefix and the version number, for example `v1.0.0`.
6. Create a release on GitHub and copy the relevant section from the changelog to the release description.

### Signing Releases

1. Download the `firmware-nk3xn.bin` as built by the CI from the release tag.
2. Download the [`runners/lpc55/commands.bd`][] for the release tag.
3. Sign the firmware and build a SB2.1 image using the `commands.bd` file.
4. Upload the SB2.1 image to the GitHub release using the filename pattern `firmware-<device>-<chip>-v<version>.sb2`, for example `firmware-nk3xn-lpc55-v1.0.0.sb2`.

[`runners/lpc55/commands.bd`]: ./runners/lpc55/commands.bd
