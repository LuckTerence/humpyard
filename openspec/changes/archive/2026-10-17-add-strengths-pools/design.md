# Design

## Context

A route names its targets; capabilities (ADR 0019) and health (ADR 0018) remove some of them per request.
Targets already carry an optional `class` (private, public, paid) used for strict routes. Free pools need
membership by property, not by list.

## Decisions

- **Strengths on the target, not the endpoint.** A target is the unit a route chooses; its endpoints are
  failover alternatives of one model. Capabilities stay on endpoints because they describe a specific
  deployment's limits; strengths describe the model's character and belong to what a route selects.
- **Resolved at load, not per request.** The membership of a pool is computed once per configuration, sorted
  by target name, and stored in the route's `targets` list. A pool route is then exactly the same as a route
  someone wrote out by hand, so every later stage (class and policy removal, capability, health, ordering)
  and the explain output work unchanged, and a reload that changes membership rebuilds the route through the
  existing "definition changed" check.
- **One strength per pool.** ModelRelay's tags are single labels and so is the reference deployment's use.
  Combinations (all of, any of) can be added later without breaking this form.
- **`min_context_window` counts only declared windows.** Request-time capability removal acts only on
  certainty, so an undeclared window never removes a target from a request. Pool membership is the opposite
  kind of question, "which targets are known to be big enough", so an endpoint with no declared window does
  not qualify. A target left out for that reason is reported by explain, so the cause of a missing member is
  visible and the fix is to declare the window. A target qualifies when at least one of its endpoints
  declares a window of at least the minimum, matching how capability removal treats a multi-endpoint target.
- **Strengths are free-form but validated.** 1 to 32 characters of lowercase letters, digits, `-` and `_`,
  at most 8 per target, no duplicates on a target. A fixed vocabulary would make the catalog's job easy and
  every operator's custom label impossible. A typo is caught the other way: a pool that resolves to nothing
  is a configuration error that lists the strengths that exist.
- **`pool` and `targets` are exclusive**, and `weights` cannot be combined with `pool` (the author cannot know
  which weight belongs to which member). Stage routers and classifier routes keep explicit tiers.
- **Strict routes still apply.** `allowed_classes` removes targets per request after the pool is resolved, so
  a pool of free targets on a strict private route simply has the public ones removed, as it would for a
  hand-written list.

## Risks / trade-offs

- A label applied by judgment can be wrong. Because strengths only build pools, the cost is a pool that
  contains a target it should not (or lacks one it should), visible in explain, and fixable by editing one
  line.
- A pool can silently shrink when a reload drops a strength from a target. Explain shows the membership, and
  the reload log lists pools whose membership changed.
