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

package config

import (
	"strings"
	"testing"
)

// getenvFrom builds a getenv function backed by a map, keeping tests free of
// process-global state so they can run in parallel.
func getenvFrom(env map[string]string) func(string) string {
	return func(key string) string { return env[key] }
}

func TestLoadUsesDefaultsWithoutFlagsOrEnv(t *testing.T) {
	t.Parallel()
	cfg, err := Load(nil, getenvFrom(nil))
	if err != nil {
		t.Fatalf("load: %v", err)
	}
	if cfg.ListenAddr != DefaultListenAddr || cfg.DBPath != DefaultDBPath {
		t.Fatalf("unexpected defaults: %+v", cfg)
	}
}

func TestLoadReadsEnvironmentVariables(t *testing.T) {
	t.Parallel()
	cfg, err := Load(nil, getenvFrom(map[string]string{
		"TYPVIA_SYNC_LISTEN": "127.0.0.1:9999",
		"TYPVIA_SYNC_DB":     "/var/lib/typvia/sync.db",
	}))
	if err != nil {
		t.Fatalf("load: %v", err)
	}
	if cfg.ListenAddr != "127.0.0.1:9999" || cfg.DBPath != "/var/lib/typvia/sync.db" {
		t.Fatalf("env not applied: %+v", cfg)
	}
}

func TestLoadFlagsOverrideEnvironment(t *testing.T) {
	t.Parallel()
	cfg, err := Load(
		[]string{"-listen", "127.0.0.1:7777", "-db", "flag.db"},
		getenvFrom(map[string]string{
			"TYPVIA_SYNC_LISTEN": "127.0.0.1:9999",
			"TYPVIA_SYNC_DB":     "env.db",
		}),
	)
	if err != nil {
		t.Fatalf("load: %v", err)
	}
	if cfg.ListenAddr != "127.0.0.1:7777" || cfg.DBPath != "flag.db" {
		t.Fatalf("flags must beat env: %+v", cfg)
	}
}

func TestLoadRejectsEmptyValues(t *testing.T) {
	t.Parallel()
	if _, err := Load([]string{"-listen", ""}, getenvFrom(nil)); err == nil {
		t.Fatal("empty listen address must be rejected")
	}
	if _, err := Load([]string{"-db", ""}, getenvFrom(nil)); err == nil {
		t.Fatal("empty database path must be rejected")
	}
}

func TestLoadRejectsUnknownFlags(t *testing.T) {
	t.Parallel()
	_, err := Load([]string{"-bogus"}, getenvFrom(nil))
	if err == nil || !strings.Contains(err.Error(), "parse flags") {
		t.Fatalf("unknown flag must fail parsing, got %v", err)
	}
}
