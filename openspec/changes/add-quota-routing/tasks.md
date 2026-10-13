# Tasks

## 1. Observe (no routing behavior)

- [x] 1.1 Quota snapshot model and the OpenCode Go adapter: strict parsing of recorded fixtures (valid, clamped over 100, malformed window, no usable window, unknown status text); tests without network
- [x] 1.2 Config: `quota_source`, `quota_poll_secs` (default 60, 1 to 3600; the minimum is low so tests can poll quickly, 15 or more is sensible in production), `quota_stale_secs` (default 300); validation; origin and https rules for the poll URL
- [x] 1.3 Poller task in `serve`: off the request path, timeout, no key in logs or snapshots; tests for hung, failing and malformed sources never affecting requests, and for staleness
- [x] 1.4 `GET /v1/quota` with snapshot age and unknown reasons; reload keeps the snapshot of unchanged providers; tests
- [x] 1.5 Docs and ADR 0015 accepted; an architecture note that the poller is the only runtime source fetch

## 2. Measure

- [ ] 2.1 Run it against the real account for several days; compare percent deltas with ledger cost; record what the percent is a percent of, the `status` values seen, and the behavior at a cap (decision input for step 3)

## 3. Use it in rules (after step 2)

- [ ] 3.1 `when.quota` condition, validation, `explain` and decision recording; tests (match, below, unknown, stale, each window, invalid configs)
- [ ] 3.2 Pacing recipe in `docs/routing.md` and the auto-routing example; verify with `explain`

## 4. Later, separate proposals

- [ ] 4.1 Per-model quota if a provider publishes it; adapters for other providers
- [ ] 4.2 Anything that acts on quota without an operator rule (advisory ordering), only after weeks of statistics
