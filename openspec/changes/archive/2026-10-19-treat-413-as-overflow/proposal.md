# Proposal

## Why

The decision log on the deep public tier showed four requests of 35,000 to 77,000 tokens (all with tools) that
ended with `413 Request too large … tokens per minute` from a free plan's cap on `gpt-oss-120b`. A 413 had no
fallback class, so it never moved on to the next pool member or to the guarded paid tail, and the client got the
error even though other members, and the tail, could have taken the request. A 413 says the request is too large
for this endpoint (a per-minute token cap, a body limit), which is what the `overflow` class means.

## What Changes

- An upstream `413` carries the `overflow` fallback class: with the route's guard it moves to the next endpoint of
  the target, the next pool member, and the tail when the guard lists `overflow` (the default).
- A 413 does not count against the endpoint's health: the endpoint is fine for smaller requests, so it is asked
  again next time and never cools because of it.
- When no target takes the request, the client receives the real 413 and the provider's message.
- Out of scope: reading the provider's stated limit to avoid sending a request that is certain to fail (a
  possible later use of ADR 0019's "remove only when certain" rule); any other status.

## Capabilities

### Modified Capabilities
- `request-routing`: the status that means "too large for this endpoint".

## Impact

`src/error.rs` classification, the pool's within-target walk and the metered client's relabel; tests; ADR 0023
amended; `docs/routing.md`. No configuration change.
