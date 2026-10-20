# ADR Review

## Reviewed

ADR 0023 (answers that describe the endpoint) and ADR 0024 (the guarded tail, `overflow` by default). A 413 differs
from the endpoint-describing statuses: it describes the request relative to one endpoint, so it must not count
against health, but it should move on like an overflow. ADR 0024's default guard then sends such a request to the
paid tail, which is the intended use of that guard.

## Outcome

No new ADR; ADR 0023 is amended with the 413 and the reasoning (done in this change).
