# Security Policy

Typvia stores credentials, tokens and other short secrets. Its security claims are narrow and stated
in full below, including what it does not defend against.

## Supported versions

Typvia is pre-1.0 and has **no released version**. There is nothing to patch downstream and no
version-support matrix to publish.

Fixes land on `main`. If you are running Typvia, you are running a build from source, and the only
supported answer to a security issue is to update to the current `main`.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting: the repository's **Security** tab, then **Report a
vulnerability**. This is the primary and only channel.

**Do not open a public issue, discussion or pull request for a security problem.** Do not post a
proof of concept publicly before an advisory is out.

Please include:

- **Affected component** — desktop app, mobile app, keyboard or IME, browser extension or its native
  messaging host, sync server, or a specific crate or package.
- **Version** — the commit SHA you tested, plus OS and version, and for mobile the device or
  simulator.
- **Reproduction** — exact steps, or a minimal proof of concept. Say whether it reproduces from a
  clean database.
- **Impact** — what an attacker gains, and which of the assumptions in the threat model below your
  attack needs.
- Any non-default configuration involved (sync enabled, self-hosted server, AI provider configured,
  biometric unlock enabled).

Reports in English or Chinese are both fine.

## What to expect

This is a small project. The commitments are correspondingly modest and meant to be kept.

- **Acknowledgement** within 3 business days.
- **Initial assessment** within 7 days: whether the report is accepted, disputed, or out of the
  threat model, with reasons.
- **Progress updates** at least every 14 days while a report is open.
- **Fix** on `main`, with a regression test. Anything touching cryptography, sensitive-data paths or
  the sync protocol also gets its red-line tests extended so the bug cannot come back silently.
- **Disclosure** is coordinated: a GitHub Security Advisory published once the fix is on `main`. We
  ask you to hold the details until then, or 90 days from the report, whichever comes first. If a
  fix is going to take longer than 90 days, we will say so and agree a date with you rather than let
  it drift.
- **Credit** in the advisory under the name or handle you choose, unless you decline.

There is no bug bounty and no monetary reward.

## Threat model

### What Typvia defends against

