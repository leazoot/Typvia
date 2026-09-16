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

## JetBrains Mono

The Typvia iOS app and its extensions bundle the **JetBrains Mono** typeface, in
Regular and Bold, for trigger words, type marks, code and counts.

- <https://www.jetbrains.com/lp/mono/> — © 2020 The JetBrains Mono Project Authors
- SIL Open Font License 1.1 — full text in
  [`apps/ios/Resources/Fonts/OFL.txt`](apps/ios/Resources/Fonts/OFL.txt), which ships
  in every product that carries the font, as the licence requires
- Upstream: <https://github.com/JetBrains/JetBrainsMono> — the font files are the
  unmodified `ttf/` release artefacts

The font files are data, not linked code: bundling them places no licence
requirement on Typvia's own source, which stays MPL-2.0.
