# Contributing to Typvia

Typvia is pre-1.0 and has no usable release yet. The architecture, data model,
and security model are settled enough to build on, but interfaces still change
without deprecation cycles. Please read this file top to bottom before your
first pull request — it should take you from a clone to a green set of checks.

## What is useful right now

| Welcome                         | Notes                                                          |
| ------------------------------- | -------------------------------------------------------------- |
| Bug reports with a reproduction | The most valuable contribution at this stage.                  |
| Small, focused fixes            | Correctness, error handling, platform edge cases, flaky tests. |
| Test coverage                   | Especially for the security red lines listed further down.     |
| Platform work                   | Windows/Linux desktop issues, iOS keyboard, Android IME.       |
| Documentation of real behavior  | Setup steps, self-hosting, packaging.                          |
| Self-hosting improvements       | The sync server and the templates under `deploy/`.             |

**Open an issue before starting anything large** — a new feature, a new
dependency, a refactor that crosses crate boundaries, or a UI change. The
product scope and the visual design system are deliberately narrow, and a large
patch that does not match them will be turned down no matter how good the code
is. An issue first saves you that.

Not accepted: adding a UI framework or icon library, adding a second way to do
something the codebase already does, or reworking the licence layout.

## Prerequisites

| Tool    | Version                      | Where it is pinned                                          |
| ------- | ---------------------------- | ----------------------------------------------------------- |
| Node.js | 22.22.0 or newer             | `engines` in `package.json`; CI uses Node 22                |
| pnpm    | 10.30.3                      | `packageManager` in `package.json`                          |
| Rust    | stable, edition 2024 (1.85+) | `Cargo.toml` workspace package, `rustfmt.toml`              |
| Go      | 1.25.13 or newer             | `apps/sync-server/go.mod` (only needed for the sync server) |

The pinned pnpm version is picked up automatically by Corepack:

```sh
corepack enable
node --version    # >= 22.22.0
pnpm --version    # 10.30.3
rustc --version   # stable, 1.85 or newer
```

Rust components `rustfmt` and `clippy` are required — CI installs both.

### Per-target extras

| Target                                         | Additionally needs                                                                                  |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Desktop on Linux                               | `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`, `libgtk-3-dev` |
| Desktop on macOS                               | Xcode command line tools (Tauri builds against WebKit)                                              |
| Desktop on Windows                             | An MSVC C toolchain — the bundled SQLite is compiled from source                                    |
| iOS app, keyboard, share and widget extensions | Xcode; the app targets iOS 16.0                                                                     |
| Android app and IME                            | Android SDK (compileSdk 36, minSdk 28), NDK r26 or newer via `ANDROID_NDK_HOME`, JDK 17             |
| Sync server                                    | Go only; no CGO — the SQLite driver is pure Go                                                      |

The Linux list is exactly what the CI bundling job installs:

```sh
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev patchelf libgtk-3-dev
```

You do not need the mobile toolchains to work on the core, the desktop app, or
the server. Install them only when you touch those targets.

## Getting set up

```sh
git clone https://github.com/leazoot/Typvia.git
cd typvia
pnpm install --frozen-lockfile
```

Run the desktop app in development (Tauri dev server plus the Rust host):

```sh
pnpm dev                              # = pnpm --filter @typvia/desktop dev
```

Run the sync server locally. It listens on `127.0.0.1:8787` and writes
`typvia-sync.db` into the working directory:

```sh
cd apps/sync-server
go run ./cmd/typvia-sync-server
curl http://127.0.0.1:8787/healthz    # {"status":"ok"}
```

Only two settings exist: `TYPVIA_SYNC_LISTEN` and `TYPVIA_SYNC_DB` (the
`-listen` and `-db` flags take precedence).

Container and self-hosting templates live under `deploy/` —
[`deploy/docker/README.md`](deploy/docker/README.md) covers both the
build-from-source compose file and the published-image one, and
`deploy/cloudflare/` covers the zero-open-port tunnel shape. To build and run
the server image from a source checkout, from the repository root:

