# Roadmap

What has shipped, what is being built, and what comes next, at a glance. The detail behind each row
lives in the [component status table](architecture.md#component-status), the OpenSpec change named
in it, and the [decision records](adr/README.md). Update this page in the same PR as the change.

Last updated: 2026-10-14. Latest release: 0.6.0. Unreleased on `main`: quota history.

## Shipped

| Release | Date | What it added |
|---|---|---|
| 0.1.0 | 2026-10-07 | The gateway: OpenAI Chat, OpenAI Responses and Anthropic Messages in; Switchyard routing; a provider pool with ordered failover; virtual keys, a SQLite ledger and budgets; endpoint circuit breakers; graceful shutdown; local `count_tokens`; release workflow, container image and launchd service |
| 0.2.0 | 2026-10-09 | Hot reload; route selectors (ordered rules on the key, headers, tags, agent metadata); content conditions (prompt size, tools, images); `POST /v1/route/explain`; `fallback_on`; a client parity suite and a reasoning-replay fix |
| 0.3.0 | 2026-10-10 | The decision log: every request's rule, attempts, fallbacks and refusals, with `GET /v1/decisions` |
| 0.4.0 | 2026-10-10 | Routing statistics: `GET /v1/stats` (failure mix, latency percentiles, cost per request) |
| 0.5.0 | 2026-10-12 | Price import and drift check (`humpyard prices import` and `check`, models.dev first); tiered prices; visible cross-class fallback; the docs site; the auto-routing recipe |
| 0.6.0 | 2026-10-13 | `price_source` in config, so one command checks every provider; provider quota: a poller and `GET /v1/quota` (OpenCode Go first) |
| unreleased | | Quota history: samples kept in the ledger and `GET /v1/quota/history`, with the cost of the calls between samples |

## Building now

| Item | Why now | Spec |
|---|---|---|
| Measure quota against cost | Days of real data decide what a quota percent means, what the status values are and what happens at a cap. No rule should use it before that | `add-quota-routing` 2.1 |

## Next, in order

| # | Item | Depends on | Spec |
|---|---|---|---|
| 1 | Rules that match on quota (`when.quota`), shown in `explain` and recorded on decisions, plus a pacing recipe | the measurement above | `add-quota-routing` step 3 |
| 2 | Strict fallback: a route lists the classes it may fall through to, and is refused instead of crossing | cross-class visibility (shipped) | `add-auto-routing` 2.2 |
| 3 | Session stickiness: find which clients send a usable session id, and whether a conversation can be pinned to a pool | nothing | `add-auto-routing` 3.1 |
| 4 | A demo: a real Claude Code or Codex session through an `auto` route, with cost and latency from `/v1/stats` | prices in a real deployment | `add-auto-routing` 5.2 |

## Planned: the operator surface

In this order, API first. See [Direction: operating humpyard](architecture.md#direction-operating-humpyard).

1. A `/metrics` endpoint (Prometheus), so existing dashboards need no custom UI.
2. Health-driven target eligibility (tier substitution on a cold target).
3. A native Anthropic upstream, for providers that speak Anthropic Messages directly.
4. A versioned admin API with its own credential, and database-managed keys beside config-defined ones.
5. A read-only status page served by the binary, built on the same API.
6. `migrate-modelrelay`: a command and bundled catalog to bring a ModelRelay setup across.

## Later, and only on a measured trigger

- **Cost-aware ordering** inside the set a route allows, advisory first, after weeks of statistics.
- **A classifier-backed rule action**, if the decision log shows the catch-all route is too coarse.
- **Quota per model**, if a provider publishes it; adapters for other providers' usage endpoints.
- **A fuller admin GUI**, as an optional separate client of the admin API.

Nothing here is a promise of a date. Items move up when something real needs them, and move out
when the measurements say they are not needed.
