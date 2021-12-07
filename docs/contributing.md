# Contributing Guide

This guide contains some guidelines for contributions to this repository.

- If your contribution includes changes to dependencies, make sure to include the updated `Cargo.lock` file.
- Please sign all commits to this repository with your OpenPGP key.
- We try to use the published versions of all dependencies.  So if your contribution requires a change to a dependency, please try to submit that change to upstream.  Only if this is not possible in time, we can fork a dependency with the `Nitrokey` organization and overwrite it in the `Cargo.toml` file like this:
  ```
  [patch."https://github.com/trussed-dev/trussed"]
  trussed = { git = "https://github.com/Nitrokey/trussed", branch = "wink" }
  ```
- If your change is not specific to the Nitrokey 3, consider submitting it to the `solo2` repository instead.  If you want to submit it to the `nitrokey-3-firmware` repository, make sure to choose the correct repository when creating a PR.
- Please update the changelog with a description of your changes.
