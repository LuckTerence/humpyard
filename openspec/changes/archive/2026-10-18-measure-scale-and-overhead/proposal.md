# Proposal

## Why

Two questions about humpyard have no measured answer, and both matter for decisions already on the table.

1. **How does the ledger behave at scale?** The ledger is one SQLite file. Decisions and quota samples are pruned
   after `decision_retention_days` (default 90), but `usage` rows, the spend history that budgets are computed from,
   are kept forever. Free pools with many models make requests and failover attempts plentiful, so the table grows
   faster than before. A public discussion of LiteLLM alternatives names Postgres-backed logging slowing down past
   about a million rows as a main complaint; humpyard has not shown that it avoids the same problem.
2. **What does the gateway add to a request?** Nothing in the documentation states an overhead or a throughput, and
   nothing should until it is measured. The answer also tells us whether capacity is ever a reason to run more than
   one replica.

## What Changes

- A **ledger scale harness**: fills a ledger with a configurable number of synthetic usage, decision and quota rows
  (seeded, so a run is reproducible) and times the operations the gateway really performs: startup budget
  hydration, per-key spend lookups, the stats and session queries over short and long windows, the decision prune,
  and sustained writes. It reports the database size.
- A **gateway overhead harness**: sends the same requests straight to a mock upstream and through the gateway at
  several concurrencies and reports the added latency and the throughput.
- **Documented results** in `docs/performance.md`: method, hardware, date and caveats, plus a statement of what is
  and is not pruned for each ledger table. No performance claim is made anywhere else without a measurement there.
- Out of scope: any change to the ledger or the request path. Findings that call for one become their own change.

## Capabilities

### New Capabilities
- `performance`: reproducible measurement harnesses for ledger scale and gateway overhead, and the rule that
  performance statements rest on recorded measurements.

## Impact

Two ignored-by-default test harnesses, one documentation page, and an ADR-free decision log of findings. No change to
serving behavior, the configuration or the API.
