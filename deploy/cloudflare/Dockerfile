# Typvia sync server image. The server stores ciphertext and metadata only —
# this image adds nothing that could see plaintext. Static build (pure-Go
# SQLite driver), distroless runtime, non-root.

# Builds on the host's own architecture and cross-compiles to the target, so a
# multi-arch build (amd64 + arm64) never runs the compiler under emulation.
FROM --platform=$BUILDPLATFORM golang:1.25-alpine AS build
WORKDIR /src
COPY apps/sync-server/go.mod apps/sync-server/go.sum ./
RUN go mod download
COPY apps/sync-server/ ./
ARG TARGETOS
ARG TARGETARCH
RUN CGO_ENABLED=0 GOOS=${TARGETOS:-linux} GOARCH=${TARGETARCH:-amd64} \
    go build -trimpath -ldflags="-s -w" \
    -o /out/typvia-sync-server ./cmd/typvia-sync-server \
    && mkdir /out/data

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /out/typvia-sync-server /typvia-sync-server
# Pre-created with the runtime uid as owner: a fresh volume inherits this
# ownership, and the nonroot server can create its SQLite files (a bare
# VOLUME would be root-owned and unwritable — found in template testing).
COPY --from=build --chown=65532:65532 /out/data /data
VOLUME ["/data"]
ENV TYPVIA_SYNC_LISTEN=0.0.0.0:8787 \
    TYPVIA_SYNC_DB=/data/typvia-sync.db
EXPOSE 8787
USER nonroot
ENTRYPOINT ["/typvia-sync-server"]
