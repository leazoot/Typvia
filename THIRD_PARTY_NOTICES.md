# Third-Party Notices

## Espanso

The Typvia desktop app bundles the official **Espanso** binary as its expansion engine.

- <https://espanso.org> — © Federico Terzi and the Espanso contributors
- GPL-3.0 — full text in [`licenses/espanso/GPL-3.0.txt`](licenses/espanso/GPL-3.0.txt), also shipped inside the application bundle
- Corresponding source: <https://github.com/espanso/espanso> at the version recorded in `vendor/espanso/VERSION`

Espanso ships **unmodified, as a separate program**: Typvia starts it as its own
process and speaks to it only through its command line and config files — never
linking against, embedding, or altering its source. Typvia itself stays MPL-2.0;
the sync server, AGPL-3.0.
