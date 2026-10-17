# ADR Review

## Reviewed

ADR 0020 (latency ordering ranks what eligibility leaves) is extended, not changed: quality only reorders the
eligible set. ADR 0021 (strengths build pools at load) is extended with a second load-time filter, in the same
way `min_context_window` is. ADR 0014 and 0022 keep third-party data out of the serving path: quality is a
declared number. ADR 0019 (capabilities) is the model for "declared, enforced on certainty": here nothing is
enforced on a guess, because an unknown quality never excludes a target from a route, only from a floor the
operator set.

## Outcome

No decision is superseded. The choices are recorded in the design and in a short addition to ADR 0020.

## New ADRs

None; ADR 0020 gains an "Amended" note describing the quality weight.
