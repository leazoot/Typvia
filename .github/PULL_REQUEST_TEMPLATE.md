# What changed and why

<!-- A short description of the change and the problem it solves. Link the issue it
closes, if there is one: Closes #123 -->

## Areas touched

<!-- Delete the lines that do not apply. -->

- [ ] Desktop app (`apps/desktop`)
- [ ] Mobile app (`apps/mobile`)
- [ ] Browser extension (`apps/browser-extension`)
- [ ] Sync server (`apps/sync-server`)
- [ ] Rust crates (`crates/*`)
- [ ] Shared TypeScript packages (`packages/*`)
- [ ] Native layers (`native/*`)
- [ ] Build, CI, or docs

## How it was verified

<!-- Which of these you ran, and anything you checked by hand. -->

- [ ] `pnpm lint`
- [ ] `pnpm format`
- [ ] `pnpm typecheck`
- [ ] `pnpm test`
- [ ] `pnpm build`
- [ ] `go test ./...` in `apps/sync-server` (sync server changes only)
- [ ] Verified manually — describe below

<!-- Manual verification notes, benchmark numbers, or anything CI cannot cover. -->

## Checklist

- [ ] Tests added or updated for the changed behaviour; a bug fix has a test that
      fails without the fix.
- [ ] No debug logging, commented-out code, or temporary files left in the diff.
- [ ] No secrets, credentials, personal data, or real snippet content anywhere in
      the diff — including tests, fixtures, and commit messages.
- [ ] Sensitive content stays out of logs, error messages, plaintext indexes, and
      generated Espanso configuration.
- [ ] Screenshots or a short recording attached for user-visible UI changes, in
      both light and dark themes.
- [ ] Commits are signed off for the Developer Certificate of Origin (`git commit -s`).

<!-- Screenshots go here. Do not show real snippet content — use placeholder text. -->
