# Design

## Ledger scale harness

- Lives in `tests/ledger_scale.rs`, marked `#[ignore]`, run with
  `cargo test --release --test ledger_scale -- --ignored --nocapture`. The row count comes from an environment
  variable (default one million usage rows) so a developer can run a smaller one.
- Rows are generated from a seeded generator with realistic spread: a few dozen keys, tens of targets across several
  providers, a mix of outcomes (ok, unavailable, rate_limit, error), session ids on most rows, timestamps spread over
  a configurable number of days, and a decision row and quota sample per fixed number of usage rows.
- It uses the real `Ledger` write path (the queue and writer task) for the sustained-write measurement, and bulk SQL
  in one transaction for filling, since filling a million rows through the queue would measure the queue.
- Measured, each with a wall time: reopening the ledger, `spend_since` for the current day and month, `stats` grouped
  by route, target and endpoint over one day and thirty days, `session_scan`, `entries_since`, a decision prune past
  retention, and the file size before and after.
- The point is to find where cost stops being roughly constant. Numbers are reported with the row count so the shape
  (flat, linear, worse) is visible, run at 100k, 1M and optionally 5M.

## Gateway overhead harness

- Lives in `tests/overhead.rs`, `#[ignore]`, release mode. A mock upstream answers a fixed chat completion, with zero
  delay and with a fixed delay, so overhead is separated from upstream time.
- The same request is sent directly to the mock and through the gateway, at concurrency 1, 16 and 64, for a fixed
  number of requests after a warm-up. It reports p50, p90 and p99 latency for each path, the difference, and requests
  per second.
- It runs in one process on loopback, so it measures the gateway's own work and not the network. That limit is
  stated with the results.

## Documentation

`docs/performance.md` records each run: date, humpyard version, machine, command and numbers, then states what the
numbers do and do not show. It also lists, per ledger table (`usage`, `decisions`, `quota_samples`), whether it is
pruned and by which setting. Any other document that quotes a performance figure links to it.

## What this change does not decide

If `usage` growth or a slow query is a problem, the options (a retention setting for usage, an archival command,
a covering index, a different backend) are decided in a follow-up change with the numbers in hand.
