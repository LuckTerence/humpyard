# Design

## Context

Selectors (ADR 0011) match request facts, first match wins, and clients can narrow but never widen.
Endpoint health (ADR 0012) already reacts to failures. ADR 0014 keeps *price* data out of the
serving process because wrong prices damage budgets; quota is different: it is live state that only
has value while serving, so it must be fetched at runtime, which needs its own bounds.

## Decisions

- **Rules consume quota; nothing acts on it silently.** A `quota` condition in `when` keeps the
  policy in the operator's hands, explainable with `explain`, and testable. The alternative, an
  automatic "avoid providers near their cap" behavior, would be a hidden policy with thresholds the
  operator did not choose.
- **Unknown is never a match.** Quota data can be missing (not configured, not yet polled, endpoint
  down, stale beyond `quota_stale_secs`). In every such case a quota condition is false, so traffic
  takes the rule's alternative or the catch-all. Quota can therefore never turn a working setup into
  a refusing one.
- **Poll off the request path.** One background task per quota-sourced provider, interval
  `quota_poll_secs` (default 60, 1 to 3600) with a short timeout, no retry storm (the next tick is
  the retry), and failures logged at most once per state change. Requests read the latest snapshot
  with no I/O.
- **Credential scope.** The poll uses the provider's configured API key, because that is what the
  provider's usage endpoint authorizes, but only ever to the origin of that provider's `base_url`
  over https (loopback http allowed for tests). The key is never logged, never in a snapshot, never
  sent to a price or other source.
- **Account-level honesty.** The snapshot is keyed by provider, not model, because that is what the
  source provides. Documentation says so, and the pacing recipe is written in those terms (shed
  load from the whole provider), not as per-model rotation.
- **Strict parsing.** `percent` must be a number from 0 to 100 (values above 100 are clamped and
  flagged, since an over-cap account may report them); a window with no usable percent is dropped;
  unknown fields and unknown `status` strings are kept as text, never interpreted. A document with no
  usable window is a failed poll.
- **Reload.** A config reload keeps the latest snapshot of a provider whose source and origin did
  not change, so a reload does not blank quota-driven routing.
- **Measure first.** Step one ships only the snapshot, `/v1/quota` and the poller. The percent is
  compared with the cost the ledger records (endpoints are priced) over a few days before the
  condition is used in production.

## Open questions (to answer by measurement)

- What is the percent a percent of: the plan's own cap, the sum of the per-model caps, or the
  busiest model? Compare deltas of the percent with ledger cost.
- Which `status` values exist besides `ok`, and what does a request get at the cap (429, a specific
  error)? Observe when a window fills.
- Does `rolling` reset exactly 5 hours after the oldest counted usage? Its `resetsAt` moves while the
  percent is 0, so it is not a guaranteed boundary.

## Risks

- A wrong or misunderstood percent could shift traffic needlessly. Mitigated by measuring first,
  fail-open semantics, and rules being explicit and explainable.
- A new outbound call from the serving process. Mitigated by ADR 0015's bounds and a test that a
  hung or failing source never delays or fails a request.
