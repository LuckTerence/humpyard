# Proposal

## Why

A model's prompt cache is a real cost lever: input tokens served from cache are billed at a fraction of the
fresh price, and a long agent session lives on it. Switching models in the middle of a session (a failover,
a route that changes its pick, a random route) throws that cache away: the next call pays full price for
tokens the previous model already had warm. Vals.ai's routers article names this as the thing a router must
weigh, and the roadmap lists session stickiness (item 3) as the possible answer. Nothing in humpyard can
tell today whether the loss is large or negligible: `GET /v1/stats` can group only by route, target or
endpoint, and it reports no cache numbers at all, though the ledger already stores each call's session id,
cached input tokens and cache-creation tokens.

The project's rule is to measure before routing on a signal (quota, health, capability all followed it).
This change makes the cost of switching, and the cost of a whole session, visible, so that stickiness is
built only if the numbers justify it.

## What Changes

- **Cache numbers everywhere stats already go.** Every group in `GET /v1/stats` gains its token totals
  (fresh input, cached input, cache creation, output) and a `cache_hit_ratio`.
- **`group_by=session`.** Sessions are listed with their cost, calls, cache-hit ratio, the endpoints they
  used in order and how many times they switched, newest or costliest first, capped at `limit`.
- **A switch report.** `GET /v1/stats/switches` summarizes, over a window: how many sessions switched
  endpoints, how many cached tokens the switches abandoned, the extra cost that implies at the
  destination's configured prices, its share of total spend, the endpoint pairs that switch most, and how
  sessions that stayed compare with sessions that switched (described, not claimed as causal).
- **Coverage.** The same report says what share of answered calls carried a session id, because everything
  above is only as good as the clients that send one (roadmap item 3.1).
- **Cache tokens in Prometheus.** `humpyard_tokens_total` gains the directions `cached_input` and
  `cache_creation`, so a dashboard can show the cache-hit ratio live; session ids are never metric labels.
- **Read-only.** Nothing here changes routing. Whether to pin a session to a target is a later, separate
  change that this data decides.
- **Out of scope:** any routing or stickiness behavior, identifying which client sent a session id,
  per-session metrics series (unbounded cardinality), exporting sessions, and changing what the ledger
  stores about content (it stores none).

## Capabilities

### Modified Capabilities
- `routing-statistics`: cache numbers, session grouping, the switch report, coverage.
- `metrics`: cache token directions.
- `usage-ledger`: an index that keeps session queries cheap.

## Impact

`src/ledger/stats.rs` (token sums and a cache ratio per group, a session grouping, the switch scan),
`src/ledger.rs` (schema v7: an index on `usage(session_id, ts_ms)`), `src/server/stats.rs` (`group_by=session`,
`limit`, `order_by`, the `/v1/stats/switches` route, pricing the estimate from the loaded config),
`src/metrics.rs`, the metrics dashboards (a cache-hit panel). Docs: budgets (Statistics), api, metrics,
roadmap, architecture.
