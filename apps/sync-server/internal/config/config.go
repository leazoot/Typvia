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

// Package config loads server configuration from command-line flags and
// environment variables. Precedence: flag > environment variable > default.
package config

import (
	"flag"
	"fmt"
)

const (
	// DefaultListenAddr binds to loopback by default: production deployments
	// must sit behind TLS, so exposing a plaintext listener on all interfaces
	// is never a safe default.
	DefaultListenAddr = "127.0.0.1:8787"
	// DefaultDBPath is relative to the server working directory.
	DefaultDBPath = "typvia-sync.db"

	envListenAddr = "TYPVIA_SYNC_LISTEN"
	envDBPath     = "TYPVIA_SYNC_DB"
)

// Config holds the runtime configuration of the sync server.
type Config struct {
	// ListenAddr is the TCP address the HTTP server binds to.
	ListenAddr string
	// DBPath is the SQLite database file path.
	DBPath string
}

// Load parses configuration from args (program arguments without the program
// name) and the environment accessed through getenv. Both are injected so
// tests can run in parallel without touching process-global state.
func Load(args []string, getenv func(string) string) (Config, error) {
	listenDefault := DefaultListenAddr
	if v := getenv(envListenAddr); v != "" {
		listenDefault = v
	}
	dbDefault := DefaultDBPath
	if v := getenv(envDBPath); v != "" {
		dbDefault = v
	}

	fs := flag.NewFlagSet("typvia-sync-server", flag.ContinueOnError)
	listen := fs.String("listen", listenDefault, "TCP listen address (env "+envListenAddr+")")
	dbPath := fs.String("db", dbDefault, "SQLite database file path (env "+envDBPath+")")
	if err := fs.Parse(args); err != nil {
		return Config{}, fmt.Errorf("parse flags: %w", err)
	}

	if *listen == "" {
		return Config{}, fmt.Errorf("listen address must not be empty")
	}
	if *dbPath == "" {
		return Config{}, fmt.Errorf("database path must not be empty")
	}
	return Config{ListenAddr: *listen, DBPath: *dbPath}, nil
}
