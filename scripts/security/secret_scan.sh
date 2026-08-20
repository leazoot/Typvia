#!/usr/bin/env bash
# Local secret scan: the same gitleaks rules CI enforces, for the
# pre-commit self-check. Findings are redacted —
# the scanner must never print the very secret it found (log red line).
#
# Install: brew install gitleaks
# Usage: scripts/security/secret_scan.sh [extra gitleaks args]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

if ! command -v gitleaks >/dev/null; then
  echo "gitleaks is not installed — brew install gitleaks" >&2
  exit 2
fi

# Committed history and the uncommitted working tree are separate scans;
# both must be clean.
gitleaks git --config .gitleaks.toml --redact --no-banner --exit-code 1 "$@" .
gitleaks dir --config .gitleaks.toml --redact --no-banner --exit-code 1 "$@" .
echo "secret scan: clean"
