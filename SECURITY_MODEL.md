# Security Model

Typvia is a place to keep credentials, tokens and other short secrets, so "it is
encrypted" is not a claim you should have to take on faith. This document
describes the design in enough detail to be argued with: what each key is, where
it lives, what exactly the sync server receives, and which properties depend on
which mechanism.

[SECURITY.md](SECURITY.md) covers the other half — how to report a vulnerability
and what is in scope. This document covers what the product is trying to do and
how.

> **Status.** Typvia is pre-1.0 and has never been independently audited. The
> design below is implemented and covered by tests, but "implemented and tested"
> is a weaker statement than "reviewed by someone whose job is breaking things."
> Known gaps are listed at the end rather than omitted.

## What is being protected, and from whom

**Primary assets.** Sensitive snippet bodies — passwords, API keys, tokens,
cookies, private key fragments — and the key material that opens them: the
master password, the derived key-encryption key, the master key, the domain keys
and the per-device private keys.

**Secondary asset.** Ordinary snippet bodies. These are not treated as
confidential by the product definition, and they are stored locally in
plaintext. See [Local storage](#local-storage-what-is-and-is-not-encrypted) for
why, and what that means for you.

| Adversary                                          | What stops them                                                                           |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| A hostile or breached sync server                  | Content is encrypted and records are signed before upload; the server holds no keys       |
| A network attacker                                 | TLS required, plus the above — transport is not what confidentiality rests on             |
| Another local account or app on the same machine   | Key material lives in platform secure storage; sensitive bodies are ciphertext            |
| A lost or stolen device, at rest                   | Ciphertext plus the platform's biometric/user-presence gate; device revocation            |
| A malicious device admitted to your account        | Pairing requires an out-of-band short-code comparison by a human                          |
| Accidental leakage (config files, index, logs, AI) | Structural exclusion — see [Containment](#containment-where-sensitive-content-never-goes) |

Explicitly **not** defended against: root/Administrator/jailbreak/kernel-level
control, a modified operating system, malware already able to read process
memory, an attacker with the device unlocked in front of them, and anything that
has already defeated the platform's own secure storage. No pseudo-defences are
added against these; a defence that cannot work is worse than an honest gap.

## Primitives

| Purpose                             | Algorithm                               |
| ----------------------------------- | --------------------------------------- |
| Password stretching                 | Argon2id                                |
| Content encryption and key wrapping | XChaCha20-Poly1305 (AEAD, 16-byte tag)  |
| Pairing key exchange                | X25519                                  |
| Device identity and record signing  | Ed25519 (strict verification)           |
| Labelled subkey derivation          | HKDF-SHA-256                            |
| Randomness                          | OS CSPRNG for every key, nonce and salt |

No custom cryptography, no unauthenticated modes, no nonce reuse, and no key or
nonce ever derived from a timestamp or a non-cryptographic RNG.

**Argon2id parameters (v1):** 64 MiB memory, 3 iterations, parallelism 1,
16-byte random salt, 32-byte output. The parameters and the salt are stored,
versioned, next to the wrapped key — so retuning later affects new derivations
only, and an old key header keeps verifying under the parameters it recorded.
Derivation happens only in the main application process; keyboard and IME
extensions never run it (a 64 MiB derivation does not fit in an iOS keyboard's
memory budget, and they have no business holding keys anyway).

## Key hierarchy

```text
master password                       (memorised; never stored, uploaded or logged)
   │  Argon2id (stored salt + versioned parameters)
   ▼
KEK                                   (exists only for the instant of unlock)
   │  unwrap, AAD "typvia.mk.v1"
   ▼
MK — master key, 32 bytes CSPRNG      (stored wrapped; never in plaintext on disk)
   │  unwrap, AAD "typvia.domain.<name>.v1"
   ├──────────────────────────┐
   ▼                          ▼
K_sync                     K_vault
ordinary sync domain       vault domain
```

Each key, and the reason it exists:

- **KEK** — the Argon2id output. It wraps the master key and nothing else, and
  is zeroized immediately after use. It never touches business data.
- **MK** — 32 CSPRNG bytes generated at vault creation, stored wrapped under the
  KEK. With biometric unlock enabled, a second copy sits in platform secure
  storage behind the platform's gate; both unlock paths yield the same MK.
- **K_sync / K_vault** — independently generated (not derived from MK), each
  wrapped under MK. Independence is the point: one domain can be rotated without
  touching the other, and a device can hold K_sync without ever holding K_vault.
- **Device keys** — per device, an Ed25519 pair for identity and signatures and
  an X25519 pair for pairing. Private keys live only in platform secure storage,
  never in the database.
- **Recovery code** — 128 bits of CSPRNG, stretched by Argon2id under its own
  salt into a second key that wraps a copy of the key bundle. Losing the master
  password with the recovery code in hand costs nothing; losing both is
  unrecoverable by design.

**Rotation.** Changing the master password re-derives the KEK and re-wraps MK —
constant work, no record is touched. Rotating a domain key mints a new
generation (`key_id + 1`); every ciphertext carries its `key_id`, so old and new
coexist and old data stays readable.

**One deliberate exception worth knowing about.** Ordinary snippets must sync
while the vault is locked — that is a product promise, and a locked vault means
no MK to unwrap K_sync with. So a runtime copy of **K_sync** is kept in platform
secure storage as a non-gated entry, alongside the device private keys. K_vault
and MK are never treated this way. The trade-off is stated rather than hidden:
K_sync covers the ordinary-snippet domain only, and an attacker who can read
non-gated platform secure storage has already defeated the platform.

## Ciphertext format

```text
envelope = version(1 byte, 0x01) || key_id(4 bytes LE) || nonce(24 bytes) || AEAD(ciphertext || tag)
```

**Every encryption draws a fresh 24-byte nonce from the OS CSPRNG.** Re-encrypting
the same record — an edit, a key rotation — draws a new one. No nonce is ever
reused, and none is derived deterministically from the input. XChaCha20's
192-bit nonce space makes random selection safe without counter state.

**Associated data binds each ciphertext to its purpose and its identity**, so a
ciphertext cannot be lifted from one record and dropped into another:

| Use             | Associated data                                                                                      |
| --------------- | ---------------------------------------------------------------------------------------------------- |
| Snippet at rest | `"typvia.snippet.v1"` \|\| snippet id                                                                |
| Master key wrap | `"typvia.mk.v1"`                                                                                     |
| Domain key wrap | `"typvia.domain.<name>.v1"`                                                                          |
| Sync record     | `"typvia.sync.v1"` \|\| entity type \|\| `0x00` \|\| entity id \|\| `0x00` \|\| version (8 bytes LE) |

The sync AAD includes the version number, which is what stops a server from
handing back an old ciphertext relabelled as a new version.

## Where keys live

| Platform | Backend                        | Gate                                                  |
| -------- | ------------------------------ | ----------------------------------------------------- |
| macOS    | Keychain                       | Touch ID / user presence                              |
| iOS      | Keychain (main app only)       | Face ID / LocalAuthentication                         |
| Android  | Keystore (non-exportable keys) | BiometricPrompt, STRONG class                         |
| Windows  | Credential Manager / DPAPI     | Windows Hello — **not yet verified on real hardware** |

Two things this table does not say, and should:

**Biometrics is a gate on retrieving a key, not a source of key material.**
Turning it off deletes that copy of the MK and leaves the master-password path
untouched. On Android the Keystore stores a _key_, not the data: the MK copy is
wrapped by a non-exportable Keystore RSA key whose private half requires a fresh
STRONG biometric per use, and the decrypting cipher is bound into the system
biometric prompt. Re-enrolling a fingerprint invalidates that key permanently —
by platform design — and the user re-enables through the master password.

**The master password and the KEK never enter secure storage**, and no key
material is ever uploaded anywhere.

## Lock state

Two states, `Locked` and `Unlocked`. There is no partial unlock. Locking
zeroizes the master key, the domain keys and every decrypted buffer. It is
triggered by explicit lock, idle timeout, application exit, and on desktop by
the OS session lock. A failed unlock attempt changes nothing.

Decrypted plaintext is held under `zeroize`, kept out of long-lived caches, and
redacted in `Debug`/`Display` implementations so it cannot leak through a log
line or a panic message.

**Forgetting the master password is not recoverable** without the recovery code.
The only in-product path is a reset that destroys the vault: the biometric copy
is deleted first (failing there aborts with zero changes), then every sensitive
snippet and every wrapped key row is removed in a single transaction, followed
by a vacuum and a write-ahead-log truncation so that ciphertext and wrapped keys
do not survive in free pages. That last step is verified at the byte level by a
test, not assumed.

## What the sync server actually sees

The server is dumb storage. It holds ciphertext and routing metadata, and it has
no key material and no decryption path. This is not merely a policy: a test
statically parses every Go source file in the server module and fails the build
if an import providing decryption or private-key cryptography appears. Ed25519
_verification_ is allowed and allow-listed — it is a public-key operation used
to reject unauthorized writes — along with SHA-256, the CSPRNG and constant-time
comparison. Everything else under `crypto/`, all of `golang.org/x/crypto`, and
known third-party AEAD packages are denied. Widening that list is a protocol
decision, not a code change.

A record on the wire:

```text
{ id, entity_type, entity_id, version, ciphertext, deleted_at,
  updated_at, device_id, key_id, signature }
```

and the bytes each device signs:

```text
"typvia.syncrec.v1" || device_id || 0x00 || entity_type || 0x00 || entity_id || 0x00
  || version(8 LE) || deleted_at(8 LE; 0xFFFF_FFFF_FFFF_FFFF when not a tombstone)
  || updated_at(8 LE) || SHA-256(envelope bytes)
```

The server verifies signatures too, but only as an admission filter. **The
client's verification is the only authoritative one**, because the server is not
trusted.

So, concretely, a fully compromised server cannot:

- **Read content** — it has no keys, and none of its code can decrypt.
- **Forge or tamper with a change** — every record carries an Ed25519 signature
  from a device whose certificate chains to your account's trust root.
- **Move a ciphertext between records or versions** — the AAD binds entity type,
  entity id and version.
- **Replay an old change** — each client tracks the applied version per entity
  and silently drops anything at or below it; pull cursors must strictly
  increase or the round is aborted.
- **Inject a device** — an unchained device's records are refused.
- **Downgrade the protocol** — version ranges are checked, not negotiated
  downward.

What it **can** do, stated plainly: deny service, withhold records, and observe
metadata — record count, sizes, timestamps, device count. Withholding is
detectable only through cursor monotonicity and eventual divergence between
devices. This is a property of the star topology, not a bug, and the fix would
be a different topology rather than a patch.

**Sensitive snippets cross the sync boundary inside two envelopes.** The body
field of a sensitive snippet stays exactly as it sits in the database — a
K_vault ciphertext, never unwrapped — and the whole entity document is then
encrypted under K_sync. A device deliberately not granted vault access can
therefore sync your library and show a locked row, and still never be able to
decrypt the body. The two key domains are not an organisational convention; they
are what makes that possible.

## Device identity and pairing

The first device on an account self-signs a **root statement**; the SHA-256
fingerprint of its Ed25519 public key is the account's identity anchor, pinned
locally on every device thereafter. A mismatch is a hard failure, checked before
anything else.

Admitting a new device means an already-trusted device signs a **certificate**
over `{device_id, ed25519_pub, x25519_pub, name, platform, created_at}` plus the
issue time and its own device id. The issue time is inside the signature so that
revocation has a cutoff: a certificate issued at or after its issuer's
revocation time is invalid, boundary included. The issuer id is inside the
signature so a certificate cannot be re-attributed to a different signer. Root
statements and certificates use different domain prefixes, so one can never be
replayed as the other.

**Fingerprints** are displayed as the first 20 base32 characters of the SHA-256
digest, in four hyphenated groups of five (`TIW3F-YR7CU-CM2BL-GAZKT`);
comparisons internally use the full 32-byte digest.

**The short authentication string is what closes the man-in-the-middle gap.**
Both devices independently compute

```text
SAS = base32( HKDF-SHA-256( salt = session_id,
                            ikm  = N.ed25519_pub || N.x25519_pub
                                   || T.ed25519_pub || root_fingerprint,
                            info = "typvia.sas.v1" ) )[0..20]
```

and the user compares them by eye. A server that substituted its own key for the
new device's changes the input, so the two codes differ. The interface does not
let you skip this step. On the new device the code is shown _after_ the key
bundle arrives but _before_ anything is installed, so aborting on a mismatch
leaves no residue.

The key bundle itself is sealed to the new device with an ephemeral X25519 key,
HKDF-SHA-256 keyed by the session id, and XChaCha20-Poly1305 whose AAD binds the
session id and the recipient's public key; the sealed blob is then signed by the
admitting device under its own domain tag. Only ciphertext passes through the
server. Whether the bundle includes MK and K_vault is a choice made at admission
time — declining it produces a device that syncs but can never open the vault,
and the grant can be issued later through a sealed key-update message without
re-pairing.

## Recovery and root rotation

A recovery code is `T1` + 26 base32 characters (128 bits of entropy) + a
4-character checksum — 32 characters, shown as eight groups of four, displayed
once, never stored or uploaded in plaintext. The checksum catches typos; it adds
no entropy and is not treated as if it did.

The server stores a **recovery blob**: the key bundle encrypted under a key
derived from the recovery code with its own salt, with the account id bound into
the AAD. The blob is worthless without the code. Its plaintext header carries
the KDF parameters, and the client bounds them on read — at most 256 MiB, 10
iterations, parallelism 4 — so a hostile server cannot hand back a blob that
demands an absurd derivation.

Recovering onto a fresh device means **rotating the trust root**: the new device
self-signs a new root statement and proves possession of the master key with an
Ed25519 key derived by HKDF-SHA-256 from MK under the account id, signing a
one-time 32-byte server challenge. On success the server replaces the root,
marks every old device revoked and invalidates all sessions, in one transaction.
K_sync rotation is then forced.

Records signed under the _old_ root still need to verify. The old root statement
travels inside the recovery blob — vouched for by the recovery code and the
master-key proof, not supplied by the server — and is persisted as a catch-up
marker. While the marker is set, each sync round verifies against the new root
first and falls back to the old one, until a round drains the server. An
interruption at any point resumes on the next round instead of leaving a hole.

## Containment: where sensitive content never goes

These are structural exclusions with tests behind them, not review conventions.

- **The expansion engine's configuration.** Generated files never contain a
  sensitive snippet. A generation failure keeps the previous configuration
  rather than writing a partial one.
- **The full-text body index.** Only ordinary snippets are indexed by body. When
  a snippet is converted from ordinary to sensitive, the index is compacted in
  the same flow so the old plaintext does not survive at the byte level, and the
  database runs with `secure_delete` enabled so deleted rows are zero-filled.
- **Logs and crash reports.** No snippet body, password, token, key material or
  clipboard content. Error types do not carry content.
- **Cloud AI requests.** Sensitive snippets and suspected secrets are screened
  at the provider boundary, and a hit is refused _before a connection is
  opened_. The egress log that records this cannot be disabled or compiled out,
  and itself contains only metadata — no prompt text, no responses, no API keys.
  Local semantic search never embeds sensitive snippets at all.
- **The system clipboard**, with one documented exception. Third-party keyboards
  cannot type into secure text fields on iOS, so the supported path is: unlock
  in the main app, copy once, automatic clear after **30 seconds**. A host that
  cannot guarantee the clearing refuses to deliver rather than copying in the
  clear. Elsewhere, injection restores whatever was on the clipboard before.
- **Extension processes.** The mobile keyboard, the Android IME and the browser
  native-messaging host read a generated snapshot and never open the main
  database. They hold no MK and no domain keys, so sensitive entries reach them
  only as opaque ciphertext metadata; decryption happens only in the main app
  process. The keyboard records no keystrokes, keeps no history, and is offline
  by default.

## Local storage: what is and is not encrypted

**The database is not encrypted as a whole, and that is a decision rather than
an omission.** Sensitive bodies are already encrypted at the application layer,
which whole-file encryption cannot replace — it would be a second layer over the
same data with no added guarantee, while breaking the ability to use ordinary
snippets before unlocking. Ordinary snippets are therefore stored in plaintext
locally and rely on OS full-disk encryption and file permissions.

**A sensitive snippet's title, tags and description are searchable.** Excluding
them would make the vault unusable. Treat titles as non-secret.

Exported plaintext is the user's own decision and is outside the model. The
default export is an encrypted backup: the document is sealed in a single
envelope under a key derived from a separate backup passphrase, with sensitive
bodies still wrapped in their K_vault envelopes inside it. The backup passphrase
therefore never opens vault contents — restoring gives you the wrapped keys and
still requires the original master password.

## Versioning

Three axes move independently, so one can evolve without dragging the others:

| Axis             | Where                      | On mismatch                                                                                  |
| ---------------- | -------------------------- | -------------------------------------------------------------------------------------------- |
| Envelope version | first byte of ciphertext   | Unknown version is refused                                                                   |
| Payload version  | inside the entity document | Newer than understood: the record is held pending and retried, never dropped or half-applied |
| Protocol version | `X-Typvia-Protocol` header | No overlap with the server's advertised range: an explicit error, never a silent downgrade   |

The current protocol version is 1; the server advertises its supported range and
answers an unsupported one with HTTP 426 and a stable error code. Server-side
limits are a 256 KiB ciphertext envelope per record, 500 records or 4 MiB per
push batch, and 500 records per pull page — all exceeded with an explicit error
rather than truncation.

## Known gaps

Listed because a security document that only lists strengths is an advert.

- **No independent audit.** Everything here is self-assessed.
- **Windows secure storage is unverified on real hardware.** The DPAPI and
  Windows Hello integration is written against the same interface as the other
  platforms and compiles in CI, but has not run on a Windows machine. Treat the
  Windows row of the platform table as a claim, not a result.
- **Hardware key binding is not required.** Secure Enclave and StrongBox are
  used opportunistically where the platform offers them, but v1 does not require
  them and does not fail if they are absent.
- **The server sees metadata**, and can withhold records. Availability is not a
  protocol guarantee.
- **Cross-device clock skew affects merge outcomes for scalar fields.** Where
  both sides edited the same non-body field, the later `updated_at` wins, with
  device id as a deterministic tie-break. Bodies are never resolved this way —
  a genuine two-sided body conflict keeps both, one as a conflict copy. Skew can
  therefore pick the wrong title, never silently discard content.
- **Usage counters take the maximum across devices rather than the sum.** An
  honest lower bound; summing correctly would need a conflict-free counter type,
  which v1 does not introduce.

[SECURITY.md](SECURITY.md) lists a further set of deliberate trade-offs that are
documented and will not be treated as vulnerabilities.

## Checking these claims yourself

The properties above are guarded by tests that run on every change. If you want
to verify a claim rather than trust it, these are the files to read first:

| Claim                                                                        | Test                                                                                         |
| ---------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Wrong key fails; nonces never repeat; zeroize works; `Debug` redacts         | `crates/crypto/tests/crypto_redlines.rs`                                                     |
| Sensitive bodies are absent from the index at the byte level                 | `crates/search/tests/sensitive_isolation.rs`                                                 |
| The server cannot decrypt — static import scan                               | `apps/sync-server/internal/redline/nodecrypt_test.go`                                        |
| Bad signatures, forged devices, replays and swapped ciphertexts are rejected | `crates/sync/tests/sync_record_redlines.rs`, `crates/sync/tests/device_identity_redlines.rs` |
| Sensitive plaintext never appears in sync bytes                              | `crates/sync/tests/sync_record_redlines.rs`, `crates/sync/tests/webdav_redlines.rs`          |
| Suspected secrets never reach the network; the egress log cannot be disabled | `crates/ai/tests/egress_redlines.rs`, `crates/ai/tests/api_key_redlines.rs`                  |
| Two clients converge through a real server                                   | `crates/sync/tests/e2e_sync.rs`, `crates/sync/tests/webdav_e2e.rs`                           |

Run them with `cargo test --workspace`, and the server's with
`cd apps/sync-server && go test ./...`.

## Related

- [SECURITY.md](SECURITY.md) — reporting a vulnerability, and what is in scope.
- [README.md](README.md) — what Typvia is and how the repository is laid out.
- [CONTRIBUTING.md](CONTRIBUTING.md) — the rules a change is reviewed against.
