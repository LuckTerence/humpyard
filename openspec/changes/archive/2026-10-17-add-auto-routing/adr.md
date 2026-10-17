# ADR Review

## Reviewed

ADR 0011 (selectors; clients narrow, never widen) is the foundation and is unchanged. ADR 0008
(routing policy seam) governs eligibility and still applies to the chosen route.

## Outcome

ADR 0009 (health never denies service) is unchanged: strictness removes targets, it never forces a
refusal because an allowed target is unhealthy. ADR 0016 (metric labels from configuration only)
governs the new metric.

## Outcome

No decision needs superseding. Fail-closed private routing is a durable commitment (a disallowed
class is never called, by construction, not by a check) and gets ADR 0017. A `classify` action
(step 2) would warrant its own ADR when built.

## New ADRs

- [0017](../../../docs/adr/0017-strict-routes-exclude-disallowed-classes-before-routing.md): Strict routes exclude disallowed classes before routing.
