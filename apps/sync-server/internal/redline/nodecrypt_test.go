// Typvia sync server
// Copyright (C) 2026 Typvia contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at
// your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero
// General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

// Package redline holds the server security red-line tests.
//
// Zero-decryption red line: the sync server is dumb storage — it must never
// gain the ability to decrypt user content. This test statically scans every
// .go source file in the module and rejects any import that provides
// decryption or private-key cryptography.
//
// Allowlist rationale:
//   - crypto/ed25519: signature VERIFICATION only (public-key operation,
//     admission filtering; clients remain the verification
//     authority).
//   - crypto/sha256: hashing (signature input digests, no secrets).
//   - crypto/rand:   CSPRNG (challenges, session tokens).
//   - crypto/subtle: constant-time comparison (token checks).
//
// Everything else under crypto/, all of golang.org/x/crypto (AEADs, NaCl
// boxes, curve25519, ...), and known third-party AEAD/keywrap packages are
// denied. Growing the allowlist is a protocol-level decision, not a code
// change: it requires updating the sync protocol first.
package redline

import (
	"go/parser"
	"go/token"
	"io/fs"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"
)

// allowedCryptoImports are the only crypto-adjacent stdlib packages the
// server may import.
var allowedCryptoImports = map[string]bool{
	"crypto/ed25519": true,
	"crypto/sha256":  true,
	"crypto/rand":    true,
	"crypto/subtle":  true,
}

// deniedPrefixes reject entire dependency families that carry decryption or
// private-key capability, including anything a future dependency might pull
// in under these paths.
var deniedPrefixes = []string{
	"golang.org/x/crypto", // chacha20poly1305, nacl, openpgp, curve25519, ...
	"filippo.io/age",
	"github.com/minio/sio",
}

func TestNoDecryptionCapabilityImports(t *testing.T) {
	root := moduleRoot(t)
	fset := token.NewFileSet()
	var scanned int

	err := filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() || !strings.HasSuffix(path, ".go") {
			return nil
		}
		scanned++
		f, err := parser.ParseFile(fset, path, nil, parser.ImportsOnly)
		if err != nil {
			t.Fatalf("parse %s: %v", path, err)
		}
		for _, imp := range f.Imports {
			p, err := strconv.Unquote(imp.Path.Value)
			if err != nil {
				t.Fatalf("unquote import in %s: %v", path, err)
			}
			checkImport(t, path, p)
		}
		return nil
	})
	if err != nil {
		t.Fatalf("walk module: %v", err)
	}
	if scanned == 0 {
		t.Fatal("no .go files scanned; red-line test is not covering the module")
	}
}

func checkImport(t *testing.T, file, imp string) {
	t.Helper()
	if imp == "crypto" || strings.HasPrefix(imp, "crypto/") {
		if !allowedCryptoImports[imp] {
			t.Errorf("%s imports %q: crypto imports outside the allowlist violate the zero-decryption red line", file, imp)
		}
		return
	}
	for _, prefix := range deniedPrefixes {
		if imp == prefix || strings.HasPrefix(imp, prefix+"/") {
			t.Errorf("%s imports %q: decryption-capable dependency violates the zero-decryption red line", file, imp)
		}
	}
}

// moduleRoot resolves the module root relative to this test file
// (internal/redline → two levels up).
func moduleRoot(t *testing.T) string {
	t.Helper()
	_, thisFile, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("cannot resolve test file location")
	}
	return filepath.Clean(filepath.Join(filepath.Dir(thisFile), "..", ".."))
}
