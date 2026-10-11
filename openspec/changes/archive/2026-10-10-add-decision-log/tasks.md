# Tasks

## 1. Storage

- [x] 1.1 Schema v2 migration (decisions table, usage.request_id), `Decision` type, writer support and flush, pruning with `decision_retention_days`; tests incl. upgrading a v1 database
- [x] 1.2 `decision_retention_days` config (default 90, validated, restart-only)

## 2. Collection

- [x] 2.1 Request id and `x-humpyard-request-id`; request id on usage rows
- [x] 2.2 Attempts from the pool and metered client; skipped rules from the selector; draft completed on every exit path including refusals

## 3. Reading

- [x] 3.1 `GET /v1/decisions` with filters, limit and per-key isolation
- [x] 3.2 End-to-end tests: success, fallback, refusals, join with usage, isolation, no effect when the DB fails, shutdown flush

## 4. Docs

- [x] 4.1 ADR 0013, README, docs/budgets.md (ledger), docs/routing.md, architecture, invariants
