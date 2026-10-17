# Tasks

## 1. Registry
- [x] 1.1 `src/metrics.rs`: counters, gauges, fixed-bucket histograms as atomics; static metric table; `snapshot()`; text encoder; strict-parser test for format, TYPE/HELP, cumulative buckets; add the module to the layer rules in `tests/architecture.rs`
- [x] 1.2 Cardinality guard: label values only from config or fixed enums; a test that sends hostile model names, headers and tags and asserts none appears in the output

## 2. Feed it
- [x] 2.1 Requests, duration (to the response), tokens, cost at the point the decision and usage are recorded; refusals by reason; failovers
- [x] 2.2 Gauges sampled at scrape time: endpoint breaker state, ledger queue depth, drops and failures, config reload results and load time, quota percent and snapshot age, build info
- [x] 2.3 Prove no request-path blocking: a concurrency test with a scraper running

## 3. Serve it
- [x] 3.1 Config: `[metrics] enabled`, `listen`, `bearer_sha256`, `key_labels`, `key_label_limit`; validation (the main listener requires the credential; a separate listener does not; the credential is not a virtual key)
- [x] 3.2 Route, auth and the separate listener; tests (disabled 404, not on the main port when `listen` is set, client key 401, metrics credential cannot call inference, scrape works, hot reload keeps counters, key labels on four counters only and capped)

## 4. Docs
- [x] 4.0 Ship examples as files under `examples/metrics/`, not in the binary: `grafana-dashboard.json` (request rate, error ratio, latency percentiles, failovers, spend by route and key, breaker state, ledger backlog, quota), `alert-rules.yaml` (PrometheusRule form and a plain rules file), `servicemonitor.yaml`, `networkpolicy.yaml`; attach the dashboard and rules to each release; a test that every `humpyard_*` name used in these files exists in the registry table, so they cannot drift from the code
- [x] 4.1 `docs/metrics.md` (metric list, label rules, recommended separate-listener setup with a ServiceMonitor and a NetworkPolicy example, example alert rules for error ratio, failover rate, ledger drops, a breaker open too long, stale quota and a rejected reload, and a Grafana-importable dashboard JSON); update configuration, api, architecture (status table, "Last updated") and the roadmap; ADR 0016 accepted

## 5. Later, separate proposals
- [ ] 5.1 A read-only status page served by the binary, reading `Registry::snapshot()` through a JSON view (the audience that does not run Prometheus)
- [ ] 5.2 Exporters (OTLP) if asked for
