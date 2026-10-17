# ADR Review

## Reviewed

ADR 0006 (money as integer micro-USD, budgets fail closed) and ADR 0008 (routing policy seam)
are unchanged; tiers make the policy's "highest price" rule more important, not different.

## Outcome

One new decision is load-bearing: prices are *proposed* by a CLI and never fetched or applied by the
serving process. It is recorded as ADR 0014.

## New ADRs

- [0014](../../../docs/adr/0014-prices-are-proposed-never-fetched-at-runtime.md): Prices are proposed by a CLI, never fetched at runtime.
