# Proposal

## Why

A free pool is full of models that are gone: removed by the provider, or not enabled for the key. Such an
endpoint answers 404 (or 401 for a rejected key). Today humpyard treats that as a client error: it ends the
request, and it does not count against the endpoint's health. In a `fastest` pool, which tries unmeasured
targets first, a target that always answers 404 is never measured and never cools, so it stays first and
fails every request. A throwaway test showed it: with a dead first target, 404 and 401 failed all six requests
and 503 was served by the healthy target. ModelRelay simply tries the next model. A migrated pool of 53 free
targets, of which most were `down` when checked, is not safe to use until this is fixed.

## What Changes

- **Endpoint health counts the answers that describe the endpoint.** A 401, 403 or 404 from an endpoint (the
  model is gone, or the key is rejected) is a failure of that endpoint, counted like a 5xx: after
  `failure_threshold` in a row it cools down, is skipped while another target remains, and is probed again
  after the cooldown. A 400 or 422 still describes the request, not the endpoint, and counts for nothing.
- **A 400 that says the model is gone is treated as 404.** Some providers answer 400 with a model-not-found
  code; a narrow, bounded check of the error code (`model_not_found`, `model_not_available`,
  `model_decommissioned`) makes that count too.
- **Failover inside a target** (its endpoints are alternatives of the same model) moves on after a 401 or 404,
  for every route.
- **A new fallback class, `unavailable`** (401 and 404), moves a request on to the next target. It is **on by
  default for `fastest` routes and pool routes**, and **off by default for every other route**, so existing
  routes behave exactly as before unless they list `unavailable` in `fallback_on`. An explicit `fallback_on`
  list is always taken as written. 403 keeps its existing `forbidden` class.
- **Visible**: attempts record the outcome `unavailable`, `explain` shows the class among a route's fallback
  kinds, and the docs say which statuses are which.
- **Out of scope:** probing models in the background, deleting targets from a configuration, changing 400/422
  handling, and any change to how 429, 5xx, timeouts or connection errors work.

## Capabilities

### Modified Capabilities
- `routing-policy`: endpoint health counts 401, 403 and 404.
- `upstream-proxy`: the `unavailable` fallback class and when it applies by default.

## Impact

`src/pool.rs` (what counts as an endpoint failure, within-target failover), `src/error.rs` (classification),
`src/config/schema.rs` (`FallbackClass::Unavailable`, defaults by route type), `src/server/explain.rs`
(fallback kinds). Docs: `routing.md`, `migrating-from-litellm.md` (the parity statement), `migrating-from-modelrelay.md`.
No ledger, config-file or API change; no new setting.
