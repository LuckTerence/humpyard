# ADR Review

## Reviewed

ADR 0017 (strict routes), ADR 0018 (health-driven eligibility) and ADR 0019 (capability eligibility) fix the
order of subtraction; ordering happens after it and cannot add a target back. ADR 0013 (decisions are
metadata only) is unaffected: the decision records target names and an ordering reason, never content.

## Outcome

Adds a new decision: ordering by observed speed is advisory (it ranks what is eligible) and never overrides
eligibility.

## New ADRs

- ADR 0020: Latency-ordered routes. Records the route type, the in-memory smoothed figure, exploration, the
  rule that only answered attempts count, and that ordering ranks eligible targets only.
