# Tasks

## 1. Parsing and rendering
- [ ] 1.1 `src/migrate.rs`: parse the two responses (tolerating unknown fields), name targets and providers, map tags to strengths, render the TOML fragment; unit tests against recorded fixtures (a trimmed real response pair) for naming, collisions, tags, windows, banned and unkeyed skips
- [ ] 1.2 The table of base URLs for the providers ModelRelay supports and for `openai-compatible:<id>` instances; unknown providers reported
- [ ] 1.3 Drop-in routes and the catch-all strength for `auto-fastest`; tests for each route form and the `--min-ctx` variants

## 2. The command
- [ ] 2.1 `humpyard migrate-modelrelay` with `--url`/`--token-env` or `--config`/`--models`, `--class`, `--min-ctx`, `--out` (refused when it exists); never requests the key endpoint; tests with a mock ModelRelay that fails the test if the key endpoint is called
- [ ] 2.2 Self-check through `Config::from_toml` with placeholder keys; the report; exit codes; CLI tests
- [ ] 2.3 End to end: a migrated fragment, merged into a header, loads and serves a request through a `tag:general+min_ctx:32000` route against mock upstreams

## 3. Docs
- [ ] 3.1 `docs/migrating-from-modelrelay.md`, `README`, `roadmap.md` (step 5 built; step 4 stays optional), `architecture.md`
