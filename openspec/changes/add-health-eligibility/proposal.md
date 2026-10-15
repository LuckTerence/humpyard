# Proposal

## Why

Endpoint health (ADR 0009) already makes a dead endpoint cost one probe per cooldown instead of one
timeout per request, but only *inside* a target. When every endpoint of a target is cooling down, the
target is still tried on every request: the request spends its first attempt on a target the gateway
already knows is down, fails, and only then falls through to the next one. For a route with a healthy
alternative (a paid pool behind a free one, a second tier, a second private model) that is wasted
latency, wasted quota and a failure in the statistics on every request until the target recovers.

ADR 0009 deferred the fix: "a policy cannot know whether a route has a healthy alternative, so it could
turn slowness into 503". Strict routes (ADR 0017) put the missing piece in place: `plan_route` now
builds the route's target list per request and sees the whole route, so it can skip a cold target
*only when something else remains*.

## What Changes

- **Cold targets are skipped when an alternative exists.** A target whose endpoints are all cooling down
  is left out of the route's targets for a request, in the same planning step that applies classes and
  the routing policy, as long as at least one other target remains. The algorithm, its fallbacks and
  tier substitution then never spend an attempt on it.
- **Fail open.** If every eligible target is cold, none is skipped: the route is tried as today. Health
  still never denies service (invariant 14), it only reorders and skips.
- **Recovery is unchanged.** A target is cold only while a cooldown is running; when it ends the target
  is eligible again and the next request is its probe (ADR 0009). A success returns it to service.
- **Visible.** Decisions list the targets skipped for health (`cold_targets`, ledger schema v5);
  `explain` marks each target `cold` or `warm` and says when a route is failing open; a
  `humpyard_health_skips_total{route,target}` counter shows how often it happens.
- **Off switch.** `[health] skip_cold_targets` (default true) restores the previous behavior.
- **Out of scope:** `Retry-After` handling, a background prober, health that persists across restarts,
  weighting by latency or error rate (that belongs with cost-aware ordering), and any change to the
  per-endpoint breaker itself.

## Capabilities

### Modified Capabilities
- `routing-policy`: health-aware target eligibility that fails open.
- `metrics`: a skip counter.

## Impact

`src/server.rs` (`plan_route`: a second pass, a warm check over `TargetClient::health()`), `src/config`
(`health.skip_cold_targets`), `src/ledger` (schema v5, `cold_targets`), `src/server/decisions.rs`,
`src/server/explain.rs`, `src/metrics.rs`. No change to `src/health.rs` or the breaker. Docs: routing
(the "Not yet" note in Endpoint health goes away), configuration, api, invariants (14 is restated, not
weakened), architecture, roadmap, and the metrics dashboards and alert examples.
