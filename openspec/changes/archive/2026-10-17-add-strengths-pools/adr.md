# ADR Review

## Reviewed

ADR 0017 (strict routes), 0018 (health), 0019 (capabilities) and 0020 (latency ordering) fix what is removed
per request and how the remainder is ordered. Pool membership is decided before any of them, at load, and
only produces the list they then act on. ADR 0014 (public data is only ever a suggestion) is respected: no
strength is fetched or assigned automatically.

## Outcome

Adds a decision: strengths are soft, build pools at load time, and never remove a target per request.

## New ADRs

- ADR 0021: Strengths build pools at load time and never remove a target. Records the field, the pool form,
  resolution at load, the rule for `min_context_window`, and the separation from capabilities and request
  tags.
