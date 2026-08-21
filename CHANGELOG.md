# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Typvia has not been released yet. Everything in the repository is pre-release
development, and this section will collect user-visible changes until the first
tagged version.

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## Versioning policy

Typvia follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html), with
the qualifications below.

**During 0.x, the minor version may carry breaking changes.** Semantic
Versioning does not guarantee stability before 1.0, and Typvia makes use of
that: while the product is still finding its shape, a `0.x` bump can change
behaviour, storage layout, or interfaces. Read the release notes before
upgrading. Once 1.0 ships, breaking changes will require a major version.

**The local database is migrated forward automatically; downgrades are not
supported.** On startup the app applies any outstanding schema migrations, each
in a single transaction, so an interrupted upgrade leaves the previous schema
intact rather than a half-migrated database. There is no reverse path: an older
build cannot migrate a newer database back, and pointing one at a database
written by a newer version is unsupported and may fail in unpredictable ways.
If you want to move back to an earlier version, restore a backup taken before
the upgrade.

**The sync protocol is versioned independently of the app.** The version is not
derived from the release number, so a client and a server on different app
versions can still talk to each other as long as their protocol versions are
compatible. The client sends its protocol version with every request, and the
server advertises the range it supports; when the two do not overlap, the
request is refused with an explicit "unsupported protocol version" error. A
mismatch therefore surfaces as a clear, actionable failure instead of writing
data that the other side cannot correctly interpret.

The stored data has its own version markers, separate again from the protocol:
the encrypted envelope format and the payload document schema are versioned on
their own. When a record's payload schema is newer than the reading client
understands, that record is set aside and retried on later sync rounds rather
than being dropped or partially applied, so upgrading the lagging client is
enough to pick it up.

## Adding an entry

Changes that a user would notice belong here. Add them to `## [Unreleased]`
under the appropriate heading as part of the pull request that makes the change.
See `CONTRIBUTING.md` for details.
