# 0013. Routing decisions are recorded per request in the ledger database

- Status: accepted
- Date: 2026-10-10
- Supersedes: none
- Change: [add-decision-log](../../openspec/changes/archive/2026-10-10-add-decision-log)

## Context

The ledger holds one row per successful upstream call, which is what budgets need. Everything else
the router decides (rule matched, requested versus served model, failed attempts and fallbacks,
refusals, skipped rules) lived only in logs and response headers. Operating the gateway and tuning
routing rules needs that history to be durable and queryable.

## Decision

Each inference request produces one decision row in a `decisions` table of the ledger database,
written by the existing asynchronous writer, with a request id that also marks the request's usage
rows. A decision records routing metadata and the ordered upstream attempts, includes requests
refused before routing, and never contains prompts or responses. A key may read only its own
decisions through `GET /v1/decisions` (an open gateway reads all) until an admin credential exists.
Rows are pruned after `decision_retention_days`.

Alternatives considered: widening `usage` (it is per upstream call and drives budgets; refusals and
fallbacks do not fit); logs only (not queryable, not durable, no join with spend); an external
tracing system (a heavy dependency for a single-binary gateway; the data model here can be exported
later); storing prompts for replay (privacy cost, not needed for routing questions).

## Consequences

Routing is auditable after the fact and rules can be tuned from real traffic. The ledger database
grows by one small row per request, bounded by retention. Decisions share the ledger's durability
limits (a hard kill can lose queued rows). The read model must be revisited with the admin API.
