# 0023. Answers that describe the endpoint make it unavailable

- Status: accepted
- Date: 2026-10-17
- Supersedes: none
- Change: `openspec/changes/add-unavailable-endpoints`

## Context

Until now every upstream client error (400, 401, 403, 404, 422) ended the request and none counted against an
endpoint's health, on the reasoning that a malformed request should not be retried on other models and spend
credits. That is right for a request error and wrong for an endpoint error: a 404 (the model is gone) or a 401
(the key is rejected) says nothing about the request. In a `fastest` route, which tries unmeasured targets
first (ADR 0020), an endpoint that always answers 404 is never measured and never cools, so it stays first and
fails every request. A test showed it. A free pool is full of such models.

## Decision

Split the answers. A 400 or 422 describes the request: it ends the request and counts for nothing. A 404, a
401 or a 403, or a 400/404 whose `error.code` or `error.type` is `model_not_found`, `model_not_available` or
`model_decommissioned` (read from the first 4 KiB of the body), describes the endpoint: it counts against the
endpoint's health like a 5xx, on every route. A 401 or 404 (and the model-gone 400) also moves on to the next
endpoint of the same target on every route, and a new fallback class `unavailable` moves it to the next target.
That class is included by default only on `fastest` routes and pool routes, and is opt-in elsewhere; an
explicit `fallback_on` list is taken as written. 403 keeps its own class, `forbidden`. Switchyard moves on a 5xx
and not a 404, so an allowed unavailable answer is handed to it re-labelled as a 503 that carries the original
status and text, and is restored if no target answers, so a client still sees the real status.

Alternatives considered: counting only 404 (a rejected key is the same problem); making `unavailable` the
default everywhere (it changes routes that were written with a deliberate list, such as private then public,
and a rejected key on the private target should not silently reach the public one); trying to remove dead
targets from the configuration (a different, slower fix that needs a prober or a person); ignoring the problem
and relying on the unmeasured-first rule changing (it would only hide the cause, and a 404 target would still
be first on every cooldown probe).

## Amendment (2026-10-17)

The first live free pool in front of a real tier found two statuses the list missed: `410 Gone` (a model
removed for good, answered on every request by one member, which a `fastest` route then tried first every time)
and `402 Payment Required` (a key with no credit left). Both describe the endpoint and are now treated like a
404 and a 401. Change: `openspec/changes/add-gone-and-payment-unavailable`. A test now pins the exact set of
client errors that describe the endpoint across 400 to 451, so a status added later is a visible decision;
405 and 407 stay request errors until a provider is seen answering them.

## Amendment (2026-10-19)

The decision log showed four deep-tier requests of 35,000 to 77,000 tokens ending in `413 Request too large … tokens
per minute` from a free plan's cap, with nothing tried after it. A 413 describes the request relative to one
endpoint, not the endpoint's health, so it differs from the statuses above: it carries the `overflow` class (the
next endpoint of the target, the next pool member, and the guarded tail under ADR 0024's default), and it is not
counted against the endpoint, which stays available for smaller requests. If nothing takes the request the client
gets the real 413. Change: `openspec/changes/treat-413-as-overflow`. The sweep test now pins 413 among the statuses
that move a pool on.

## Consequences

A pool of free models survives dead members: the request moves on, the dead endpoint cools after the
threshold and is probed again after its cooldown. Routes that do not opt in return the same status as before
for a 404 or 401, but their dead endpoints now cool, so a later request can skip them, and a target with
several endpoints now moves on within itself. A real misconfiguration (a wrong base URL, a revoked key) now
cools instead of failing loudly on every request; the log records the transition, `/v1/health` shows the state,
and health still never denies service.
