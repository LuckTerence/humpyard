# Roadmap

What has shipped, what is being built, and what comes next, at a glance. The detail behind each row
lives in the [component status table](architecture.md#component-status), the OpenSpec change named
in it, and the [decision records](adr/README.md). Update this page in the same PR as the change.

Last updated: 2026-10-14. Latest release: 0.8.2. Community: [Discord](https://discord.gg/qfTGr7KTfq) for questions and feedback; GitHub issues for bugs and requests.

## Shipped

| Release | Date | What it added |
|---|---|---|
| 0.1.0 | 2026-10-07 | The gateway: OpenAI Chat, OpenAI Responses and Anthropic Messages in; Switchyard routing; a provider pool with ordered failover; virtual keys, a SQLite ledger and budgets; endpoint circuit breakers; graceful shutdown; local `count_tokens`; release workflow, container image and launchd service |
| 0.2.0 | 2026-10-09 | Hot reload; route selectors (ordered rules on the key, headers, tags, agent metadata); content conditions (prompt size, tools, images); `POST /v1/route/explain`; `fallback_on`; a client parity suite and a reasoning-replay fix |
| 0.3.0 | 2026-10-10 | The decision log: every request's rule, attempts, fallbacks and refusals, with `GET /v1/decisions` |
| 0.4.0 | 2026-10-10 | Routing statistics: `GET /v1/stats` (failure mix, latency percentiles, cost per request) |
| 0.5.0 | 2026-10-12 | Price import and drift check (`humpyard prices import` and `check`, models.dev first); tiered prices; visible cross-class fallback; the docs site; the auto-routing recipe |
| 0.6.0 | 2026-10-13 | `price_source` in config, so one command checks every provider; provider quota: a poller and `GET /v1/quota` (OpenCode Go first) |
| 0.7.0 | 2026-10-14 | Quota history: samples kept in the ledger (schema v3, upgraded in place) and `GET /v1/quota/history`, with the cost of the calls between samples; the roadmap page |
| 0.7.1 | 2026-10-14 | [Try it in a minute](demo.md): a no-key demo (mock provider, real gateway) that routes by request type, explains a decision and compares cost; the humpyard badge, favicon and link previews; `rustls` 0.23.46 (RUSTSEC-2026-0346) |
| 0.8.0 | 2026-10-14 | `/metrics`: Prometheus metrics on one registry (a status page can read the same one later), off by default, on a separate listener or behind its own credential, labels only from configuration, opt-in per-key series; Grafana dashboard, alert rules, ServiceMonitor and NetworkPolicy examples (ADR 0016) |
| 0.8.1 | 2026-10-15 | Strict routes: `allowed_classes` removes targets of other classes before routing, so a private route can never reach a public target (validated at load, recorded on decisions, `humpyard_fallback_blocked_total`, ADR 0017); a summary Grafana dashboard (totals over a time range) and a corrected Switchyard link |
| 0.8.2 | 2026-10-15 | Health-driven eligibility: a target whose endpoints are all cooling down is left out of a request's plan when another target remains, failing open so health still never denies service (`cold_targets` on decisions, `humpyard_health_skips_total`, `[health] skip_cold_targets`, ADR 0018) |

## Building now

| Item | Why now | Spec |
|---|---|---|
| Capability eligibility: endpoints declare a context window, tool and image support, and a target that cannot serve a request is removed before routing, only on certainty (built, in review for 0.8.3, ADR 0019) | An oversized prompt or a tools request is sent to a target that cannot take it and recovers by failing; prompted by Switchyard #427 and the router literature | `add-capability-eligibility` |
| Measure quota against cost | Days of real data decide what a quota percent means, what the status values are and what happens at a cap. No rule should use it before that | `add-quota-routing` 2.1 |

## Next, in order

| # | Item | Depends on | Spec |
|---|---|---|---|
| 1 | Rules that match on quota (`when.quota`), shown in `explain` and recorded on decisions, plus a pacing recipe | the measurement above | `add-quota-routing` step 3 |
| 2 | Session and cache analytics (fast follow to capability eligibility): `group_by=session`, cache-hit ratio per target, and sessions that switched targets with the cached input they lost, so the cost of switching models mid-session is measured before anything pins a session | nothing | not yet proposed |
| 3 | Session stickiness: find which clients send a usable session id, and whether a conversation can be pinned to a pool. Soft stickiness (keep a session on its previous target while it stays eligible) only if item 2 shows switching is costly | item 2 | `add-auto-routing` 3.1 |
| 3b | Session-level experiments (fast follow): a `random` route that splits per session, not per request, records the arm on the decision and compares cost, latency and errors per arm; quality comparison waits for quality signals | item 2 | not yet proposed |
| 3c | Content signals: local, deterministic detectors (private keys and common API-token patterns, `.env`-style secrets, PII-lite such as emails and Luhn-valid card numbers) that produce facts for selector rules (`when.contains`), so a request holding a secret can be routed to a private pool, and with a strict route can never reach a public one. Categories and counts only are recorded, never matches or positions. The default action is to route; `refuse` (a clear 400) is an opt-in per rule. No masking, no response filtering, no third-party calls | nothing (best after strict routes, shipped) | not yet proposed |
| 4 | A demo on real traffic: a Claude Code or Codex session through an `auto` route, with cost and latency from `/v1/stats` (the no-key demo with a mock provider is shipped) | prices in a real deployment | `add-auto-routing` 5.2 |

## Planned: the operator surface

In this order, API first. See [Direction: operating humpyard](architecture.md#direction-operating-humpyard).

1. ~~A `/metrics` endpoint (Prometheus)~~ Shipped in 0.8.0; it is built on one registry, so the read-only status page (item 5) is a thin view over it.
2. ~~Health-driven target eligibility~~ Shipped in 0.8.2 (ADR 0018): a cold target is skipped only when an alternative remains.
3. A native Anthropic upstream, for providers that speak Anthropic Messages directly.
4. A versioned admin API with its own credential, and database-managed keys beside config-defined ones.
5. A read-only status page served by the binary, built on the same API.
6. `migrate-modelrelay`: a command and bundled catalog to bring a ModelRelay setup across.

## Later, and only on a measured trigger

- **Quality signals** in the statistics, so cost is never judged without how well a model answered.
  In stages: first free, objective signals (truncated or empty output, refusals, malformed tool
  calls, a retry in the same session); then a sampled, opt-in judge that runs off the request path
  on a target no more public than the request, stores only a score and a reason code (never content),
  and shows counts and spread in `/v1/stats`; calibrated against a stronger judge before it is trusted.
  Requests are grouped by task class (matching rule, tools, size), not by their text.
- **Cost-aware ordering** inside the set a route allows, advisory first, after weeks of statistics
  and with quality signals in them; it needs exploration, so a model that scored badly once still
  gets looked at again.
- **An external guardrail hook**, only on demand: one generic rule action that calls a configured HTTP service with a strict timeout and an explicit fail-open or fail-closed choice, so moderation and PII services (Lakera, Presidio, Bedrock and similar) can be used without humpyard embedding each. It puts network on the request path, so it needs its own ADR. Response-side filtering and prompt masking are not planned.
- **A classifier-backed rule action**, if the decision log shows the catch-all route is too coarse.
- **Quota per model**, if a provider publishes it; adapters for other providers' usage endpoints.
- **A fuller admin GUI**, as an optional separate client of the admin API.

Nothing here is a promise of a date. Items move up when something real needs them, and move out
when the measurements say they are not needed.
