# Design

## Context

`fails_over` in `pool.rs` decides what an endpoint failure is (transport, timeout, 408, 429, 5xx), what moves
to the next endpoint of a target, and what the breaker counts. `fallback_class` in `error.rs` decides what a
route may fall through to the next target. Any other client error (400, 401, 403 for the breaker, 404, 422) ends
the request, by design: a malformed request should not be retried on other models and spend credits, and the
LiteLLM parity tests pin that. A free pool that tries unmeasured targets first (ADR 0020) makes the cost of
that rule visible: an endpoint that always answers 404 is never measured, never cools, and is always first.

## Decisions

- **Split request errors from endpoint errors.** 400 and 422 say the request is wrong, so every model would
  say the same and nothing should retry. 401, 403 and 404 say something about this endpoint: the model does not
  exist there, the key is rejected, the key may not use the model. Those are endpoint health, like a 5xx.
- **Health counts them for every route.** Breaker state belongs to the endpoint, which routes share. Counting
  needs `failure_threshold` consecutive failures and a success resets it, so one stray 403 does not cool a good
  endpoint, and health never denies service (the fail-open pass is unchanged).
- **Cross-target fallback follows the route type.** `unavailable` is on by default only where a route is built
  to try many targets that may be gone (`fastest` and pool routes). Elsewhere it is opt-in through
  `fallback_on`, because the operator wrote a deliberate list there (for example private then public) and a
  rejected key on the private target should not silently reach the public one. Strict routes keep their
  class limits regardless: `allowed_classes` removes targets before any fallback.
- **Within a target, failover on 401 and 404 for every route.** A target's endpoints are the operator's own
  alternatives of one model across providers, so moving to the next one is what they are for.
- **403 stays `forbidden` for routing.** It already has a class and an opt-in; it only joins the health count.
- **A narrow model-gone check on 400.** Providers differ (OpenRouter answers some missing models with 400).
  Only an `error.code` or `error.type` of `model_not_found`, `model_not_available` or `model_decommissioned`
  in the first 4 KiB of the body is read, as with retry hints (untrusted text, a bounded result). Anything
  else on a 400 is still the client's error.
- **When every target is unavailable the client still gets an error.** The last status is returned as it is
  today; nothing is hidden.

## Risks / trade-offs

- A real misconfiguration (a wrong base URL, a revoked key) now cools endpoints and moves on instead of
  failing loudly on the first request. The fail-open pass still tries them when nothing else remains, `explain`
  and `/v1/health` show the state and the cooldown source, and the log already records every transition.
- A 404 can come from a wrong path on a gateway in front of the provider; the effect is the same cooling and the
  same visibility.
- An existing multi-endpoint target now fails over on a 401 or 404 where it returned the error before. That is
  the point of having several endpoints, and it is stated as a change in the docs.
