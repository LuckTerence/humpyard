# Performance and scale

Every performance figure about humpyard should be traceable to a run recorded on this page. Nothing here is a
capacity guarantee: each run states what it measured and what it did not. The harnesses are in the repository and
are not part of the default test run:

```sh
# Ledger scale: LEDGER_SCALE_ROWS usage rows (default 1,000,000) over LEDGER_SCALE_DAYS days (default 30)
LEDGER_SCALE_ROWS=1000000 cargo test --release --test ledger_scale -- --ignored --nocapture

# Gateway overhead: direct to a mock upstream versus through the gateway
cargo test --release --test overhead -- --ignored --nocapture
```

## What the ledger keeps, and what it prunes

| Table | Holds | Pruned? | Setting |
|---|---|---|---|
| `usage` | one row per upstream call: tokens, cost, outcome (the spend history budgets are computed from) | **No**, kept forever | none |
| `decisions` | one row per request: routing, attempts, outcome (read by `/v1/stats`, `explain`, the decision log) | Yes | `decision_retention_days` (default 90, 0 keeps everything) |
| `quota_samples` | provider quota readings | Yes, with the decisions | `decision_retention_days` |

The pruning runs inside the ledger's writer, so while it deletes, queued writes wait. At steady state each
prune removes about a day of rows and is short; the first prune after a long gap, or after lowering the setting,
removes many at once (see the run below).

## Run 1: the ledger at 100,000 and 1,000,000 usage rows (2026-10-18)

humpyard 0.9.5, Apple M5 (10 cores, 16 GiB), macOS 26.6, rustc 1.99.1, local SSD, `--release`, seed 1. Rows are
synthetic and chronological over 30 days: 30 keys, 8 providers, 6 routes, 70 percent of rows with a session id,
85 percent `ok` outcomes. A decision accompanies two of every three usage rows, and one quota sample per 1,000.
The fill goes through the real queue and writer, so it is also the sustained-write measurement.

| | 100,000 rows | 1,000,000 rows |
|---|---:|---:|
| Rows written (usage / decisions / quota) | 100,000 / 66,667 / 100 | 1,000,000 / 666,667 / 1,000 |
| Sustained write, usage + decision rows per second | 46,380 | 35,500 |
| Database size (db + wal) | 55.7 MiB | 523.9 MiB |
| Reopen the ledger | 0.8 ms | 1.3 ms |
| `spend_since`, last day | 2.8 ms | 203.5 ms |
| `spend_since`, whole 30-day window (budget hydration at start-up) | 45.3 ms | 1,267.8 ms |
| `/v1/stats` by route, last day | 8.9 ms | 88.1 ms |
| `/v1/stats` by route, whole window | 144.0 ms | 483.6 ms |
| `/v1/stats` by target / endpoint, whole window | 149.8 / 158.4 ms | 507.0 / 577.0 ms |
| Session scan, last day / whole window | 9.9 / 160.1 ms | 114.3 / 885.9 ms |
| Decisions, newest 100 | 0.5 ms | 0.6 ms |
| Decisions, newest 100 of one route | 1.1 ms | 1.2 ms |
| Decisions, one request id | 10.8 ms | 308.3 ms |
| Prune decisions older than 7 days | 72.7 ms (51,112 rows) | 1,283.2 ms (511,112 rows) |

### What it shows

- **Writes are far faster than any real need.** 35,000 to 46,000 rows per second against the single writer; the
  deployment this was built for records a few hundred rows a day.
- **Two queries do not scale the way an index would suggest.**
  - `spend_since` filters on the timestamp alone, but the only index that contains it is `(key_id, ts_ms)`, so it
    cannot seek; its cost grows with the whole table (a one-day window took 72 times longer at 10 times the
    rows). It runs once, at start-up, to rebuild budget counters, so the cost is a slower restart, not a slower
    request.
  - The decisions lookup by request id uses an optional-filter pattern (`? IS NULL OR request_id = ?`) that stops
    SQLite from using the `request_id` index (308 ms at 1M rows). It is a debugging query, not on the request path.
- **`/v1/stats` and the session view are bounded by design.** They read at most 50,000 rows (the response says
  when it truncated), so their cost stops growing once a window holds more than that.
