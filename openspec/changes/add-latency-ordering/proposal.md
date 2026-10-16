# Proposal

## Why

The free pools that ModelRelay backs are "the fastest free model with enough context". humpyard can already
skip targets that are failing (health, ADR 0018) and ones that cannot serve a request (capabilities,
ADR 0019), but a route still tries its targets in the order the operator wrote, so the first healthy one
serves every request however slow it is. Free endpoints vary in speed from hour to hour, so a fixed order is
the wrong answer for them, and a hand-maintained order cannot follow it. Step 2 of "Replacing ModelRelay".

## What Changes

- **A new route type, `fastest`**, with `targets` like a passthrough route. For each request it orders the
  targets that are still eligible by their recently observed time to answer, fastest first, and the request
  walks that order with the usual fallback.
- **Observed time is kept in memory**, one smoothed figure per target, updated from every attempt the gateway
  already times (no background prober, nothing sent that a client did not ask for). A target with too few
  samples counts as unmeasured and is tried early so it gets measured.
- **Exploration** so the route does not lock onto one target: with probability `explore` (default 0.1) a
  request tries a random eligible target first instead of the fastest.
- **Failures are not speed.** Only answered attempts update a target's figure; a failed attempt does not make
  a target look fast, and health skipping already removes a failing target.
- **The decision log records the order** the route used and why the first target was chosen (`fastest` or
  `explore`), and `route/explain` and `/v1/stats` show each target's current figure.
- **Eligibility is unchanged and first**: class, policy, capabilities and health still remove targets before
  ordering; the order only ranks what remains. A route cannot be made to try an ineligible target by being
  fast.
- **Out of scope:** ordering by price, quality or a blend (the cost half of "cost-aware ordering"),
  per-session stickiness, probing endpoints in the background, and persisting figures across restarts
  (figures rebuild from traffic within a few requests).

## Capabilities

### Modified Capabilities
- `route-selectors`: the `fastest` route type and its ordering.

## Impact

`src/config/{schema,validate}.rs` (route type), `src/routing.rs` (ordering at plan time), a small latency
tracker (`src/latency.rs`), `src/pool.rs` (report answered attempts), `src/server.rs` and `src/server/explain.rs`
(decision fields, explain), `src/metrics.rs` (one gauge). Docs: `routing.md`, `configuration.md`,
`metrics.md`. Ledger: a decision column for the ordering (schema v8).
