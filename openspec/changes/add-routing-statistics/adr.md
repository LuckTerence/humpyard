# ADR Review

## Reviewed

ADR 0013 (decisions recorded per request) supplies the data and the per-key read model. ADR 0005
(SQLite ledger) is the store. Nothing is superseded.

## Outcome

No new durable decision: this is a read-only view over existing records. The later change that
acts on statistics (cost-aware choice) will need its own ADR.

## New ADRs

None.
