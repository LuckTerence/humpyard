# 0016. Metrics are one registry with bounded labels

- Status: accepted
- Date: 2026-10-14
- Supersedes: none
- Change: `openspec/changes/add-metrics`

## Context

Operators need live health numbers (rate, errors, failovers, latency, breaker state, backlog), and the
roadmap adds an audience that will not run Prometheus and wants the same numbers in a page the binary
serves. Metric systems fail in two known ways: label values taken from request data explode the
number of series and leak content, and a second view built later re-derives numbers that then
disagree with the first.

## Decision

Operational metrics live in one in-process registry owned by `src/metrics.rs`. Every operator-facing
view reads that registry: the Prometheus text endpoint now, a JSON view for a status page later.
Label values come only from the loaded configuration or from fixed sets; a client-supplied string
never becomes a label. Identity (configured key ids) is off by default and opt-in, only on a few counters and never on histograms, with a cap on distinct values. The endpoint has its own
credential and is off by default. Counters are monotonic, money is integer micro-USD.

Alternatives considered: the `prometheus` or `metrics` crates (a dependency on the serving path and a
second place that owns metric names, with no gain for a set this small); deriving metrics from the
ledger at scrape time (reads on the hot store, and it cannot see refusals or breaker state);
per-key labels on by default (a topology and identity leak to anyone who can scrape).

## Consequences

Adding a metric means adding a row to the registry table, which gives every view the metric at once.
We must keep label sets bounded and tested against hostile input, and the exposition must stay valid
text format. Counters reset on restart, so the ledger remains the source of truth for cost. A status
page later is a thin view, not a new data path.
