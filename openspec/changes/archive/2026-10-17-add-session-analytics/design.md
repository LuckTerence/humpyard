# Design

## Context

Statistics are computed on read from the decision log and the usage rows (ADR 0013) and are scoped to the
caller's key. Usage rows already hold `session_id`, `target`, `provider`, `model`, `kind`, `ts_ms`, fresh
input tokens, cached input tokens, cache-creation tokens and cost per call. So session analytics need no
new stored data, only new queries; the one schema change is an index.

## Decisions

- **Prompt size and cache ratio are defined once.** A call's prompt is `input + cached_input +
  cache_creation` tokens (fresh input excludes the cached part; this is how the ledger already sums
  tokens). `cache_hit_ratio = cached_input / prompt`, or `null` for a group with no prompt tokens. A
  provider that reports no cache usage shows `0.0`, which is true for billing purposes; `usage_missing`
  rows are left out of the ratio so they do not pull it down.
- **Sessions come from usage rows, not decisions.** Usage rows outlive decision retention and carry cost
  and cache numbers, so `group_by=session` and the switch report read `usage` (answered calls only,
  `kind = 'answer'`, ordered by time within a session). Calls with no session id are counted in
  `coverage`, never invented into a session.
- **A switch is a change of endpoint, not of target.** The cache lives with a provider and model, so two
  consecutive answered calls in a session on different `provider/model` endpoints are one switch, whether
  a route chose differently or a failover inside a target moved the call.
- **"Warm tokens abandoned" is what the previous call left cached.** For a switch from call *i* to call
  *i+1*, `warm_tokens_abandoned = cached_input(i) + cache_creation(i)`: the prefix the old endpoint held
  warm after that call, which the new endpoint must pay full input price for. It is a deliberate
  approximation (the true next prompt may be longer or share less), stated as such in the response.
- **The dollar estimate is priced from the config, at read time.** The ledger crate knows no prices (they
  live in configuration, ADR 0006/0014), so the server prices each switch at the destination endpoint's
  `price.input - price.cached_input` per million tokens, using the loaded configuration. An endpoint
  with no price contributes nothing and is counted as unpriced; the report says how many switches were
  priced. The estimate is in micro-USD, integer arithmetic only.
- **Stayed versus switched is descriptive.** Sessions that switch are probably longer and harder, so
  comparing their average cost and calls with sessions that stayed does not show that switching *caused*
  the difference. The response carries that caveat in a `note` field, and the docs say so.
- **Bounded.** Session queries scan at most `ROW_CAP` usage rows of the window (newest first) and report
  `truncated` as `/v1/stats` already does; `limit` (default 20, at most 100) bounds the rows returned and
  `order_by` is one of `cost`, `requests`, `switches`, `recent`.
- **Privacy.** A session id is an opaque client string the key's own requests already carry; stats return
  it only to that key, as `GET /v1/decisions` does today, clipped to the existing length limit. Nothing
  about content is read or stored. Session ids never become Prometheus labels (ADR 0016); the metric
  change adds two fixed `direction` values only.
- **Index only.** Schema v7 adds `usage_session_ts` on `(session_id, ts_ms)` where `session_id` is not null,
  an in-place, idempotent upgrade like the earlier ones.

## Risks / trade-offs

- A client that sends no session id gives no session view; `coverage` makes that visible rather than
  hiding it, and it is the first thing to look at.
- The abandoned-tokens figure overstates the loss when the new endpoint would have rebuilt less, and
  understates it when the prompt grew; it is an order-of-magnitude answer to "is this worth fixing", which
  is all the decision needs.
- Prices change over time; the estimate uses today's configured prices for past calls.
