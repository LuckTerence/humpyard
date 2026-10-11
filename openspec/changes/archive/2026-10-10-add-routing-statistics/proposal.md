# Proposal

## Why

Routing today is rule based: selector rules, ordered pools, random spreading and a binary endpoint
breaker. Database optimizers began the same way and became good by adding measured statistics
before adding cost-based choice. The decision log (ADR 0013) already records, per request, the
route, the ordered attempts with outcomes and milliseconds, and the usage rows carry tokens and
cost. Nothing summarizes them, so operators cannot answer "which target has been throwing 429s",
"how slow is this pool really", or "what does a request on this route cost". Any later cost-aware
or auto routing needs these numbers first, and they are useful on their own.

## What Changes

- **Statistics over the decision log and usage rows**, per route and per target (and per
  provider/model endpoint): request and attempt counts, outcome mix by class (`ok`, `rate_limit`,
  `timeout`, `server_error`, `connection`, ...), latency percentiles (p50, p90, p99) of attempts,
  fallback rate, and cost and tokens per request, over a chosen window.
- **`GET /v1/stats`** with `window` (for example `1h`, `24h`, `7d`, default `24h`), `group_by`
  (`route`, `target`, `endpoint`) and `route`/`target` filters. A key sees statistics over its own
  requests only (the same read model as decisions); an open gateway sees all.
- **Statistics in `explain`.** When the ledger is available, `explain` annotates each target it
  would use with its recent failure rate and latency, so an operator sees why a route is slow or
  fragile before changing it.
- **Read-only.** Nothing in routing changes: no ordering, weighting or selection uses these
  numbers yet. That is a separate, later change (cost-aware choice within the allowed set).

Out of scope: acting on statistics (ordering, weighting, exploration), quality signals, per-model
price history, exporting to external monitoring (`/metrics` is its own roadmap item).

## Capabilities

### New Capabilities
- `routing-statistics`: summaries of how routes and targets have actually performed.

### Modified Capabilities

## Impact

New `src/ledger/stats.rs` (aggregation queries and percentile computation),
`src/server/stats.rs`, `explain` annotation, an index if needed, tests, docs, README.
