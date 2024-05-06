# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# [Unreleased]

- Bump `cw-dex-astroport` to `0.2.0-rc.1`.

# [0.4.0] - 2024-03-08

- Replace `cw-dex/astroport` with `cw-dex-astroport` and `cw-dex/osmosis` with `cw-dex-osmosis`.
- Bump cw-it and cw-dex.

# [0.3.1] - 2024-02-06

- Bump `cw-dex` to version `0.5.1`.

# [0.3.0] - 2023-10-26

### Changed

- Bump `cw-dex` to version `0.5.0`.

# [0.2.0] - 2023-09-27

### Fixed

- Return Ok if amount to swap is zero in `execute_swap_operation`. This fixes a bug where the swap operation would fail if the amount to swap was zero, which might happen when basket liquidating assets with overlapping paths.

### Changes

- Bump cw-dex to version `0.4.0` and make relevant API changes.
  - NB: This is a breaking change.
