#!/usr/bin/env bash

# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# SPDX-License-Identifier: MPL-2.0

# One-time local dev signing identity: a self-signed
# code-signing certificate in the login keychain gives rebuilt bundles a
# stable identity, so keychain ACLs and TCC grants survive repackaging
# during walkthroughs. No Apple account involved; the private key lives
# only in the keychain and the working files are deleted on exit. The
# Developer ID chain supersedes this once certificates exist.
#
# macOS asks for the login password once (trusting the certificate), and
# once more on first codesign use ("Always Allow" makes it stick).
#
# Usage: scripts/release/macos_dev_cert.sh
set -euo pipefail

IDENTITY="${TYPVIA_DEV_SIGN_IDENTITY:-Typvia Dev Signing}"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning | grep -Fq "$IDENTITY"; then
  echo "identity already present: $IDENTITY"
  exit 0
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

cat > "$WORK/req.cnf" <<EOF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $IDENTITY
[ext]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
EOF

openssl req -x509 -newkey rsa:2048 -days 3650 -nodes \
  -keyout "$WORK/key.pem" -out "$WORK/cert.pem" -config "$WORK/req.cnf"

# Import key and certificate as PEM directly (OpenSSL 3 p12 containers
# use algorithms the keychain importer rejects).
security import "$WORK/key.pem" -k "$KEYCHAIN" -T /usr/bin/codesign
security import "$WORK/cert.pem" -k "$KEYCHAIN"

# User trust domain; this is the login-password dialog.
security add-trusted-cert -p codeSign -k "$KEYCHAIN" "$WORK/cert.pem"

security find-identity -v -p codesigning | grep -F "$IDENTITY"
echo "created: $IDENTITY"
echo "sign builds with: APPLE_SIGNING_IDENTITY=\"$IDENTITY\""
