# Proposal

## Why

Putting a real free pool in front of the deep public tier showed an upstream answering `410 Gone` on every
request. A 410 is classed as a bad request, so it ends the request, never counts against the endpoint and never
cools, and a `fastest` route (unmeasured targets first) tries it first every time: the route failed 5 of 5.
ADR 0023 listed the statuses that describe the endpoint (404, 401, 403, model-gone 400) from the cases seen so
far; 410 and 402 were missed. 410 says the resource is permanently gone; 402 says the key is out of credit
(a spent free tier), which no retry on that endpoint fixes.

## What Changes

- `410` and `402` become "unavailable" answers: they count against the endpoint's health like a 404, fail over
  within a target on every route, and carry the `unavailable` fallback class (default-on for `fastest` and pool
  routes, opt-in elsewhere), restored to the real status when nothing answers.
- Out of scope: other statuses (405, 451, ...) until seen; no change to 400/422 request errors.

## Capabilities

### Modified Capabilities
- `request-routing`: the statuses that describe the endpoint.

## Impact

`src/error.rs` classification; tests; ADR 0023 amended; docs/routing.md list. No config change.
