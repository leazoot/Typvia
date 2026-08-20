# typvia-espanso-adapter

Integration adapter between Typvia and [Espanso](https://espanso.org), a
third-party, independently developed text expander that users install
themselves. Typvia does **not** bundle, install, or ship Espanso.

## Relationship with Espanso and communication mode

This crate interacts with Espanso only in two arm's-length ways:

1. **Separate-process CLI invocation** — detecting the install, reading the
   version, locating config directories (`espanso path`), and querying/reloading
   the service, all by executing the `espanso` binary as an external process and
   reading its output. See the [`EspansoCli`] boundary trait.
2. **Configuration files** — writing Typvia's expansions into an isolated
   `match/typvia/` subdirectory of the user's Espanso config (later tasks), a
   data format consumed by Espanso, never touching the user's own match files.

Espanso runs as its own process throughout; the two programs communicate at
"arm's length" over the command line and configuration files.

## No code-level dependency (GPL-3.0 isolation)

Espanso is licensed under **GPL-3.0**. This crate has **no code-level
dependency** on it:

- It does **not** link, embed, or depend on any `espanso-*` crate.
- It does **not** copy or re-implement Espanso source (including paraphrasing).
- The installer does **not** bundle the Espanso binary.
- "Espanso" is used nominatively (to name the tool) only, not as Typvia branding.

Under the standard reading of the GPL, `exec` + command-line / configuration-file
communication keeps the two works independent, so GPL-3.0 obligations do not
propagate to Typvia. Changing any of the above requires an explicit new
licensing decision.

[`EspansoCli`]: src/cli.rs
