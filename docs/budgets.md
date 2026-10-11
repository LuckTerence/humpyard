# Keys, usage and budgets

Everything here is opt-in. With no `[keys.*]` the gateway is open and unbudgeted, exactly as
before. See [examples/budgets.toml](../examples/budgets.toml) for a complete config.

## Virtual keys

Clients authenticate with gateway-issued keys. Keys are stored **only as SHA-256 hashes** in the
config; the key itself is shown once when you mint it:

```sh
humpyard keygen alice
# Key (shown once; store it securely): sk-humpyard-…
# [keys.alice]
# sha256 = "sha256:…"
```

Paste the printed block into the config. Clients send `Authorization: Bearer <key>` or
`x-api-key: <key>` (Anthropic clients use the latter). Responses to bad keys are 401; a key
restricted by `allowed_routes` that asks for another route gets 403. Logs name the key **id** only,
never the key or its hash. `GET /v1/key/info` shows the calling key's limits, spend and state.

## Prices and cost

Each endpoint may declare `price = { input, output, cached_input }` in USD per million tokens.
Cost is computed from the tokens the provider reports and kept in whole micro-USD. A config with
any USD budget must price every endpoint (use `0` for free models), so nothing is silently free.
Reasoning tokens are part of the output tokens the provider reports (they are recorded as detail,
not added again), and cache-read tokens bill at `cached_input` (default: `input`).

## The usage ledger

`[ledger] path = "…"` is a SQLite file. Every upstream call is one row: time, key id, session,
route, target, provider, model, kind (`answer`, or `judge` for classifier calls the client never
sees), token counts, cost and outcome (`ok`, `error`, `cancelled`). Judge calls are charged to the
same key. Rows are written by a background task and never delay a response; a full queue or a
failing database is logged and counted, not surfaced to clients. Streams are recorded when they end
(or when the client disconnects, as `cancelled`; a request cancelled before it answers still
records the judge calls it already paid for); a stream that ends without usage (cut by the grace period, dropped client, or a provider that
never reports it) is recorded with `usage_missing = 1` and an **output-only estimate** from the
text delivered (conservative, about 3 bytes per token); its input is not counted. A buffered
response without usage, or a stream that generated nothing, records zero.

On SIGTERM or Ctrl-C the gateway drains in-flight requests, cancels what is left after
`shutdown_grace_secs`, and **flushes the queue to the ledger before exiting**; if it cannot (10
second limit) it exits non-zero and logs that entries may be lost. A hard kill (SIGKILL, a crash,
power loss) can still lose entries that were queued but not yet written, so budgets rebuilt after
one may be slightly low.

## The decision log

Next to the usage rows, the same database holds one **decision** per inference request, so you can
see afterwards what the router did and why, not only what it cost. A decision records: the request
id (also returned as `x-humpyard-request-id`, and stored on the request's usage rows so the two
join), the key, session and protocol, the model the client asked for, the route and selector rule
that applied and whether it came from a `selector` or the `requested_model`, rules that matched but
were skipped because the key may not use their route, the budget state, the targets that were
eligible, request features (streaming, estimated prompt size when a rule needs it, tools, images),
the **attempts** in order (target, provider, model, class, outcome such as `ok`, `rate_limit`,
`timeout`, `server_error`, and milliseconds), `fallback_crossed_classes` (whether those attempts spanned
more than one [target class](routing.md#target-classes)), the final status and `outcome`, the serving target and
the latency. Refusals are decisions too (`unauthorized`, `forbidden`, `unknown_model`,
`budget_exhausted`, `no_eligible_target`, ...), and a request the client abandons is recorded as
`cancelled`.

It is **metadata only**: no prompts, no responses, error text clipped to 200 characters, and the
requested model name clipped too. It is written by the same asynchronous writer as usage (never
delaying a response, flushed on shutdown), and pruned after `decision_retention_days` (top level,
default 90, `0` keeps everything; changing it needs a restart). An older database is upgraded in
place when the gateway first opens it.

Read it with `GET /v1/decisions` (a key sees only its **own** decisions, newest first; an open
gateway sees all; there is no admin view yet). Filters: `route`, `rule`, `outcome`, `request_id`,
`since_ms` (unix milliseconds) and `limit` (default 50, at most 500):

```sh
curl "localhost:8080/v1/decisions?outcome=forbidden&limit=5" -H "Authorization: Bearer $KEY"
curl "localhost:8080/v1/decisions?request_id=<id from the x-humpyard-request-id header>" -H "Authorization: Bearer $KEY"
```

### Statistics

`GET /v1/stats` summarizes how routes, targets and endpoints have actually performed, computed on
read from the decision log and usage rows (nothing in routing uses these numbers). Parameters:
`window` (`30m`, `24h`, `7d`; default 24h, never longer than `decision_retention_days`),
`group_by` (`route` (default), `target` or `endpoint` as `provider/model`) and the filters `route`
and `target`. Like decisions, a key sees only its own requests.

```sh
curl "localhost:8080/v1/stats?group_by=target&window=24h" -H "Authorization: Bearer $KEY"
```

Per group: `requests`, `attempts`, `outcomes` (attempts by class: `ok`, `rate_limit`, `timeout`,
`server_error`, `connection`, ...), `failures` and `failure_rate`, `fallbacks` and `fallback_rate`
(requests that needed more than one attempt), `latency_ms` (`p50`, `p90`, `p99` over all attempts,
nearest rank, so slow timeouts show in the tail) and, from the usage rows, the `answered` count with
`avg_cost_usd` and `avg_tokens` per answered request. `truncated` is true when the window held more
than 50,000 decisions and only the newest were summarized. Small samples mislead: look at the counts
before trusting a rate. `POST /v1/route/explain` adds the same recent numbers (last 24 hours: attempts,
failure rate, p50) to each target it lists.

Not recorded: why a `stage_router` or classifier route picked a tier (the engine does not expose
it; you see the target that served), and attempts that fail after a stream has begun.

## Budgets

Per key: `daily_usd`, `monthly_usd`, `daily_tokens`, `monthly_tokens` over UTC calendar days and
months. Spend counts the instant a call finishes, and is rebuilt from the ledger on restart.

| State | Condition | What happens |
|---|---|---|
| healthy | under `restricted_at` of every limit | normal routing |
| restricted | at or over `restricted_at` of any limit | targets whose output price is above `restricted_max_output_price` are excluded; routing degrades (capable falls back to efficient) |
| exhausted | at or over any limit | `over_budget = "block"`: HTTP 402 before any upstream call. `"free_only"`: only zero-priced targets, 402 if the route has none |

**Overshoot is bounded, not zero.** Cost is known only when a call finishes, so requests already in
flight when a limit is crossed will complete and can take a key past it. New requests are refused
as soon as the limit is observed.

A target's price for these rules is the **highest** price among its endpoints (so a target with an
expensive failover endpoint counts as expensive); a target is free only if every endpoint is free.

## Notes

- Switchyard falls back across *all* of a route's targets when the answer candidates fail, so a
  route's judge target can end up answering as a last resort; that call is recorded as an answer.
- **Moving to database-managed keys later** is designed for: key lookup sits behind a store trait
  and the ledger refers to key ids, never hashes. A managed store adds a keys table, an admin API
  and an import of config keys; the ledger and budgets do not change.
