# Tasks

## 1. Decide cold, skip it

- [x] 1.1 Config: `[health] skip_cold_targets` (default true), validation and docs; tests (default, off)
- [x] 1.2 `is_cold` over `TargetClient::health()` (all endpoints cooling down; probing is warm) and the two-pass `plan_route` (warm check, then fail-open); tests with mock upstreams that count calls: a cold target gets zero calls when an alternative is warm, fail-open when all are cold, tier substitution on a cold tier, random weights realigned, a strict route skips only within its allowed targets and keeps a cold single allowed target, the switch off restores the old order
- [x] 1.3 Recovery test: after the cooldown the next request is the probe and success restores the target (uses the clock hook)

## 2. See it

- [x] 2.1 Decisions: `cold_targets` (ledger schema v5, nullable JSON column, in-place upgrade test), `GET /v1/decisions`
- [x] 2.2 `explain`: per-target `cold` and a route-level `health_fail_open`
- [x] 2.3 Metric `humpyard_health_skips_total{route,target}`, a dashboard panel, kept in step by the example drift test

## 3. Docs

- [x] 3.1 `routing.md` (Endpoint health: remove the "Not yet" note), `configuration.md`, `api.md`, invariant 14 restated with its enforcing tests, architecture status row, roadmap; ADR 0018 accepted
