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
