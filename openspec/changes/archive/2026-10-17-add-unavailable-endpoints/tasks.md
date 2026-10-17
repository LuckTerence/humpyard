# Tasks

## 1. Classification and health
- [x] 1.1 `src/error.rs`/`src/config/schema.rs`: `FallbackClass::Unavailable` (401, 404, and a 400/404 with a model-gone code), its name `unavailable`, the model-gone body check (bounded, first 4 KiB); unit tests for each status and code, and for a 400 without a code
- [x] 1.2 `src/pool.rs`: 401/403/404 and model-gone answers count against the breaker; 401/404 fail over within a target; 400/422 unchanged; tests for the breaker count, the reset on success and the unchanged request errors
- [x] 1.3 Defaults by route type: `unavailable` included by default for `fastest` and pool routes only; an explicit `fallback_on` is exact; tests for each route type

## 2. End to end and reports
- [x] 2.1 The probe, as permanent tests: a `fastest` pool, a pool, a `passthrough` route and an opt-in route, each with a first target answering 404, 401, 403, 400 and 503; the dead target cools after the threshold and is probed after the cooldown; every-target-404 returns the last 404; a strict route does not cross classes
- [x] 2.2 Attempts record the outcome `unavailable`; `explain` lists the class in a route's fallback kinds; the metrics label sets and the dashboard drift test still hold

## 3. Docs and decisions
- [x] 3.1 ADR 0023; `routing.md` (which statuses are which), `migrating-from-litellm.md` (the parity statement now has two sides), `migrating-from-modelrelay.md` (why dead models no longer break a pool), `invariants.md`
