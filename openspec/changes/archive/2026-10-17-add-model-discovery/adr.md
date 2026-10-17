# ADR Review

## Reviewed

ADR 0014 (prices are proposed by a CLI and never fetched at runtime): the serving process still fetches
nothing here, the commands are CLI, and the serving-path test is extended to the new network module. The
decision departs from 0014 in one narrow way: the update is applied on a schedule without review.
ADR 0015 (quota polled at runtime, advisory) is the precedent for a network task near serving, and the reason
this change does not put one in `serve` yet: quota never changes routing membership, discovery would.
ADR 0021 (strengths) defines what the default strengths on generated targets may do. ADR 0007 (migration)
points users at this change for keeping pools current. ADR 0004 (no plaintext secrets) governs keys.

## Outcome

A new ADR is needed. ADR 0014 is not superseded: it still governs prices and everything else.

## New ADRs

- ADR 0022: Free-pool membership may be updated on a schedule without review. Records the scope, the two-source
  agreement, the generated-file mechanism, the fail-safe loader, the guardrails, and that running discovery
  inside `serve` is a later opt-in under the same rules. Also corrects ADR 0014's status to accepted.
