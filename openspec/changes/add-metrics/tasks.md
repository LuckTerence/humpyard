# Tasks

## 1. Registry
- [ ] 1.1 `src/metrics.rs`: counters, gauges, fixed-bucket histograms as atomics; static metric table; `snapshot()`; text encoder; strict-parser test for format, TYPE/HELP, cumulative buckets; add the module to the layer rules in `tests/architecture.rs`
- [ ] 1.2 Cardinality guard: label values only from config or fixed enums; a test that sends hostile model names, headers and tags and asserts none appears in the output

## 2. Feed it
- [ ] 2.1 Requests, duration, time to first byte, tokens, cost at the point the decision and usage are recorded; refusals by reason; failovers
- [ ] 2.2 Gauges sampled at scrape time: endpoint breaker state, ledger queue depth, drops and failures, config generation and reload results, quota percent and snapshot age, build info
- [ ] 2.3 Prove no request-path blocking: a concurrency test with a scraper running

## 3. Serve it
- [ ] 3.1 Config: `[metrics] enabled`, `listen`, `bearer_sha256`, `key_labels`; validation (main listener requires the credential; the credential is not a virtual key)
- [ ] 3.2 Route, auth and the separate listener; tests (disabled 404, client key 401, metrics credential cannot call inference, scrape works, hot reload keeps counters)

## 4. Docs
- [ ] 4.1 `docs/metrics.md` (metric list, label rules, scrape config, example alert rules and a Grafana-importable dashboard JSON); update configuration, api, architecture (status table, "Last updated") and the roadmap; ADR 0016 accepted

## 5. Later, separate proposals
- [ ] 5.1 A read-only status page served by the binary, reading `Registry::snapshot()` through a JSON view (the audience that does not run Prometheus)
- [ ] 5.2 Exporters (OTLP) if asked for
