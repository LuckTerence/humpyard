# ADR Review

## Reviewed

ADR 0008 (routing policy seam), ADR 0017 (strict routes) and ADR 0018 (health skipping) define the
planning step this extends; capability removal is a third subtract-only rule in the same place, with the
same promise that nothing is ever added back. ADR 0009 (health never denies service) is not affected:
capability is a declared fact, not a health signal, and refuses only when no target can serve the
request. ADR 0014 (prices are proposed, never fetched at runtime) governs the importer's capability
proposals. ADR 0016 governs the metric's labels.

## Outcome

Two commitments outlive this change: a target is removed for capability only on certainty (a declared
limit, or an estimate that errs low), and a request that no target can serve is refused with a named
reason before any upstream call. That is ADR 0019.

## New ADRs

- [0019](../../../docs/adr/0019-declared-capabilities-remove-targets-that-cannot-serve-a-request.md): Declared capabilities remove targets that cannot serve a request.
