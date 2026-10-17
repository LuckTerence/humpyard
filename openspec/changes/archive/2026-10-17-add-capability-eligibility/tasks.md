# Tasks

## 1. Declare and decide

- [x] 1.1 Config: `context_window`, `tools`, `vision` on `Endpoint` (optional; validation: window at least 1); every existing config loads unchanged; tests
- [x] 1.2 `estimate_tokens_low` (a lower bound, bytes/6) beside the existing estimate, a calibration test over text, code, JSON and non-Latin fixtures asserting it stays below a reference count, and the prompt estimate computed whenever any endpoint declares a window
- [x] 1.3 Capability removal in `plan_route` after classes and policy and before health: a target is removed when no endpoint can serve; the `no_capable_target` refusal (400) naming tools, images or context; tests with call-counting mock upstreams: tools, images, context, any-endpoint-is-enough, unknown means capable, borderline prompt stays, all-removed refuses before any upstream call, strict route not widened, health skipping runs after

## 2. See it

- [x] 2.1 Decisions: `incapable_targets` (ledger schema v6, nullable JSON, in-place upgrade test), `GET /v1/decisions`
- [x] 2.2 `explain`: per-target `capable` with `reason` and the estimate used
- [x] 2.3 Metric `humpyard_capability_skips_total{route,target,reason}`, a dashboard panel, kept in step by the example drift test

## 3. Propose and document

- [x] 3.1 `prices import` proposes `context_window`, `tools` and `vision` from models.dev; `prices check` reports drift; tests on recorded fixtures
- [x] 3.2 Docs: `configuration.md`, `routing.md`, `api.md`, price-import page, invariants (a new rule), architecture, roadmap; ADR 0019 accepted
- [x] 3.3 A calibration note and a report (or documented query) comparing the estimate with the provider's reported input tokens from the ledger, so the divisor can be tuned from data