```sh
docker compose -f deploy/docker/docker-compose.yml up -d --build
```

Building a desktop bundle (this is what CI produces on Linux):

```sh
pnpm --dir apps/desktop tauri build --bundles deb appimage
```

The mobile FFI artifacts are generated, never committed. Regenerate them with
`crates/mobile-ffi/build-android.sh` (Kotlin bindings plus per-ABI `.so`) and
`crates/mobile-ffi/build-xcframework.sh` (Swift xcframework). The Android Gradle
module runs the first script for you as part of `preBuild`.

## Checks before you open a pull request

Run all five from the repository root. They are the same commands CI runs, in
the same order.

```sh
pnpm lint
pnpm format
pnpm typecheck
pnpm test
pnpm build
```

| Command          | What it covers                                                                                           |
| ---------------- | -------------------------------------------------------------------------------------------------------- |
| `pnpm lint`      | `cargo clippy --workspace --all-targets -- -D warnings` and `eslint .`. Any warning fails.               |
| `pnpm format`    | `cargo fmt --all --check` and `prettier --check .`. Fix with `pnpm format:write`.                        |
| `pnpm typecheck` | `tsc --noEmit` in every workspace package that defines a `typecheck` script. A new package must add one. |
| `pnpm test`      | `cargo test --workspace` and `vitest run`.                                                               |
| `pnpm build`     | `cargo build --workspace` and each package's build script.                                               |

Each has a per-language half if you want a faster loop: `lint:rust`,
`lint:js`, `format:rust`, `format:js`, `test:rust`, `test:js`, `build:rust`,
`build:js`.

### Dependency and secret checks

CI runs these in separate jobs; run them yourself whenever you add, remove, or
bump a dependency.

```sh
cargo install --locked cargo-audit cargo-deny    # once
cargo audit                                      # Rust advisories
cargo deny check                                 # dependency licence gate
pnpm audit                                       # JS advisories
cd apps/sync-server && go run golang.org/x/vuln/cmd/govulncheck@latest ./...
```

`cargo deny check` is the dependency-licence gate, configured in `deny.toml`.
The clients and every shared crate are MPL-2.0 while the sync server is
AGPL-3.0, so a strong-copyleft crate entering the client tree is a real licence
conflict, not paperwork — the gate rejects it before it lands. Licences are
allow-listed, which means a dependency carrying a licence nobody has reviewed
yet fails by design and needs a human decision. CI runs the licence, bans, and
sources checks; the plain `cargo deny check` above also re-runs the advisory
check that `cargo audit` covers.

A new dependency has to justify itself in the pull request as well: why nothing
already in the tree does the job, that it is maintained, and that it does not
duplicate an existing capability.

Secret scanning uses gitleaks. CI scans the full history on every pull request;
locally:

```sh
brew install gitleaks                 # or your platform's package
scripts/security/secret_scan.sh       # scans history and working tree, redacted
```

### Area-specific checks

| If you touched                    | Also run                                                                                                                                                                                                           |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| The sync server                   | `go test ./...` in `apps/sync-server`. The root `pnpm test` does not cover Go, and CI currently only runs the Go vulnerability scan — so server tests are on you.                                                  |
| Search or the data layer hot path | `scripts/perf/search_bench.sh` (wraps `cargo bench -p typvia-search`). Red lines: 10k snippets under 50ms, 50k under 150ms; more than 20% slower than the recorded baseline counts as a failure.                   |
| Anything with a security red line | The guarding tests are ordinary `cargo test` cases and must stay green. Never skip or relax one.                                                                                                                   |
| Desktop OS integration            | The opt-in end-to-end scripts under `apps/desktop/e2e/` (macOS, GUI session, accessibility permission granted, app running under `tauri dev`). They are not part of headless CI; see the README in that directory. |

### CI

