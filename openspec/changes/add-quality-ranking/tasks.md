# Tasks

## 1. Quality on targets and pools
- [x] 1.1 `quality` on targets and `min_quality` on pools: schema, range validation, pool resolution with the new left-out reasons; tests including undeclared and the empty-pool error
- [x] 1.2 `explain` shows each target's quality; pool report shows `quality_below` and `quality_undeclared`

## 2. The weighted order
- [x] 2.1 `quality_weight`, `latency_target_ms` and `unknown_quality` on `fastest` routes with validation; the blended ranking in `latency.rs`/`routing.rs` (weight 0 byte-for-byte today); unit tests for each scenario, ties, unknown quality and unmeasured-first
- [x] 2.2 End to end: a route over mock upstreams of different speed and declared quality picks as the formulas say; `explain` shows speed score, quality and score

## 3. Migration and docs
- [x] 3.1 `migrate-modelrelay` reads `intell` and `isEstimatedScore`; measured scores become `quality`, estimates become comments, `--estimated-quality` opts in; tests on the real-shaped fixtures
- [x] 3.2 `routing.md` (Fastest routes, Pools), `configuration.md`, `migrating-from-modelrelay.md` (how ModelRelay's ranking maps and what is not carried); the ADR 0020 amendment; the roadmap marks this gap closed
