# Typvia sync server · Docker

Two files, two ways to run it:

- **`compose.image.yml`** — for the deployment host. Runs a published image; the
  host needs **no source, no Go and no build toolchain**.
- **`docker-compose.yml`** — for a development machine. Builds the image from
  this repository's source, then runs it.

For remote access with no port exposed at all, see
[`../cloudflare/`](../cloudflare/guide.md) (Cloudflare Tunnel).

## Deployment host: pull and run

Images are pushed to GHCR by `.github/workflows/publish-sync-server.yml` as
`ghcr.io/<owner>/typvia-sync`, tagged `latest` (default branch), `sha-xxxxxxx`
(every commit) and `1.2.3` / `1.2` (when a `v1.2.3` tag is pushed). Substitute
your own owner if you are running a fork.

```sh
mkdir -p ~/typvia-sync && cd ~/typvia-sync
curl -fsSL -o docker-compose.yml \
  https://raw.githubusercontent.com/leazoot/Typvia/main/deploy/docker/compose.image.yml
echo 'TYPVIA_IMAGE=ghcr.io/leazoot/typvia-sync:latest' > .env
docker compose up -d
curl http://127.0.0.1:8787/healthz    # {"status":"ok"}
```

In production, replace `latest` with a concrete version (`:1.2.3`) or commit
(`:sha-xxxxxxx`) — rolling back is then one line and `up -d`.

**GHCR packages are private by default.** After the first push, go to GitHub →
Packages → the package → Package settings → Change visibility → Public so the
host can pull anonymously. To keep it private, log in on the host first:

```sh
echo <PAT with read:packages> | docker login ghcr.io -u <owner> --password-stdin
```

Images are multi-arch (linux/amd64 + linux/arm64), so an x86 VPS and an arm NAS
both pull the same tag.

## Development machine: build from source

Run this from the **repository root** — the image needs `apps/sync-server/`, so
the build context is the repository root:

```sh
docker compose -f deploy/docker/docker-compose.yml up -d --build
```

## Connecting a client

Settings → Sync → Set up sync → Self-hosted server, then enter the server
address. There is no account to create on the server: the first device creates
the account and registers itself, and every device after that joins with a
pairing code.

## Just the binary, no container

```sh
cd apps/sync-server && go run ./cmd/typvia-sync-server
```

It listens on `127.0.0.1:8787` and writes `typvia-sync.db` into the working
directory. Two environment variables are the entire configuration —
`TYPVIA_SYNC_LISTEN` and `TYPVIA_SYNC_DB` (the `-listen` / `-db` flags take
precedence). Cross-compiling a static binary to drop on a server is one line,
because the SQLite driver is pure Go and the result has no dependencies:

```sh
GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build ./cmd/typvia-sync-server
```

## Making it reachable from another device

Both compose files bind the port to `127.0.0.1`, deliberately. Clients validate
the server address strictly — **https, or http on loopback** — and never disable
certificate verification. So reaching the server from another device means one
of two things:

- put Caddy or nginx in front to terminate TLS and reverse-proxy to
  `127.0.0.1:8787`, leaving the port mapping on loopback; or
- switch to [`../cloudflare/`](../cloudflare/guide.md), which opens no port at
  all. Replace `build:` with `image: ${TYPVIA_IMAGE}` in that compose file and it
  needs no source either.

Binding straight to `0.0.0.0` only gets you a plaintext listener that clients
refuse to talk to.

## Operating it

- **Backups** — the data is a single SQLite database (WAL mode) in the
  `sync-data` volume. The image is distroless, so there is no shell to exec
  into: stop the service with `docker compose stop` and back the volume up at
  file level, or run `sqlite3 .backup` from the host (find the volume path with
  `docker volume inspect`).
- **Upgrades** — `docker compose pull && docker compose up -d` (for the
  build-from-source file, `up -d --build`). Migrations run at startup inside a
  transaction and roll back as a whole on failure.
- **What the server can see** — ciphertext and metadata only. It holds no keys
  and contains no decryption path.
