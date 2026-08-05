# Desktop E2E

Opt-in, platform-native end-to-end scenarios for flows that no WebView-DOM
driver can reach — global shortcuts and cross-application text injection.

## Why not tauri-driver / WebdriverIO

`tauri-driver` supports Windows and Linux only; macOS WKWebView has no
WebDriver. And the flows here (summon via a **global** shortcut, inject into a
**third-party** app) live outside the WebView DOM on every platform. See
`docs/11_DECISIONS.md` **DEC-012** (closes OQ-P1). WebDriver stays on the table
for pure DOM flows and Linux CI, if those arise.

## Scenarios

| Script            | Scenario                                                                                                                               |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `panel_insert.sh` | ⌘⇧V summon → search → ↵ → text delivered into TextEdit, original clipboard restored, panel hidden with focus returned, usage recorded. |

## Running

```
zsh apps/desktop/e2e/panel_insert.sh   # exit 0 = pass, 1 = fail
```

Requirements (why this is opt-in, like the injector live tests):

- macOS with a GUI session.
- **Accessibility permission** granted to the terminal driving the run
  (System Settings → Privacy & Security → Accessibility) — needed to synthesize
  the global shortcut and keystrokes.
- Runs the app via `pnpm dev` (`tauri dev`); the packaged binary currently
  renders blank under the production CSP (a separate packaging concern noted in
  `docs/12_PROGRESS.md`).

The script seeds one snippet through the real schema, drives the run, asserts
delivery/clipboard/focus/usage, and cleans up the seed and processes. It is not
part of headless CI.
