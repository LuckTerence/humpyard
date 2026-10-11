# Design

- **A separate table, not more columns on `usage`.** `usage` is one row per upstream call and the
  budgeting source of truth; a decision is one row per request and may have zero upstream calls
  (refusals) or several (fallbacks). They join on `request_id` (a new nullable column on `usage`).
- **Same writer, same guarantees.** Decisions travel the existing bounded channel, are batched,
  counted when dropped or failed, and flushed on shutdown (invariant 7 extends to them).
- **Attempts are collected where they happen.** The pool already walks endpoints and the metered
  client wraps each target call; both report an `Attempt` to the request's context through a sink,
  so success and failure are captured without changing Switchyard's types. Failures after a stream
  has begun are not attempts (nothing fails over then).
- **A draft built along the request path.** `infer` creates a decision draft with the request id,
  `handle` fills it stage by stage (route, rule, budget, targets, features), and `infer` completes
  and records it on every exit path, including early refusals.
- **Per-key visibility.** With no admin credential, the safe read model is "your own decisions".
  Open gateways return everything. An admin view arrives with the admin API.
- **Retention in the writer.** Pruning runs inside the writer task at most hourly; one extra
  `DELETE` with an index on `ts_ms`.
- **Schema migration.** `PRAGMA user_version` 1 to 2: create `decisions`, add `usage.request_id`
  (nullable; old rows stay null). Opening an old database upgrades it in place.
- **Privacy.** Only routing metadata is stored; the error message is truncated to 200 characters
  and comes from the gateway's own sanitized errors.
- **Cost.** One extra small insert per request, batched with usage writes.

## Risks / Trade-offs

- Tier choices made inside a Switchyard algorithm (stage router, classifier) are recorded as the
  target that served, not the algorithm's reasoning.
- SQLite on NFS is already a deployment risk for the ledger; decisions share it.
