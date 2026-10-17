# ADR Review

## Reviewed

ADR 0004 (no plaintext secrets) governs the metrics credential: stored as a hash, never logged.
ADR 0005 and 0013 (ledger and decision record) stay the durable record; metrics are an in-memory
aggregate beside them and never replace them. ADR 0009 (endpoint health) is read, not changed.

## Outcome

Two commitments outlive this change: metric labels never carry client-controlled strings, and every
operator-facing view (Prometheus text now, a status page later) reads one registry. That is ADR 0016.

## New ADRs

- [0016](../../../docs/adr/0016-metrics-are-one-registry-with-bounded-labels.md): Metrics are one registry with bounded labels.
