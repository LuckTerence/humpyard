# Tasks

## 1. Aggregation

- [ ] 1.1 Ledger queries and nearest-rank percentiles for route, target and endpoint groups over a window; unit tests (percentile edge cases, outcome mix, fallback rate)
- [ ] 1.2 Cost and token averages joined from usage rows by request id

## 2. API

- [ ] 2.1 `GET /v1/stats` with `window`, `group_by`, filters, per-key isolation, row cap and truncation flag
- [ ] 2.2 `explain` annotation per target (failure rate, p50)
- [ ] 2.3 End-to-end tests: failure mix, percentiles, isolation, empty window, explain annotation

## 3. Docs

- [ ] 3.1 README, docs/budgets.md (decision log section), docs/routing.md, architecture
