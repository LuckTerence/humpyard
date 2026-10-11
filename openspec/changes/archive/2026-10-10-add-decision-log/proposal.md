# Proposal

## Why

The ledger records only successful upstream calls. Everything else the router decides is visible
only in logs and response headers, and is gone afterwards: which selector rule fired, what the
client asked for versus what served it, failed attempts and fallbacks, refusals (401, 402, 403,
404, 503), rules skipped because the key may not use their route, and request features. Debugging
a failover at 3am, or tuning routing rules (including the planned auto-routing), needs a durable
record of the decisions, not just the spend.

## What Changes

- Every request gets a **request id**, returned as `x-humpyard-request-id` and stored on its usage
  rows, so a decision and its upstream calls can be joined.
- A new **`decisions` table**, one row per request, written by the same asynchronous writer as the
  ledger (never delaying a response; flushed on shutdown): time, key, session, protocol, requested
  model, selected route, rule and its source, rules skipped for the key's allowlist, budget state,
  eligible targets, request features (estimated size, tools, images, stream), the ordered
  **attempts** (target, provider, model, outcome, milliseconds), the final status and outcome, the
  serving target and latency.
- **Refusals are decisions too**: unauthenticated, forbidden, unknown model, budget exhausted, no
  eligible target.
- **`GET /v1/decisions`** with filters (`limit`, `since_ms`, `route`, `rule`, `outcome`, `request_id`);
  a key sees only its own decisions (an open gateway sees all); there is no admin credential yet.
- **Metadata only, with retention**: no prompts or responses are stored, error text is truncated,
  and `decision_retention_days` (default 90, 0 = keep forever) prunes old rows.
- Docs and README updated.

Out of scope: why a stage router or classifier chose a tier (Switchyard does not expose it), an
admin view across keys, exporting to external systems, per-conversation stickiness.

## Capabilities

### New Capabilities
- `decision-log`: a durable, queryable record of routing decisions.

### Modified Capabilities

## Impact

`src/ledger.rs` (schema v2, decisions, pruning), `src/pool.rs` and `src/metering.rs` (attempt
collection), `src/server.rs` and a new `src/server/decisions.rs`, `src/select.rs` (skipped rules),
config (`decision_retention_days`), tests, docs, README, ADR 0013.
