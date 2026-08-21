# Typvia sync server · Cloudflare Tunnel

A self-hosting path with no public IP and no exposed port: the server listens
only on the compose network, `cloudflared` dials out to Cloudflare's edge, and
TLS terminates at Cloudflare. The server stores ciphertext and metadata only, so
neither Cloudflare nor the host can read your snippets.

## Prerequisites

- A host that can run Docker Compose — NAS, home machine or VPS. No public IP
  required.
- A domain on Cloudflare. The free plan is enough.

## Steps

1. **Create the tunnel** — Cloudflare dashboard → Zero Trust → Networks →
   Tunnels → Create a tunnel (Cloudflared type) → name it, e.g. `typvia-sync` →
   copy the token.
2. **Add a public hostname** — under the tunnel's Public Hostname tab, map for
   example `sync.example.com` to the service `http://typvia-sync:8787`. The
   compose service name is the internal hostname.
3. **Put the files on the host** — copy this directory over, then
   `cp .env.example .env` and set `TUNNEL_TOKEN` to your token. `.env` is never
   committed to any repository.
4. **Start it** — `docker compose up -d --build`.
5. **Verify** — `curl https://sync.example.com/healthz` should succeed. In the
   client, go to Sync settings → Self-hosted server and enter
   `https://sync.example.com`.

## Operating it

- **Backups** — the data is a single SQLite database (WAL mode) in the
  `sync-data` volume. `docker compose exec` will not help: the image is
  distroless and has no shell. Stop the service before backing the volume up at
  file level, or run `sqlite3 .backup` from the host (find the volume path with
  `docker volume inspect`).
- **Upgrades** — pull the new code and `docker compose up -d --build`.
  Migrations run at startup inside a transaction and roll back as a whole on
  failure.
- **Ports** — do not add a `ports:` mapping for `typvia-sync`. The tunnel being
  the only way in is the security boundary of this setup.
- **A leaked token** — rotate the tunnel token in the Cloudflare dashboard,
  update `.env` and restart. The token authorises a tunnel connection, not data
  access; reading data still requires the account keys held by your devices.

## Why not Workers

The server is a stateful single binary backed by SQLite, and Workers has no
persistent filesystem. Porting it would mean writing a different server, not
deploying this one.
