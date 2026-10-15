# Design

## Context

Three things already observe requests: the decision record (ADR 0013), the usage row in the ledger
(ADR 0005) and the endpoint breaker (ADR 0009). Metrics are a fourth, cheaper view: aggregates in
memory, lost on restart, scraped by something else that keeps history. They must never be on the
critical path and never carry content.

## Decisions

- **One registry, many views.** Handlers update the registry; encoders read it. The Prometheus text
  encoder is the first view. A later JSON view for a status page reads the same `Registry::snapshot()`,
  so a UI never re-derives numbers or opens a second source of truth. The registry type owns metric
  names, help text and label sets, so a view cannot drift from `/metrics`.
- **Hand-rolled, not a crate.** Counters and gauges are atomics, histograms are a fixed bucket array
  of atomics, and the text format is simple. This avoids a dependency on the serving path and keeps
  `cargo deny` and the binary small. Registration is static (a table in `metrics.rs`), so there is no
  runtime registry lookup on the hot path: a request holds `&'static` handles chosen by index.
- **Cardinality is the real risk.** Every label value comes from the loaded configuration snapshot or
  from a fixed enum. The label sets are fixed per snapshot: on reload the registry keeps series for
  removed routes until restart (counters must not reset), but never adds a series from request data.
  A request that matches nothing is counted under `route="none"`, not under what the client asked for.
- **No identity by default; scoped when opted in.** Keys are not a label. An operator may set
  `key_labels = true` to add a `key` label (the configured key id, never a hash or secret) to four
  counters only: `requests_total`, `tokens_total`, `cost_micro_usd_total` and `refusals_total`. Never
  to a histogram: buckets multiply with routes, targets and keys, and per-key latency is in the
  ledger. Only keys defined in configuration can be labels, and the registry caps distinct key
  labels (`key_label_limit`, default 100); a key past the cap is counted under `key="other"`, so a
  future database-managed key set cannot make the label unbounded. Off by default because a scrape
  endpoint may be reachable by more people than the admin.
- **Where it is served.** `[metrics] enabled = false` by default. The recommended setup is a separate
  listener (`listen`): `/metrics` is then unreachable from the public port and any ingress that
  routes to it, by construction, and access is a network matter (a NetworkPolicy that admits only the
  monitoring namespace; `docs/metrics.md` ships a ServiceMonitor and a NetworkPolicy example). No
  credential is needed there. Without `listen`, `/metrics` is served on the main listener and a
  `bearer_sha256` is required, or startup is refused, because an open `/metrics` on a public port
  must not be possible by omission. The credential is a hash in config like keys (ADR 0004) and is
  distinct from every virtual key; it authorizes `GET /metrics` and nothing else.
- **Latency buckets.** Request duration (to first byte and to completion), seconds, buckets from 50 ms
  to 120 s, because model calls are slow. Token and cost counters are monotonic integers (tokens;
  micro-USD per ADR 0006), so Prometheus `rate()` works and no float money is exposed.
- **Pull only.** No push gateway, no OTLP. Scraping is the model; a later change can add exporters.

- **Dashboards and alerts are examples, not features.** They ship as versioned files in
  `examples/metrics/` and as release assets, never embedded in the binary: they change faster than the
  gateway, belong to the operator's Grafana, and the binary must stay small. A test ties them to the
  registry so a renamed metric breaks the build instead of a dashboard. The built-in status page
  (later) is the answer for people with no Grafana, and it reads the same registry.

## Initial metric set

All names are prefixed `humpyard_`.

| Metric | Type | Labels |
|---|---|---|
| `requests_total` | counter | `protocol`, `route`, `target`, `provider`, `status_class`, `key`\* |
| `request_duration_seconds` | histogram | `route`, `target` |
| `time_to_first_byte_seconds` | histogram | `route`, `target` |
| `tokens_total` | counter | `route`, `target`, `direction` (`input`/`output`), `key`\* |
| `cost_micro_usd_total` | counter | `route`, `target`, `key`\* |
| `failovers_total` | counter | `route`, `from_target`, `to_target` |
| `refusals_total` | counter | `reason` (`auth`, `budget`, `policy`, `no_route`, `forbidden`), `key`\* |
| `endpoint_state` | gauge | `provider`, `endpoint`, `state` (`closed`/`open`/`probe`) |
| `ledger_queue_depth` | gauge | none |
| `ledger_dropped_total`, `ledger_failed_total` | counter | none |
| `config_reload_total` | counter | `result` (`ok`/`rejected`) |
| `config_generation` | gauge | none |
| `quota_percent`, `quota_snapshot_age_seconds` | gauge | `provider`, `window` |
| `build_info` | gauge (value 1) | `version` |

\* only when `key_labels = true`.

Names follow Prometheus conventions (`_total` for counters, `_seconds` for durations, base units).
No label is a request id, a model name as sent, or any unbounded value.

## Risks / trade-offs

- A metrics scrape that reveals route and target names exposes topology. Hence the credential and the
  separate-listener option; the metric list contains no key, tenant or content.
- Counters reset on restart, which Prometheus handles. The ledger stays the source of truth for cost.
- Hand-rolled exposition must match the text format exactly; a test parses the output with a strict
  reader and checks names, types and bucket monotonicity.