| Job             | Runs on | When                                                                                                                                                   |
| --------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `checks`        | Ubuntu  | Every push to `main` and every pull request                                                                                                            |
| `audit`         | Ubuntu  | Every push to `main` and every pull request                                                                                                            |
| `secrets`       | Ubuntu  | Every push to `main` and every pull request, over the full history                                                                                     |
| `linux-bundle`  | Ubuntu  | Push to `main` only — produces `.deb` and AppImage artifacts                                                                                           |
| `windows-check` | Windows | Push to `main` only — `cargo check -p typvia-desktop -p typvia-browser-host`, because the registry-based native-messaging registration is Windows-only |

**CI must be green.** Passing locally and failing in CI counts as not done. Do
not delete a test, mark it skipped, weaken an assertion, or disable a lint rule
to get a check to pass.

## Repository layout

| Path                     | Contents                                                                |
| ------------------------ | ----------------------------------------------------------------------- |
| `apps/desktop`           | Tauri 2 desktop app (Windows/macOS/Linux)                               |
| `apps/mobile`            | Tauri 2 mobile app (iOS/Android)                                        |
| `apps/sync-server`       | Go sync server — ciphertext and metadata only, self-hostable            |
| `apps/browser-host`      | Native-messaging host for the browser extension                         |
| `apps/browser-extension` | Browser extension front end                                             |
| `crates/core`            | Data model, migrations, repositories, use-case orchestration            |
| `crates/crypto`          | Key derivation, encryption, secure-storage traits                       |
| `crates/search`          | SQLite FTS5 index and weighted ranking                                  |
| `crates/semantic`        | Local embedding search                                                  |
| `crates/sync`            | End-to-end encrypted sync client                                        |
| `crates/template`        | Template parsing and rendering                                          |
| `crates/espanso-adapter` | Expansion-engine config generation and process lifecycle                |
| `crates/ai`              | AI provider integration                                                 |
| `crates/host-service`    | Shared IPC orchestration used by the app hosts                          |
| `crates/mobile-ffi`      | UniFFI surface for the Swift and Kotlin layers                          |
| `packages/shared`        | Shared TypeScript types and the typed IPC layer                         |
| `packages/ui`            | Shared React component library and design tokens                        |
| `native/`                | iOS keyboard, share and widget extensions; Android IME and share target |
| `deploy/`                | Self-hosting templates for the sync server                              |
| `scripts/`               | Performance probes, release packaging, local secret scan                |

### Fixed dependency direction

```text
espanso-adapter, sync, search, semantic, template, ai  ->  core  ->  crypto
host-service  ->  core, search, template               (used only by app hosts)
```

Rules that reviewers check first, because violating them means a rewrite rather
than a fix:

- Feature crates depend on `core`; `core` depends on `crypto`. Crates at the
  same level never depend on each other.
- `core` is platform-free. Tauri, Swift, and Kotlin types never appear in any
  crate. `crypto` knows nothing about business models.
- Business logic lives in `core`. A Tauri command parses its arguments, calls
  into `core`, and maps the error — nothing else.
- The Swift and Kotlin layers do UI and system APIs only. They never implement
  business rules; they reach the core through FFI or a read-only snapshot.
- `host-service` orchestrates IPC for the app hosts and knows nothing about
  Tauri types.

## Code standards

These are enforced in review, with the reason attached so you can tell when an
exception is real.

### Comments and identifiers

- Comments are English and explain constraints the code cannot express —
  invariants, ordering requirements, why the obvious approach is wrong. Do not
  restate the code.
- No planning comments ("implement later", "next milestone will…"). If work is
  outstanding, it belongs in an issue. A `TODO` is acceptable only with a
  specific issue number attached.
- No internal task, decision, batch, stage, or question identifiers anywhere in
  code, tests, commit messages, CI configuration, or deployment files.
  Traceability belongs in the tracker; a comment states the constraint itself.

### Rust

