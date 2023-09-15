# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# [Unreleased]

### Fixed

- Return Ok if amount to swap is zero in `execute_swap_operation`. This fixes a bug where the swap operation would fail if the amount to swap was zero, which might happen when basket liquidating assets with overlapping paths.

### Changes

- [PR #26](https://github.com/apollodao/cw-dex-router/pull/26): Bump cw-dex to version `0.4.0-rc.1` and make relevant API changes.
  - NB: This is a breaking change.
