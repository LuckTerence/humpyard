# Tasks

## 1. Source adapters and comparison (no config change yet)

- [ ] 1.1 Normalized price record and a pure comparison of records with a config's endpoints (match, differs, unpriced, missing at source); tests
- [ ] 1.2 models.dev adapter: strict parsing of recorded fixtures (flat, cache-read, tiered, malformed, free), sanity ceiling, HTTPS-only fetch with timeout and size cap; tests without network
- [ ] 1.3 `prices import`: TOML snippet and diff output with provenance; never writes the config; tests
- [ ] 1.4 `prices check`: human and `--json` output, exit codes 0/1/2; tests

## 2. Config

- [ ] 2.1 Provider `price_source` and validation; unknown source is a config error
- [ ] 2.2 `price.tiers` in the schema and validation (ordered thresholds, non-negative); highest rate used by policy and budgets
- [ ] 2.3 Metering applies the tier by prompt tokens; tests on both sides of a threshold

## 3. Operations and docs

- [ ] 3.1 Scheduled drift check for the cluster (CronJob report only, in the private config repo)
- [ ] 3.2 Docs: configuration, budgets (what cost means on a flat-fee plan), deployment (running the check); example config
- [ ] 3.3 A test or invariant that `serve` makes no price-source requests

## 4. Later, separate proposals

- [ ] 4.1 Provider quota polling (for example remaining quota windows) feeding routing, once the importer exists
- [ ] 4.2 Additional adapters (OpenRouter models API, hand-maintained table)
