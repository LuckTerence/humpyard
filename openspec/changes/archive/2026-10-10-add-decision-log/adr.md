# ADR Review

## Reviewed

ADR 0005 (SQLite ledger with an asynchronous writer) is extended, not changed: decisions use the
same writer and guarantees. ADR 0011 (selectors) supplies the rule and skip information. ADR 0003
(keys behind a KeyStore) shapes the per-key read model.

## Outcome

A durable decision about what is recorded, where, and who may read it.

## New ADRs

- 0013 Routing decisions are recorded per request in the ledger database.