- No `unwrap()` or `expect()` on a reachable runtime path. Tests are exempt, and
  the lint configuration already reflects that. An `expect` that asserts an
  invariant must document the invariant.
- One error type, three classes: user errors (recoverable, shown as guidance),
  business errors (conflicts, validation failures), system errors (IO, database,
  network). They map to stable codes across the IPC boundary.
- Every external input is validated at the boundary before it reaches `core`:
  IPC arguments, imported files, sync payloads, AI responses, config files read
  back from disk. Parse failures are business errors, never panics.
- A failed write leaves no half-written state. Use a transaction; on failure the
  previous state must survive intact.
- Public items carry English doc comments. Keep the public surface minimal —
  `pub(crate)` unless something outside the crate genuinely needs it.
- Sensitive values are zeroized, kept out of long-lived caches, and redacted in
  `Debug`.

### Database

- SQL lives only in the repository layer. Business code never assembles a query.
- Every query is parameterized. String-concatenated SQL is an injection defect,
  not a style preference.
- Schema changes go through migrations with paired upgrade and rollback, tested
  for upgrade, rollback, and re-running safely. No runtime DDL. A migration that
  has landed on `main` is never edited — add a new one.
- List queries are paginated or limited, and are index-backed: the product
  targets 50k snippets.

### Frontend

- TypeScript strict. No `any`, no `@ts-ignore`, no unnecessary assertions.
- Components never call the Tauri `invoke` bridge directly. All data goes
  through the typed IPC layer in `packages/shared`; ESLint enforces this.
- Business rules — sensitivity classification, trigger validation, ranking —
  live in Rust. Never reimplement one in the frontend; two copies drift.
- The frontend is not a data source. Local storage holds UI preferences only,
  never snippet content.
- Every colour, size, radius, shadow, and duration comes from the design tokens.
  Hardcoded visual values are rejected. Pages assemble; reusable pieces go into
  `packages/ui` so desktop and mobile do not each grow their own version.
- All functionality is keyboard reachable, and each screen handles its loading,
  empty, error, and offline states.

### Security red lines

Non-negotiable, in any patch:

- Content of a snippet marked sensitive must never reach the expansion engine's
  config files, the plain full-text index, logs, crash reports, cloud AI
  requests, error messages, or test snapshots.
- No real API keys, tokens, passwords, credentials, or personal data in code,
  tests, fixtures, or commits. Use obviously fake values (`AKIA_FAKE_...`).
  Secrets come from environment variables or platform secure storage.
- Master passwords, derived keys, and master keys are never persisted outside
  platform secure storage, never uploaded, and never printed.
- Never disable authentication, authorization, input validation, permission
  checks, or certificate verification — including temporarily, for debugging.
  "I meant to put it back" is how it ships.
- Cryptography uses the established primitives and maintained libraries already
  in the tree. No home-grown constructions, no unauthenticated modes, no nonce
  reuse. Randomness comes from a CSPRNG.
- The sync server contains no decryption logic and no decryption keys.

If you notice a security problem outside the scope of your change, report it as
described under Security below rather than fixing it in passing.

### Testing

- A bug fix starts with a failing regression test. Write the test, watch it
  fail, then fix it. The test stays forever.
- Test behavior, not implementation. Assert concrete values —
  `assert!(result.is_ok())` proves almost nothing. Name a test after the
  behavior it pins down.
- Tests build their own data and share no mutable global state; they must pass
  in any order and in parallel.
- Never delete, skip, or weaken a test to make a check pass.

## Commits and pull requests

Commit subject format:

```text
<type>: <short description>
```

| Allowed types |                                       |
| ------------- | ------------------------------------- |
| `feat`        | New user-visible capability           |
| `fix`         | Bug fix                               |
| `refactor`    | Behavior-preserving restructuring     |
| `style`       | Formatting only                       |
| `test`        | Tests only                            |
| `docs`        | Documentation only                    |
| `build`       | Build system, packaging, dependencies |
| `chore`       | Maintenance that fits nothing above   |

