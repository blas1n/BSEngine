# `deno_path_util`

Common path utilities used across Deno's repos.

## Versioning Strategy

This crate does not follow semver so if you're outside the Deno org make sure to pin it to a patch version.
Instead a versioning strategy that optimizes for more efficient maintenance is
used:

- Do the dependencies of [Deno](https://github.com/denoland/deno) compile?
  - If yes, it's a patch release.
  - If no, it's a minor release.
