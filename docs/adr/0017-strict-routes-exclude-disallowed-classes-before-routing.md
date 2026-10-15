# 0017. Strict routes exclude disallowed classes before routing

- Status: proposed
- Date: 2026-10-14
- Supersedes: none
- Change: `openspec/changes/add-auto-routing`

## Context

Targets carry an optional class (`private`, `public`, `paid`, or the operator's own). Until now a
route that lists a private target and then a public one will, when the private target fails, serve the
request from the public one. That is sometimes wanted and is the default, but for an operator whose
private route exists so a request stays on their own hardware, it is a silent leak. Class crossings
are already visible (ADR 0013 decisions, `explain`); this decision adds the enforcement.

Enforcement could happen at the moment of fallback (inspect each next target and refuse a disallowed
one) or before the algorithm runs (remove disallowed targets from the route). The first depends on
Switchyard's internal order of attempts and leaves every other code path (retries, tier substitution,
a future classifier) a chance to reach a disallowed target.

## Decision

A route may set `allowed_classes`. Targets of any other class, and unclassed targets, are removed from
the route's targets in the planning step, beside the routing policy (ADR 0008), before the algorithm
runs. A disallowed target is therefore never constructed for the request and cannot be called by any
failure path. Configuration is validated fail-closed: an unclassed target on a strict route, an
invalid class, or a route with no allowed target is rejected at load. Routes without `allowed_classes`
keep today's behavior. Strictness only removes targets: health still never denies service (ADR 0009),
and no selector, header or budget can add a target back (ADR 0011). When the allowed targets fail the
client gets their error, and the exclusion is recorded on the decision and counted in a metric.

Alternatives considered: a check at fallback time (depends on Switchyard internals, other paths
remain); a global fail-closed default (breaks deployments that want private-to-public degradation);
rewriting the upstream error into a gateway refusal (hides the real failure from the client for no
gain, since the decision and metric already carry the signal).

## Consequences

Privacy of a route becomes a property of its configuration that a test can prove (zero calls reach
the excluded target). Operators who want fail-closed set one line per route, and a mistake in
labelling fails the load instead of leaking. A strict route has less redundancy by design, so its
availability is its allowed targets' availability; the blocked-fallback metric makes that cost
visible. Decisions gain a stored `excluded_targets` field (ledger schema v4), which holds
configuration names only.
