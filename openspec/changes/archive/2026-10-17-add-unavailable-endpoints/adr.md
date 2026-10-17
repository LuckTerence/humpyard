# ADR Review

## Reviewed

ADR 0009 (endpoint health lives in the pool) and ADR 0018 (health-driven eligibility) define what counts as an
endpoint failure and what skipping does; this widens the count and nothing else. ADR 0017 (strict routes) keeps
fallback from crossing classes a route does not allow. ADR 0020 (latency ordering) is where the problem
appears: an unmeasured target is tried first, so an endpoint that fails without counting is never demoted.
The LiteLLM parity tests (`tests/parity.rs`, the "client errors do not fall back" rule) are kept for every
route that does not opt in.

## Outcome

A new ADR is needed: it refines the client-error rule from "no 4xx counts" to "401, 403 and 404 are about the
endpoint".

## New ADRs

- ADR 0023: Answers that describe the endpoint count as the endpoint being unavailable. Records the split
  between request errors (400, 422) and endpoint errors (401, 403, 404), the route-type default for the new
  `unavailable` class, and why it is off by default elsewhere.
