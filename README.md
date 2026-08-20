# Typvia

> Save once. Type anywhere. / 保存一次,随处输入。

Typvia is an open-source, local-first, end-to-end encrypted text snippet vault and injector for Windows, macOS, iOS, and Android. It turns the text you type repeatedly — commands, code blocks, prompts, replies, addresses, credentials — into a searchable, fillable, securely retrievable personal input layer.

**Status: early development.** No usable release exists yet.

## Repository structure

```text
apps/desktop        Tauri 2 desktop app (Windows/macOS)
apps/mobile         Tauri 2 mobile app (iOS/Android)
apps/sync-server    Go sync server (ciphertext-only, self-hostable)
crates/core         Data model, storage, use-case orchestration
crates/crypto       Key derivation, encryption, secure storage traits
crates/search       SQLite FTS5 index and weighted search
crates/sync         E2EE sync client
crates/template     Template parsing and rendering
crates/espanso-adapter  Espanso config generation and CLI interaction
packages/ui         Shared React component library
packages/shared     Shared TypeScript types and typed IPC layer
native/             iOS keyboard/share extension, Android IME/share target
deploy/             Self-hosting templates for the sync server
```

## License

Typvia uses a two-tier license layout:

- **MPL-2.0** (repository root `LICENSE`) covers the clients and all shared code: `apps/desktop`, `apps/mobile`, `crates/*`, `packages/*`, `native/*`.
- **AGPL-3.0** (`apps/sync-server/LICENSE`) covers only the sync server.

Rationale: MPL-2.0 keeps file-level copyleft on the clients while remaining compatible with the iOS App Store and Google Play; AGPL-3.0 on the sync server ensures that anyone offering a hosted service based on it must publish their modifications.

Typvia integrates with [Espanso](https://espanso.org) (GPL-3.0) strictly at arm's length: separate processes, configuration files, and CLI calls only. Typvia contains no Espanso code and does not bundle its binary.

Contributions are accepted under the [Developer Certificate of Origin](https://developercertificate.org/) (DCO).
