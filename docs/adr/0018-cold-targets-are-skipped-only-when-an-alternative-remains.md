# 0018. Cold targets are skipped only when an alternative remains

- Status: accepted
- Date: 2026-10-15
- Supersedes: none
- Change: `openspec/changes/add-health-eligibility`

## Context

ADR 0009 gave each endpoint a circuit breaker in the pool and decided that a target whose endpoints are
all cooling down is still tried, so health never denies service. It listed, and deferred, a
health-aware routing policy that makes cold targets ineligible, because a policy cannot know whether
the route has a healthy alternative and so could turn slowness into a 503. The cost of the deferral is
a wasted first attempt, a wasted quota unit and a recorded failure on every request to a route whose
first target is down, for as long as it is down.

Since ADR 0017 the planning step (`plan_route`) builds the route's target list per request and sees
the whole route, which is the missing information.

## Decision

In the planning step, after the route's allowed classes and the routing policy have narrowed the
targets, a target whose endpoints are all cooling down is left out of the request's plan, but only if
at least one other target remains. If skipping would leave nothing, nothing is skipped and the route is
tried as before (fail open). The check is two planning passes, the second without the warm check.
`probing` counts as warm, so a cooled target is probed and recovers exactly as ADR 0009 describes.
Skipping only removes targets and never adds one, so it cannot widen a strict route. It is on by
default and `[health] skip_cold_targets = false` restores the previous behavior.

Alternatives considered: a health-aware `RoutingPolicy` (the ADR 0009 shape: it has no route topology
and could not tell whether an alternative exists); skipping at fallback time (a cold target still gets
the first attempt in the algorithm's order); a background prober (spends tokens and quota on idle
providers); weighting targets by observed latency or error rate (a different, statistics-driven
change that belongs with cost-aware ordering).

## Consequences

Requests stop spending an attempt on a known-down target when a healthy one exists, and the failure
statistics stop counting it. A route's availability is still the union of its targets', and health
still never refuses a request, which keeps invariant 14 true as written. Load concentrates on the
remaining targets for the length of a cooldown. Decisions gain a stored `cold_targets` field (ledger
schema v5) holding configuration names only.

## Amended 2026-10-16: stated retry times

A failure that states when the endpoint will serve again (a 429 or 503 whose error says to retry after some
seconds) cools the endpoint for exactly that time, at once, clamped to between 1 second and
`[health] max_retry_after_secs` (default 300). It does not count as a trip, so a later unannounced failure
streak is not escalated by it, and it is not held to `max_cooldown_secs`, which bounds guesses. The time is
read from the error body for a small set of well-known phrasings and fields until Switchyard exposes the
`Retry-After` header on its error (NVIDIA-NeMo/Switchyard#904), after which the header takes precedence.
Eligibility, ordering and the fail-open rule are unchanged. Decisions record the honored time on the
attempt (`retry_after_ms`, inside the existing attempts column, so no schema change).
