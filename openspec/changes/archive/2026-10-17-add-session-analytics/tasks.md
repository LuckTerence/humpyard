# Tasks

## 1. Cache numbers

- [x] 1.1 Token totals and `cache_hit_ratio` on every group of `GET /v1/stats` (route, target, endpoint); `usage_missing` rows excluded; tests with known cached and uncached calls, a zero-prompt group, and unchanged existing fields
- [x] 1.2 `cached_input` and `cache_creation` directions on `humpyard_tokens_total`, a cache-hit panel on the example dashboards, kept in step by the drift test

## 2. Sessions

- [x] 2.1 Ledger schema v7: the `usage(session_id, ts_ms)` index, with an in-place upgrade test (built offline like the earlier ones)
- [x] 2.2 `group_by=session` with `limit`, `order_by`, endpoints in order of first use, switches, first and last time, and the count of calls without a session id; per-key isolation test; truncation flag
- [x] 2.3 `GET /v1/stats/switches`: sessions, switches, warm tokens abandoned, coverage, endpoint pairs, stayed versus switched; the dollar estimate priced from the loaded config in the server, with priced and unpriced counts; tests with a controlled ledger (single switch, repeated switches, failover inside a target, no session id, unpriced destination, truncation, per-key isolation)

## 3. Docs

- [x] 3.1 `budgets.md` (Statistics: cache numbers, sessions, how to read the switch report, what "warm tokens abandoned" means and its limits), `api.md`, `metrics.md`, architecture and the roadmap (item 2 shipped; item 3's decision rule: build stickiness only if the report shows a meaningful share of spend)
