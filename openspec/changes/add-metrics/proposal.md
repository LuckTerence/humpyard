# Proposal

## Why

Operators watch humpyard today through `GET /v1/health`, `GET /v1/stats` and the ledger. That is
enough to answer "what did it cost" but not "is it healthy right now": request rate, error and
failover rate, latency by target, endpoint breaker state, ledger writer backlog. Those belong in the
monitoring stack the operator already runs. A Prometheus-format `/metrics` endpoint lets existing
Prometheus and Grafana setups, and alerting, work with no custom UI.

The next audience will not run Prometheus. Someone trying humpyard on a laptop should be able to see
the same live numbers in a page the binary serves. That page is a later change, but the metrics must
be built so it needs no rework: **one registry, many views**.

## What Changes

- **A metrics registry.** A small in-process registry of counters, gauges and fixed-bucket
  histograms, fed from the places that already record a decision or usage row. No new dependency.
- **`GET /metrics`.** Prometheus text exposition of the registry. Off by default.
- **A bounded set of labels.** Only values the operator wrote in configuration (route, target,
  provider, rule, key id when opted in) and small fixed sets (protocol, status class, outcome). Never
  a client-controlled string (model name as sent, header values, prompt text, tags).
- **Its own credential.** The endpoint is served either on a separate listener or on the main one
  behind a dedicated metrics credential. A client key never reads it, and the metrics credential
  never calls inference.
- **Out of scope:** the status page or any UI, a JSON view of the registry (the registry is shaped
  so one is a thin addition), traces, push gateways, alert rules and dashboards, per-request logs.

## Capabilities

### New Capabilities
- `metrics`: a registry of operational metrics and its Prometheus exposition.

### Modified Capabilities
- `gateway-config`: a `[metrics]` block.

## Impact

New `src/metrics.rs` (registry and text encoder, Core), hooks in `src/server.rs` where decisions and
usage are recorded, the ledger writer (queue depth and drops), the pool (breaker state) and the quota
poller (snapshot age), a route and auth path in `src/server`, config validation. The architecture
test needs the new module's layer. Docs: a new `docs/metrics.md` (metric list, label rules, example
Prometheus scrape config and alert rules), `configuration`, `api`, `architecture`, `roadmap`.
