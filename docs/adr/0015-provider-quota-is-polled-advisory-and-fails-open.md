# 0015. Provider quota is polled at runtime, advisory, and fails open

- Status: accepted
- Date: 2026-10-13
- Supersedes: none
- Change: [add-quota-routing](../../openspec/changes/add-quota-routing)

## Context

Capped plans fail at a limit the gateway cannot see until a provider answers 429. Some providers
publish usage windows (percent used, reset time). ADR 0014 keeps price data out of the serving
process, but quota is live state with no value outside serving. The data is account-level and its
semantics are not fully documented.

## Decision

A provider may name a quota source. While serving, a background task polls it with the provider's
own key, only to that provider's origin over https, and keeps the latest snapshot. Nothing on the
request path waits for it. Quota is exposed (`/v1/quota`, `explain`, decisions) and consumed only by
an explicit selector condition an operator writes; unknown, stale or missing quota never matches, so
quota can change which rule applies but can never refuse a request or remove a target by itself.
Measurement against the real account comes before any production rule depends on it.

Alternatives considered: acting automatically near a cap (a hidden policy with thresholds the
operator did not choose, built on a number we do not yet understand); relying on circuit breakers
only (reactive, and cannot pace a quota); an external exporter feeding quota into config (more
moving parts and a reload for every change); per-model quota (the source does not provide it).

## Consequences

The serving process makes one more kind of outbound request, so it needs a timeout, scoped
credentials and a failure that is invisible to requests; tests keep that true. Operators can write
pacing rules and see exactly why they matched. Rules built on the account-level percent shed load
from a whole provider, not from one model.

## Addendum: history (step 2)

To judge what the percent measures, each poll that differs from the last stored one (and at least
one every 15 minutes) is written to the ledger as one sample per window, and
`GET /v1/quota/history` reads them back with the cost of the provider's calls between samples.
Samples hold numbers and short status text only, follow `decision_retention_days`, and need no new
configuration. Polling and routing are otherwise unchanged: the history is for people, and nothing
acts on it.
