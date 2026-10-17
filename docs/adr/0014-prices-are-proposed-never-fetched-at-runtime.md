# 0014. Prices are proposed by a CLI and never fetched at runtime

- Status: accepted
- Date: 2026-10-12
- Supersedes: none
- Change: [add-price-import](../../openspec/changes/add-price-import)

## Context

Endpoint prices are typed by hand and drift. Public machine-readable sources exist (models.dev,
provider model APIs). Prices drive budgets, which fail closed, so a wrong price changes who is
blocked and what requests are said to cost. Sources are third-party and can be wrong, change shape,
or be unreachable.

## Decision

Prices come from the configuration file only. A `humpyard prices` CLI can fetch a source and
*propose* entries (`import`) or *report drift* (`check`), but never writes the config and is never
called by the serving process. Source data is untrusted: HTTPS, timeouts, size caps, strict parsing,
a sanity ceiling, exact-id matching, and no API keys sent to a source.

Alternatives considered: the gateway refreshing prices itself on a timer (a source outage or bad
data would reach budgets and routing in production, and serving would gain a network dependency it
does not have today); an auto-applied scheduled import (a silent wrong price is worse than a stale
one, and it bypasses review); a bundled price table in the binary (stale the day it is released).

## Consequences

Serving behavior is unchanged by any source. Operators get cheap, reviewable updates and a drift
signal, at the cost of one manual step. A test must keep `serve` free of price-source requests.

## Refined by ADR 0022

The rule that the serving process never fetches third-party data stands, for prices and for model lists.
[ADR 0022](0022-free-pool-membership-may-be-updated-on-a-schedule-without-review.md) allows one narrow
exception to "an operator reviews every change": the membership of explicitly opted-in free pools may be
updated on a schedule by a separate command, under guardrails. Prices are untouched by it.

