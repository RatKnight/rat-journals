# Scripts

Development helpers for Rat Journal.

| Command | Purpose |
|---------|---------|
| `bun run check` | Type-check + ESLint + Prettier + locale validation + repo guards (no tests) |
| `bun run pre-commit` | The above + frontend tests + script tests + Rust tests + clippy + rustfmt |
| `bun run test:scripts` | `node:test` suites for these scripts (`scripts/*.test.{js,mjs,cjs}`) |
| `bun run check:test-runners` | Fails when a tracked `*.test.*` / `*.spec.*` file belongs to no test runner |
| `bun run test:e2e` | End-to-end tests against the release build (`run-e2e.js`) |
| `bun run validate:locales` | Every locale file must have every key (`validate-locales.ts`) |

`scripts/check-no-network.ps1` checks that the built app makes no network connections.

## Test runners

Every test file must belong to a runner (`scripts/check-test-runners.mjs` enforces it):

| Files | Runner |
|-------|--------|
| `*.{test,spec}.*` outside `e2e/`, `tests/`, `scripts/` (mostly `src/**`) | Vitest (`bun run test:run`) |
| `scripts/*.test.{js,mjs,cjs}` (top level only) | `node:test` (`bun run test:scripts`) |
| `tests/print/**` | Playwright (`bun run test:print`) |
| `e2e/specs/**/*.spec.ts` | WebdriverIO (`bun run test:e2e`) |

When you change a runner's include or exclude list, change the matching rule in `scripts/check-test-runners.mjs` in the same commit.

The guard runs Git by absolute path (`/usr/bin/git`, or `Git\cmd\git.exe` under Program Files on Windows). For another location set `MINI_DIARIUM_GIT_EXECUTABLE` to a trusted absolute path.

## Git hook

`scripts/install-hooks.js` runs on `bun install` and points `core.hooksPath` at `.githooks/`. The hook formats staged files only (Prettier for `src/**`, `cargo fmt` for Rust) and re-stages them. Bypass once with `git commit --no-verify`.