- **The `usage` table is the one that grows without limit.** It is never pruned.
- **How far away is a million rows?** On the author's gateway, measured on 2026-10-18 from `/v1/stats`: 376
  usage rows in a day (a busy day, with testing), which is about 2,700 days to reach 1,000,000. A team sending
  a thousand times that traffic would reach it in about three days of the same rate, so this matters only
  for shared deployments.

### Limits of this run

One machine and one local disk, synthetic rows, one pass per measurement (not averaged), SQLite's page cache warm
from the fill. It did not measure disk-full behavior, concurrent readers during writes, or a cold start after
a reboot. It says nothing about a different filesystem (network storage is not supported for the ledger).

## Run 2: gateway overhead (2026-10-18)

Same machine and version. The mock upstream returns a fixed completion and the gateway has a ledger on local
disk, so each gateway request includes its ledger write. The client, the mock and the gateway share one process
and one multi-threaded runtime on loopback, so the client and the mock take CPU from the gateway too; this
overstates the gateway's cost under load more than it understates it. Latencies are milliseconds; "overhead" is the
gateway figure minus the direct figure.

| Upstream | Concurrency | Path | p50 | p90 | p99 | req/s | Overhead p50 / p99 |
|---|---:|---|---:|---:|---:|---:|---|
| instant | 1 | direct | 0.02 | 0.03 | 0.05 | 37,944 | |
| instant | 1 | gateway | 0.09 | 0.10 | 0.14 | 10,853 | +0.07 / +0.09 |
| instant | 16 | direct | 0.12 | 0.17 | 0.22 | 128,308 | |
| instant | 16 | gateway | 0.34 | 0.41 | 0.49 | 46,466 | +0.22 / +0.27 |
| instant | 64 | direct | 0.44 | 0.60 | 0.92 | 138,959 | |
| instant | 64 | gateway | 1.29 | 1.62 | 2.10 | 48,437 | +0.85 / +1.18 |
| 50 ms | 1 | direct | 52.28 | 52.46 | 52.92 | 19 | |
| 50 ms | 1 | gateway | 52.85 | 53.27 | 53.51 | 19 | +0.57 / +0.59 |
| 50 ms | 16 | direct | 52.40 | 52.72 | 53.57 | 306 | |
| 50 ms | 16 | gateway | 52.81 | 53.66 | 54.21 | 303 | +0.41 / +0.64 |
| 50 ms | 64 | direct | 52.41 | 53.47 | 54.15 | 1,219 | |
| 50 ms | 64 | gateway | 53.64 | 55.57 | 59.88 | 1,188 | +1.22 / +5.72 |

### What it shows

- Against an upstream that takes 50 ms, the gateway adds well under a millisecond at the median up to 16 concurrent
  requests and about a millisecond and a quarter at 64. Real providers take hundreds of milliseconds to seconds, so
  the gateway's own time is not what a client waits for.
- Throughput through the gateway on this machine was above 46,000 requests per second against an instant upstream,
  far beyond what any provider lets one operator send. **Capacity is not a reason to run more than one replica**
  for a deployment like this; availability during deploys is.

### Limits of this run

One pass per cell; a repeat run of the instant-upstream cells gave +0.07 ms at concurrency 1 and +0.92 ms at 64
(the table says +0.85 ms), so read the figures as good to a tenth of a millisecond or so at the median. A mock upstream, plain HTTP on loopback (no TLS), non-streaming responses, one route and one target, no failover,
no tool or image content, and the single-process contention described above. Streaming overhead, TLS, large
bodies, memory use under load and behavior over a long soak were not measured.

## Open findings

These are recorded here, not fixed. Each would be its own change with its own spec.

1. **`usage` is never pruned.** Whether it needs a retention setting or an archival command depends on the
   deployment; for a single operator it is not urgent.
2. **`spend_since` cannot use an index on the timestamp.** An index on the timestamp, or a different query shape,
   would make restarts fast on very large ledgers.
3. **The request-id lookup scans.** Rewriting the optional filters so SQLite can use the `request_id` index fixes it.
4. **A prune after a long gap blocks writes** for as long as it runs (1.3 s for 511,112 rows here).
