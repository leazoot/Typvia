<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/assets/logo-on-dark.png">
  <img src=".github/assets/logo-on-light.png" alt="Typvia" width="380">
</picture>

**Save once. Type anywhere.**

An open-source, local-first, end-to-end encrypted snippet vault and text injector
for Windows, macOS, iOS and Android.

[![CI](https://github.com/leazoot/Typvia/actions/workflows/ci.yml/badge.svg)](https://github.com/leazoot/Typvia/actions/workflows/ci.yml)
[![Clients: MPL-2.0](https://img.shields.io/badge/clients-MPL--2.0-blue)](LICENSE)
[![Sync server: AGPL-3.0](https://img.shields.io/badge/sync%20server-AGPL--3.0-blue)](apps/sync-server/LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20iOS%20%7C%20Android-lightgrey)](#platforms)

English · [简体中文](README.zh-CN.md)

</div>

> **Status: early development — no release yet.**

## Why

The text you retype every day — commands, SQL, code blocks, prompts, canned
replies, addresses, tokens — is scattered across notes, chat history, the
clipboard and a password manager. Every existing home for it gives something up:
a text expander has no mobile side and no place for secrets, a clipboard manager
forgets, a notes app is too many taps away, a password manager is not built for
code or templates.

Typvia keeps all of it in one local, encrypted store and puts it one keystroke
away — on the desktop, and inside the keyboard on your phone.

## What it does

**Capture** — snippets in folders and tags, typed as text, code, command, prompt,
template, secret, AI action or link. Import from existing expander match files
and from CSV.

**Recall** — a global panel over any application, full-text search built on
SQLite FTS5 with CJK segmentation, optional local semantic search, ranked by how
you actually use each snippet. Designed for 50,000 snippets: 10k searched in
under 50 ms, 50k under 150 ms, held by in-tree benchmarks.

**Type** — abbreviation triggers through a managed expansion engine, or direct
insertion from the panel. On mobile the entry point is a real keyboard: a custom
keyboard on iOS, an IME on Android, plus share targets and a browser extension.

**Fill** — templates with typed variables, completed at the moment of insertion
instead of being pasted and edited.

**Protect** — a vault for sensitive snippets, encrypted at rest and unlocked by
biometrics. Vault content never reaches the expansion engine's config files, the
plain full-text index, logs, crash reports or AI requests.

**Sync** — optional end-to-end encrypted sync across devices, with QR pairing and
a spoken-code verification step. Run the included server yourself, or use WebDAV.
The server only ever holds ciphertext and contains no decryption path.

**Assist** — optional AI for titling, tagging and extracting template variables.
Bring your own key, or point it at a local model. Off by default, and gated so
that vault content and anything that looks like a credential cannot leave the
device.

## Platforms

| Platform | Minimum    | Entry points                                       |
| -------- | ---------- | -------------------------------------------------- |
| Windows  | 10         | App, global panel, tray, abbreviation triggers     |
| macOS    | 12         | App, global panel, menu bar, abbreviation triggers |
| iOS      | 16         | App, custom keyboard, share extension, widget      |
| Android  | 9 (API 28) | App, IME, share target                             |

Development happens on macOS. The Windows paths are written against the same
abstractions and compile in CI, but have not been exercised on a real Windows
machine yet — treat them as unverified. Linux is not a supported target yet,
though CI builds `.deb` and AppImage artifacts from every push to `main`.

## Security

Typvia is a place people are meant to keep credentials, so the security model is
part of the product rather than a footnote:

- Content is encrypted on the device. Key material lives only in the platform's
  secure storage — Keychain, Keystore, DPAPI — and is never uploaded or logged.
- The sync server is treated as hostile. It sees ciphertext and routing metadata,
  nothing else, and has no code path that could decrypt a record.
- Sensitive snippets are structurally excluded from every place plaintext tends
  to leak: expander config files, the plain search index, logs, crash reports,
  error messages, cloud AI requests and test snapshots.
- Everything works offline. Sync, accounts and AI are each optional and each can
  be switched off entirely.

[`SECURITY_MODEL.md`](SECURITY_MODEL.md) is the full design — key hierarchy,
ciphertext format, what the server receives, pairing and recovery — down to the
tests that guard each claim. [`SECURITY.md`](SECURITY.md) covers the threat
model, what is explicitly _not_ defended against, and how to report a
vulnerability privately.

## Self-hosting the sync server

The server is a single static Go binary with a SQLite file. On the deployment
host you need neither the source nor a toolchain:

```sh
mkdir -p ~/typvia-sync && cd ~/typvia-sync
curl -fsSL -o docker-compose.yml \
  https://raw.githubusercontent.com/leazoot/Typvia/main/deploy/docker/compose.image.yml
echo 'TYPVIA_IMAGE=ghcr.io/leazoot/typvia-sync:latest' > .env
docker compose up -d
curl http://127.0.0.1:8787/healthz
```

Multi-arch images are published to GHCR on every push. See
[`deploy/docker/`](deploy/docker/README.md) for building from source, upgrades
and backups, and [`deploy/cloudflare/`](deploy/cloudflare/guide.md) for a
tunnelled deployment that exposes no port at all.

## Build from source

Requires Node 22.22+, pnpm 10.30+, a stable Rust toolchain with the 2024
edition, and Go 1.25+ for the sync server. Building the mobile apps and their
native layers additionally needs Xcode or the Android SDK and NDK.

```sh
pnpm install
pnpm dev            # run the desktop app

pnpm lint           # clippy + eslint
pnpm typecheck
pnpm test           # cargo test + vitest
pnpm build
```

[`CONTRIBUTING.md`](CONTRIBUTING.md) covers the full toolchain, the layout, and
the code rules a review will hold you to.

## Repository layout

```text
apps/desktop            Tauri 2 desktop app (Windows, macOS)
apps/mobile             Tauri 2 mobile app (iOS, Android)
apps/sync-server        Go sync server — ciphertext only, self-hostable
apps/browser-extension  Browser extension
apps/browser-host       Native-messaging host for the extension
crates/core             Data model, SQLite storage, use-case orchestration
crates/crypto           Key derivation, AEAD, secure-store trait
crates/search           FTS5 index and weighted ranking
crates/semantic         Local semantic search kernel
crates/sync             End-to-end encrypted sync client
crates/template         Template parsing and rendering
crates/ai               AI provider adapter and egress gate
crates/espanso-adapter  Expansion-engine config, import and lifecycle
crates/host-service     IPC orchestration shared by the app shells
crates/mobile-ffi       UniFFI surface for the native mobile layers
packages/ui             Shared React component library and design tokens
packages/shared         Shared TypeScript types and typed IPC layer
native/                 Swift keyboard and share extensions, Kotlin IME
deploy/                 Self-hosting templates for the sync server
```

## Contributing

Issues and pull requests are welcome — please open an issue before starting
anything large. See [`CONTRIBUTING.md`](CONTRIBUTING.md) and
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). Contributions are accepted under the
[Developer Certificate of Origin](https://developercertificate.org/); sign your
commits with `git commit -s`.

## License

- **MPL-2.0** ([`LICENSE`](LICENSE)) — the clients and all shared code.
- **AGPL-3.0** ([`apps/sync-server/LICENSE`](apps/sync-server/LICENSE)) — the
  sync server.

The desktop release bundles [Espanso](https://espanso.org) (GPL-3.0) as its
expansion engine — an unmodified official binary, shipped as a separate program
and driven only through its command line and config files. Typvia links against
no part of it and copies none of its source. Details and the corresponding-source
pointer are in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
