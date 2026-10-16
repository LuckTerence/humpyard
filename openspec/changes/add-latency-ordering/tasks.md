# Tasks

## 1. Tracker and route type
- [x] 1.1 `src/latency.rs`: smoothed figure and count per target, staleness, thread-safe, injectable clock; unit tests for smoothing, `min_samples`, staleness and that failures do not update
- [x] 1.2 Config: the `fastest` route type with validation (probabilities, positive values, known targets, strict-route class rules) and tests
- [x] 1.3 `src/routing.rs`: order eligible targets at plan time (unmeasured first, then fastest; seeded exploration); tests for each scenario including that eligibility is applied first

## 2. Wiring and visibility
- [x] 2.1 `src/pool.rs` reports answered attempts to the tracker; the decision records the order and the reason (schema v8, idempotent upgrade built offline)
- [x] 2.2 `route/explain` and `/v1/stats` show figures and counts; a metrics gauge `humpyard_target_latency_seconds{target}` with docs and the dashboard drift test
- [x] 2.3 End to end: a route over two mock upstreams of different speed converges on the faster one, keeps exploring at the configured rate, and routes around a target that fails

## 3. Docs and decisions
- [x] 3.1 ADR 0020; `routing.md`, `configuration.md`, `metrics.md`, `architecture.md`; the roadmap marks step 2 shipped