- English, imperative mood ("add", not "added" or "adds").
- 72 characters or fewer for the subject line.
- One purpose per commit. If the subject needs an "and", split it.
- Rejected on sight: `wip`, `update code`, `fix bug`, `misc changes`.
- **No AI or tool co-author trailers of any kind.** Commit under your own Git
  identity.

### Developer Certificate of Origin

Contributions are accepted under the
[Developer Certificate of Origin](https://developercertificate.org/). Sign off
every commit, which appends a `Signed-off-by` line with your real name and
email:

```sh
git commit -s -m "fix: restore clipboard after a failed injection"
```

To sign off a series you already wrote:

```sh
git rebase --signoff main
```

### Before you push

```sh
git status
git diff
git diff --staged
```

Confirm the diff contains only your change: no debug logging, no temporary
files, no unrelated lockfile churn, no credentials.

### A good pull request description

1. What changes, in one or two sentences.
2. Why — the bug, the issue number, or the use case.
3. How you verified it: which checks you ran and their result, plus any manual
   verification (which platform, which OS version).
4. Anything a reviewer should look at closely: trade-offs, known gaps, follow-up
   work.
5. For UI changes, before and after screenshots in both light and dark themes.
6. For anything touching cryptography, sensitive data paths, or the sync
   protocol, a short "security impact" section stating what the change can and
   cannot expose.

Keep pull requests focused. Unrelated fixes discovered along the way go into
their own pull request or an issue — do not fold them in.

## Licensing of contributions

| Path                                                            | Licence                                   |
| --------------------------------------------------------------- | ----------------------------------------- |
| Repository root and everything under it, except the sync server | MPL-2.0 — see [`LICENSE`](LICENSE)        |
| `apps/sync-server`                                              | AGPL-3.0 — see `apps/sync-server/LICENSE` |

MPL-2.0 is file-level copyleft: modifications to existing files stay open, while
the licence remains compatible with the iOS App Store and Google Play. AGPL-3.0
on the sync server means anyone offering it as a hosted service must publish
their modifications.

By opening a pull request you agree that your contribution ships under the
licence covering the path it touches, and you certify its origin under the DCO
as described above.

### Every new source file needs a licence header

Because MPL-2.0 is file-level copyleft, the root `LICENSE` does not substitute
for a per-file notice. Copy the notice matching the path you are adding to, at
the very top of the file — after a `#!` line if there is one, and before any
module documentation:

```rs
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0
```

Files under `apps/sync-server` carry the AGPL-3.0 notice instead; copy it from
any neighbouring `.go` file. Never mix the two — the notice must match the
licence covering the path.

Adjust the comment syntax to the language (`#` for shell, `/* … */` for CSS) and
keep the wording verbatim. Two kinds of file are deliberately left without a
header: generated project scaffolding under `src-tauri/gen/`, and the SQL
migrations, which are frozen once merged and are compiled into an already
covered Rust source file. Both are covered by the licence of the directory they
live in.

The expansion engine Typvia integrates with is GPL-3.0 and is kept strictly at
arm's length: it runs as a separate, unmodified program, reached only through
its command line and its configuration files. Never link against it, copy its
source into this repository, or reimplement its code inside Typvia — any of
those would pull the GPL across the boundary. Details and attribution are in
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

## Security and conduct

Do not report security vulnerabilities in public issues or pull requests.
Follow the private process in [`SECURITY.md`](SECURITY.md).

Before changing anything under `crates/crypto`, `crates/sync` or
`apps/sync-server`, read [`SECURITY_MODEL.md`](SECURITY_MODEL.md). It states the
properties those paths are required to hold, and the tests that enforce them; a
change that alters one of those properties needs the document updated in the
same pull request, not afterwards.

Participation in this project is governed by the
[Code of Conduct](CODE_OF_CONDUCT.md).