- **A compromised or hostile sync server.** The server is dumb storage: it holds ciphertext and
  metadata and no keys, and its code contains no decryption path. An attacker with full read/write
  control of the server cannot read snippet contents, cannot forge or tamper with changes (every
  record carries an Ed25519 device signature, verified by clients), cannot transplant ciphertext
  between records (the AEAD's associated data binds entity type, id and version), cannot replay old
  changes (per-entity version monotonicity), cannot inject a device (device certificates must chain
  to the account's trust root), and cannot downgrade the protocol version.
- **Network attackers.** Production deployments require TLS and clients never disable certificate
  verification. Payload confidentiality and authenticity do not depend on the transport: content is
  encrypted and records are signed before they leave the device.
- **Another local user account, or another application on the same machine.** Key material lives in
  platform secure storage, not in the database or in config files. Sensitive snippet bodies exist on
  disk only as ciphertext. The keyboard, IME and browser-extension processes read a generated
  snapshot and never connect to the main database.
- **Loss or theft of a device at rest.** Sensitive bodies are ciphertext, the keys that open them sit
  behind the platform's secure storage and its biometric or user-presence gate, and a lost device can
  be revoked and the domain keys rotated from a remaining device.
- **Accidental leakage paths**, which are treated as first-class threats: expansion-engine config
  files, the full-text index, logs and crash reports, the system clipboard, cloud AI requests, and
  exported backups.

### What Typvia does not defend against

Stated plainly, because pretending otherwise would be worse than the gap itself:

- An attacker with **root, Administrator, jailbreak or kernel-level control** of the device.
- A **compromised or modified operating system**.
- A **keylogger, malicious IME, or any malware already running** with the ability to read process
  memory.
- A **physical attacker with the device unlocked** — and, likewise, an unlocked vault session.
- **Anything that has already defeated the platform's own secure storage** (Keychain, Keystore,
  DPAPI). Typvia's key protection is exactly as strong as the platform's, and no stronger.
- Leakage of data the user **deliberately exported in plaintext**.
- Availability. A hostile server can deny service, withhold records, and observe metadata.

We do not add pseudo-defences against these. A report whose premise is one of the above will be
closed as outside the threat model — with one exception we do want to hear about: if Typvia itself
_weakens_ a protection the platform would otherwise give you, that is a real bug, please report it.

## Security properties you can rely on

The design is documented in full in **[SECURITY_MODEL.md](SECURITY_MODEL.md)** — key hierarchy,
ciphertext format and the associated data that binds it, what the sync server receives byte for
byte, device certificates and the pairing short-code check, recovery and root rotation, and the
tests that guard each claim. In summary:

- **Key derivation and encryption.** Argon2id for the master password; XChaCha20-Poly1305 for
  content and key wrapping, with a fresh CSPRNG nonce on every operation and associated data binding
  each ciphertext to its purpose and record identity.
- **Key hierarchy.** Master password → a key-encryption key that exists only for the instant of
  unlock → the master key → separate domain keys for ordinary sync and for the vault, so one domain
  rotates without touching the other. The master password is never stored, uploaded or logged.
  Templates that reference a secret carry the reference, never the value.
- **Where keys live.** Platform secure storage only, with biometrics acting as a gate on retrieving
  a key rather than as a source of key material. No key material is ever uploaded.
- **Lock state.** Locked or unlocked, with no partial unlock; locking zeroizes every key and
  decrypted buffer.
- **The sync server.** Ciphertext and metadata, nothing else — no decryption logic and no private
  key material, enforced by a static source check plus tests that scan server logs and the server
  database for plaintext.
- **Where sensitive snippets never go.** The generated expansion-engine configuration, the full-text
  body index, logs and crash reports, cloud AI requests, error messages, and the system clipboard
  (with one documented exception, below). The AI egress log cannot be disabled or compiled out and
  contains only metadata.
- **Extension processes.** The mobile keyboard, the Android IME and the browser-extension host read
  a generated snapshot, never open the main database, and hold no keys. The browser integration uses
  native messaging to a host binary — deliberately not a loopback HTTP port, which any web page can
  reach — and does no in-page trigger expansion.

## Known limitations that are not vulnerabilities

These are deliberate, documented trade-offs. Reporting them is welcome as discussion, but they will
not be treated as vulnerabilities.

- **The local database is not encrypted as a whole.** SQLCipher is intentionally not used: sensitive
  bodies are already encrypted at the application layer, which whole-file encryption cannot replace.
  Ordinary, non-sensitive snippets are stored in plaintext locally and rely on OS full-disk
  encryption and file permissions. To limit residue, the database runs with `secure_delete` enabled
  and the search index is compacted in the same flow when a snippet is converted from ordinary to
  sensitive.
- **A sensitive snippet's title, tags and description are searchable.** Only the body is excluded
  from the full-text index — otherwise the vault would be unusable. Treat titles as non-secret.
- **The clipboard fallback on iOS.** Third-party keyboards cannot type into secure text fields, so
  the documented path is: unlock in the main app, copy once, automatic clearing countdown. A host
  that cannot guarantee automatic clearing refuses the delivery rather than copying in the clear.
  Injection elsewhere restores the previous clipboard contents.
- **The sync server sees metadata.** Record count, sizes, timestamps and device count are visible to
  whoever runs the server, and a hostile server can withhold records; detection rests on cursor
  monotonicity and eventual convergence across devices. This is a property of the topology, not a
  bug.
- **The master password cannot be recovered.** The only in-product path after losing it is a reset
  that destroys the vault contents and the wrapped keys. That is the design.
- **Windows secure storage is not yet verified on real hardware.** DPAPI and Windows Hello
  integration is written against the same interface as the other platforms but has not been tested on
  a Windows machine. This is a known gap and a release prerequisite, not a permanent limitation.
- **Injection needs an OS permission.** On macOS, keystroke injection requires Accessibility
  authorization; if it is denied, Typvia degrades to copy-only rather than working around the
  platform.
- **Unsigned development builds can trigger keychain prompts on macOS.** Keychain ACLs bind to the
  build identity, so a rebuilt development binary reading an entry it created earlier prompts for
  authorization. Signed builds are unaffected.

## Scope

**In scope:**

- The sync server, and the self-hosting deployment templates.
- The desktop app (Windows, macOS) and the mobile apps (iOS, Android).
- The browser extension and its native messaging host.
- The iOS keyboard extension and the Android IME, plus the share extensions.
- The shared Rust crates and TypeScript packages, and the snapshot pipeline that feeds the extension
  processes.

**Out of scope:**

- **Third-party dependencies.** Report those to their own maintainers; see
  [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). If Typvia's _use_ of a dependency is what makes
  the issue exploitable, that is in scope and we want the report.
- **The text-expansion engine.** Typvia launches a separately licensed upstream expansion engine as
  an independent process; vulnerabilities in that engine belong to its own project and should go
  there. How Typvia _invokes_ it is ours: the configuration Typvia generates for it, the private
  directories and process lifecycle it runs under, and the OS permissions attributed to it are all in
  scope.
- Social engineering, physical attacks, and attacks requiring an already-compromised device.
- Denial of service against someone's self-hosted server. Rate limits exist; availability is not a
  protocol guarantee.
- Automated scanner output with no demonstrated impact, and missing hardening headers or best
  practices with no exploit path.

## Related

- [SECURITY_MODEL.md](SECURITY_MODEL.md) — the cryptographic design in full: keys, formats,
  protocol, and how to verify each claim.
- [README.md](README.md) — what Typvia is and how the repository is laid out.
- [CONTRIBUTING.md](CONTRIBUTING.md) — the security rules a change is reviewed against.
- [LICENSE](LICENSE) — MPL-2.0 for the clients and shared code; the sync server carries AGPL-3.0
  separately.
- [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) — third-party components shipped in a release.
