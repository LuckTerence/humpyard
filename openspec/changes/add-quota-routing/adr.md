# ADR Review

## Reviewed

ADR 0011 (selectors; clients narrow, never widen) is extended by a new condition and otherwise
unchanged. ADR 0012 (endpoint health) stays the reactive mechanism; quota is the proactive input.
ADR 0014 (prices are never fetched while serving) is not contradicted: prices are configuration
that budgets depend on, quota is advisory live state that is useless unless fetched while serving.

## Outcome

Serving gains one new, bounded outbound call. That is a decision worth recording: ADR 0015.

## New ADRs

- [0015](../../../docs/adr/0015-provider-quota-is-polled-advisory-and-fails-open.md): Provider quota is polled at runtime, advisory, and fails open.
