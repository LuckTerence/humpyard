# ADR Review

## Reviewed

ADR 0018 (health-driven eligibility) defines the breaker, its cooldown and that a cooling target is left out of
a plan while another remains. This change only changes how long an endpoint cools after a failure that states a
retry time; eligibility, ordering and fail-open behavior are untouched. ADR 0013 (decisions are metadata only)
is unaffected: only a duration is kept, never body text.

## Outcome

Amends ADR 0018's cooldown rule, with the amendment recorded in the ADR rather than a new one.

## New ADRs

None; ADR 0018 gains an "Amended" note describing stated retry times.
