# 0020. Latency-ordered routes rank only what eligibility leaves

- Status: accepted
- Date: 2026-10-16
- Supersedes: none
- Change: `openspec/changes/add-latency-ordering`

## Context

A route tries its targets in the order written, or by weight. For free endpoints, whose speed varies from
hour to hour, a fixed order is the wrong answer, and the first healthy target serves every request however
slow it is. The gateway already times every attempt. Removal (classes and policy, ADR 0017; capability,
ADR 0019; health, ADR 0018) is subtract-only and fixed in order; whatever orders the targets must not undo
it.

## Decision

A route of type `fastest` takes `targets` like a passthrough route. For each request, after removal, it
orders the targets that remain by a smoothed time to answer, fastest first, and the request walks that
order with the route's usual fallback. Targets with fewer than `min_samples` answered attempts, or whose
newest sample is older than `max_age_secs`, count as unmeasured and go first in written order so they get
measured. With probability `explore` a request tries a random remaining target first, drawn from a
generator that `seed` makes reproducible. The figure is one exponentially smoothed value per target (weight
0.3 on the newest sample), kept in memory and updated only from answered attempts; it is the time until the
upstream answered, which is the full time for a buffered call and the time until the response began for a
streamed one. The order and why its first target was first (`fastest` or `explore`) are recorded on the
decision (schema v8); explain shows the order a request would use now without exploring, statistics and
`/v1/health` show each target's figure, and `humpyard_target_latency_seconds` exposes it.

Alternatives considered: a flag on `passthrough` (it would blur a route type that the docs promise is
deterministic); a background prober (spends tokens and quota on idle providers, and ADR 0018 already
rejected it); counting failures as slow samples (makes a dying target look merely slow, and health owns
failures); persisting figures in the ledger (not needed for correctness and adds a startup read; a restart
only costs a few exploratory requests); measuring the full stream (needs per-token normalisation and holds
the figure back until the stream ends); blending price or quality into the order (a different decision,
part of cost-aware ordering).

## Consequences

A free pool follows speed without anyone maintaining its order, and the cost is a small share of requests
that explore. A figure is a rough ranking: it mixes request sizes (a target that mostly gets large prompts
looks slower) and, for streamed calls, measures when the answer began, not when it ended. Speed says
nothing about quality, so a pool should hold targets of comparable capability. Ordering ranks the eligible
set and never adds a target back, so strict routes, capability removal and health skipping hold exactly as
before. Decisions gain a stored `ranking` field holding target names and a reason, never content.

## Amended 2026-10-17: an optional quality weight

A `fastest` route may take `quality_weight` (0 to 1, default 0), `latency_target_ms` (default 2000) and
`unknown_quality` (default 0.5). With a weight above 0, each measured target is ranked by
`(1 - w) * speed + w * quality`, with `speed = min(1, latency_target_ms / observed_ms)` and `quality` the
target's declared `quality` (a number from 0 to 1 the operator states; never fetched), ties by observed speed.
With the weight at 0 the order is exactly the speed order, so existing routes do not change. Unmeasured
targets still go first, exploration is unchanged, and ordering still only ranks what eligibility left. A pool
may also set `min_quality`, a load-time floor like `min_context_window` (an undeclared quality does not
qualify). `migrate-modelrelay` declares only ModelRelay's measured scores. The formula is a deliberate blend
an operator can read, not ModelRelay's product-plus-speed.

