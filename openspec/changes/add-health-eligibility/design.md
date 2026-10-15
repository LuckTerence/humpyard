# Design

## Context

Three layers already exist. The pool owns a circuit breaker per endpoint (ADR 0009) and tries an
endpoint's target even when all its endpoints are cooling down. The routing policy narrows targets per
request, with tier substitution when a tier empties (ADR 0008). `plan_route` applies the policy and,
since ADR 0017, the route's `allowed_classes`. What is missing is a *preference* that uses health
without ever being able to refuse a request.

## Decisions

- **Skip in planning, with a fail-open second pass.** `plan_route` first plans with an eligibility
  predicate of `class allows && policy eligible && target is warm`. If that yields no plan (every
  remaining target is cold, or a budget or class rule already left only cold ones), it plans again
  without the warm check. A request therefore can never be refused because of health, and the "all
  cold" case behaves exactly as before. Doing it as two passes keeps `Route::plan` unchanged and
  puts the rule where the route's whole topology is visible.
- **"Cold" is exact and cheap.** A target is cold when it has at least one endpoint and every
  endpoint's breaker state is `cooling_down`. `probing` counts as warm: the cooldown has ended and the
  next request is the probe that lets the target recover. A target that is cold and a target whose
  probe is already in flight are treated the same as today by the pool.
- **Skipping happens after classes and policy, never before.** A strict route (ADR 0017) skips only
  within its allowed targets; if its single allowed target is cold it is kept (fail open), and no
  excluded target is ever added back. Budget narrowing is unchanged.
- **Tier substitution does the rest.** When a tier's targets are all cold the existing substitution
  serves it from the other tier. That matches what already happens after the cold tier's attempt
  fails, minus the wasted attempt. The substitution is logged as it is today.
- **Per-request state, no new state.** The cold check reads the existing breaker snapshots at planning
  time. Nothing new is stored, persisted or probed, and a restart starts everything warm, as before.
- **Recording.** The decision gains `cold_targets` (names from configuration, like
  `excluded_targets`), stored as a nullable JSON column in schema v5 so old rows read as none.
  `explain` reuses the endpoint health it already shows and adds per-target `cold` and a route-level
  `health_fail_open`. The metric counts requests that skipped at least one target, per route and
  target, with labels from configuration only (ADR 0016).
- **A switch.** `[health] skip_cold_targets` defaults to true because the change only ever reorders and
  skips, but an operator who wants the old "always try in order" behavior can have it.

## Risks / trade-offs

- A cold target that recovers is only noticed by its probe; with skipping, only requests that reach it
  as a probe test it, so recovery may take a request longer than when every request tried it. The
  cooldown already bounds this, and the probe is guaranteed once the cooldown ends.
- Skipping concentrates load on the remaining targets for the length of a cooldown. That is the point,
  but a route with one weak alternative can saturate it; the failover counter and error ratio show it.
- A route with `fallback_on` limited to some failure kinds can still fail on an overflow; skipping does
  not change which failures move to the next target, only which targets are in the plan.
