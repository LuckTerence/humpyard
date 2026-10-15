# Tasks

## 1. The auto pattern (configuration and docs; no code)

- [x] 1.1 Example rule set and `docs/routing.md` recipe: one `auto` route, intent rules (private tag, sub-agent, long prompt, tools), safe catch-all; verify each rule with `explain` tests
- [x] 1.2 Document client-side intent hints (static provider headers, one provider entry per intent) in docs/clients.md

## 2. Pool classes and opt-in strict fallback

- [x] 2.1 Make cross-class fallback visible first: flag it on decisions and in `explain` (no behavior change)
- [x] 2.2 Strict fallback (ADR 0017), in steps:
  - [x] 2.2.1 `allowed_classes` on every route type; load-time validation (valid labels, no unclassed target on a strict route, at least one allowed target); tests for each rejection and for the default being unchanged
  - [x] 2.2.2 Apply it in `plan_route` beside the routing policy so excluded targets never reach the algorithm; tests: strict route fails without calling the public target (mock upstream asserts zero calls), lenient route still falls back, tier substitution stays inside the allowed set, a cooling-down allowed target is still tried
  - [x] 2.2.3 Decisions: `excluded_targets` (ledger schema v4 with an in-place upgrade test), `GET /v1/decisions` output, `explain` per-target allowed or excluded with the reason
  - [x] 2.2.4 Metric `humpyard_fallback_blocked_total{route}`, the alert rule in `examples/metrics` and the dashboard panel, kept in step by the existing drift test
  - [x] 2.2.5 Docs (`routing.md` Target classes, `configuration.md`, `api.md`), the private-routes recipe in `examples`, architecture and roadmap

## 3. Session stickiness (investigate first)

- [ ] 3.1 Measure which clients send a usable session id and whether the existing per-session state can pin a conversation to a pool; decide whether it is a separate change

## 4. Classifier-backed rule action (only on a measured trigger)

- [ ] 4.1 Define the trigger from the decision log (catch-all share, default-pool cost per request) and revisit with real numbers
- [ ] 4.2 If triggered: proposal and ADR for `classify` with timeout and fallback pool; implementation and tests (slow, failing and inconclusive classifier all fall back)

## 5. Prerequisites for the demo and for cost-aware ordering

- [ ] 5.1 Endpoint prices in real deployments so cost per request is meaningful
- [ ] 5.2 Demo: a real Claude Code or Codex session through an auto route, with before/after cost and latency from `/v1/stats`
- [ ] 5.3 After weeks of statistics: a separate proposal for advisory cost-aware ordering inside the allowed set, with exploration
