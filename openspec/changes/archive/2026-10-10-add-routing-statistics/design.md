# Design

## Decisions

- **Compute from existing rows, on read.** Attempts are already stored as JSON on each decision,
  and usage rows carry cost and tokens. Aggregation runs as a query at request time over a bounded
  window (at most the decision retention, default 90 days; the default window is 24 hours). No new
  tables or write-path cost. If reads get heavy, a rollup table is a later optimization.
- **Percentiles in the application.** SQLite has no percentile function; attempt latencies for a
  window are read (capped) and ranked with the nearest-rank method, which is exact and easy to test.
  A per-request cap on rows read keeps the cost bounded, and the response says when it truncated.
- **Same read model as decisions.** Own requests only, until an admin credential exists, so
  statistics cannot reveal other keys' traffic patterns.
- **Read-only first, like databases.** Statistics collection precedes any optimizer. Acting on them
  (cost-aware ordering within the allowed set, with exploration and operator hints) is a separate
  proposal, to be informed by what these numbers show in real use. Hard constraints from rules
  (access, privacy) must never be overridable by a cost model.
- **Endpoint grouping** uses the attempt's provider and model; target grouping uses the attempt's
  target; route grouping uses the decision's route.
- **Explain annotation** reuses the same query with a one-day window and a per-target filter.

## Risks / Trade-offs

- Statistics can mislead on small samples; every group reports its counts so operators can judge,
  and nothing acts on them.
- Provider behavior drifts; windows (not all-time averages) are the default for that reason.
- Quality is not measured; only latency, outcome, cost and size are.
