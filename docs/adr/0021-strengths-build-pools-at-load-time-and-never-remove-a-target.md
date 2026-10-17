# 0021. Strengths build pools at load time and never remove a target

- Status: accepted
- Date: 2026-10-16
- Supersedes: none
- Change: `openspec/changes/add-strengths-pools`

## Context

A route names its targets. The free pools of the reference deployment are described by a property ("the
fastest free general model with at least 32,000 tokens of context"), so listing members by hand means every
new free model has to be added to every route that should use it. Capabilities (ADR 0019) are hard facts
that remove a target from a request only on certainty. What a model is good at is a judgment, and a wrong
judgment must not refuse a request.

## Decision

A target may declare `strengths`: up to 8 free-form lowercase labels (1 to 32 characters of letters, digits,
`-` and `_`). A `passthrough`, `fastest` or `random` route may take
`pool = { strength = "...", min_context_window = N }` instead of `targets`. Its targets are every target
that carries the strength and, when a window is given, has at least one endpoint that declares a
`context_window` of at least `N`, resolved when the configuration loads or reloads and written into the
route's target list in target-name order. After that a pool is an ordinary route, so class, policy,
capability and health removal and latency ordering act on it unchanged. Strengths remove nothing per
request. An undeclared window does not qualify for `min_context_window` (this is the operator asking which
targets are known to be big enough, the opposite question to request-time removal, which acts only on
certainty); explain lists targets left out and why. A pool that resolves to nothing is a configuration
error naming the strengths that exist; a reload that changes a pool's membership logs it.

Alternatives considered: matching strengths per request (membership would be non-deterministic, harder to
explain and costs work on every call); a fixed vocabulary of strengths (blocks operators' own labels and
buys little, since typos are caught by the empty-pool error); strengths on endpoints (they describe the
model a route chooses, not a deployment's limits); treating an undeclared window as qualifying (it admits
targets whose size is unknown into a pool defined by size); combining strengths with and/or in the first
version (the reference use is a single label, and combinations can be added without breaking this form);
fetching strengths from public data automatically (ADR 0014: public data is only ever a suggestion).

## Consequences

A free pool is a property statement instead of a list, and a new free model joins it by being labelled. A
wrong label costs a slightly different pool, visible in explain and fixed by one line, and never a refused
request. Membership is fixed per configuration, so it is deterministic and explainable, and it changes only
when the file does (a reload logs the change). A target whose context window is not declared is absent from
size-limited pools until it is, which is stated rather than silent.
