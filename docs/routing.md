# Routing, failover and policy

How a request becomes a provider call. The flow is: decode the client protocol, pick a route,
let the routing policy narrow the eligible targets, run the route's Switchyard algorithm, call
the selected target (failing over across its endpoints), then encode the answer for the client.

## Providers, targets, routes

- A **provider** is an OpenAI-compatible endpoint with an API key from an environment variable
  and optional static `headers` sent on every call (some providers require identifying headers).
  Per-conversation header values, such as a session id, are not supported yet.
- A **target** is a named model served by an ordered list of `{ provider, model }` endpoints.
- A **route** is what clients request as `model`. It wraps a built-in Switchyard algorithm over
  targets. A bare target name also works: it behaves as a passthrough route.

| Route type | Behavior |
|---|---|
| `passthrough` | First target; the rest are fallbacks |
| `random` | Weighted pick (`weights`, `seed`); the rest are fallbacks |
| `stage_router` | Tool-result signals choose `efficient` or `capable` (`mode`, `confidence_threshold`) |
| `llm_classifier` | A `judge` target decides: `capability` mode judges task difficulty, `escalation` mode latches the session to `capable` after `confirmations` verdicts |

Algorithms keep per-session state (escalation latches, capable-hold turns). Clients identify a
session with the `x-switchyard-session-id` header; without it, state-dependent behavior is limited.

## Selecting the route

By default the client's `model` names the route. Selector rules let the operator decide the route
from facts about the request instead, so different clients, keys or agents follow different flows
(for example private, then paid, then public for one client and paid only for CI). Rules are
evaluated top to bottom and the first one that matches wins; with no match the requested model is
the route.

```toml
[[select]]
name = "subagents"                       # optional; shown in x-humpyard-rule
when = { subagent = true }
route = "cheap-first"

[[select]]
when = { profile = "deep" }              # the client sent x-humpyard-profile: deep
route = "paid-only"

[[select]]
when = { tag = { team = "infra" } }      # x-humpyard-tag-team: infra
route = "paid-only"

[[select]]
when = { key = "ci-*" }                  # the authenticated key's id
route = "paid-only"

[[select]]
when = { header = { "x-client" = "openclaw" } }
route = "cheap-first"

[[select]]
when = { prompt_tokens = { min = 30000 } }   # long prompts go to a long-context tier
route = "long-context"

[[select]]
when = { images = true }                 # vision-capable models only
route = "vision"

[[select]]
route = "default"                        # no conditions: matches everything
```

All conditions of a rule must hold. Text conditions are globs (`*` matches any text, everything
else is literal and case-sensitive). The conditions are:

| Condition | Matches |
|---|---|
| `model` | the model the client asked for |
| `key` | the authenticated key's id (never matches on an open gateway) |
| `profile` | the client's `x-humpyard-profile` header |
| `tag` | `{ name = glob }` against `x-humpyard-tag-<name>` headers |
| `header` | `{ name = glob }` against any other request header (names are case-insensitive) |
| `agent`, `task` | the agent and task ids the client reports (`x-switchyard-agent-id`, `x-switchyard-task-id`) |
| `subagent` | whether the client marked the request as coming from a sub-agent |
| `stream` | whether the client asked for a streamed response |
| `prompt_tokens` | `{ min, max }` (inclusive, either may be omitted) against the gateway's estimate of the prompt size, the same conservative number `count_tokens` returns |
| `tools` | whether the request defines tools |
| `images` | whether the request carries images |

**Standard client headers.** Clients such as agent harnesses can describe themselves with
`x-humpyard-profile: <name>` and `x-humpyard-tag-<name>: <value>`; they mean nothing until a rule
uses them.

**Clients can narrow, never widen.** A rule only applies if the key may use its route
(`allowed_routes`); otherwise it is skipped and the next rule, or the requested model, applies, so
a header can neither reach a route the key may not use nor turn a working request into a refusal.
Give any key whose clients you do not fully trust an `allowed_routes` list. Credential headers
(`authorization`, `x-api-key`, `cookie`, `proxy-authorization`) cannot be matched. A rule can also
rescue a model name the gateway does not know, which helps clients that hard-code one.

**Explaining a decision.** `POST /v1/route/explain` (with your key) says what a request would do,
without calling a provider or spending anything. Describe the request; only `model` is required:

```sh
curl localhost:8080/v1/route/explain -H "Authorization: Bearer $KEY" -H 'content-type: application/json' \
  -d '{"model":"agent","headers":{"x-humpyard-profile":"deep"},"subagent":false,"stream":false}'
```

Add `prompt_tokens` (the estimate, for example from `count_tokens`), `tools` and `images` to ask
about content-based rules; an unstated size never matches a `prompt_tokens` rule. The answer has `selected` (`route`, `rule`, and whether it came from a `selector` or the
`requested_model`), `rules` (every rule in order: `matched`, the `mismatches` that stopped it, and
`key_may_use_route`), the key's `budget` state, the `targets` the route would use after the routing
policy with each endpoint's health, the route's `fallback_on`, and an `outcome`: `ok`, `forbidden`,
`unknown_model`, `budget_exhausted` or `no_eligible_target`. It runs the same code as real
requests, so the two agree; it only answers for the calling key.

Targets in the report carry `recent` (24-hour attempts, failure rate and median latency, from
[`/v1/stats`](budgets.md#statistics)) so you can see how a route has really been behaving.

Every request also gets an `x-humpyard-request-id`; the [decision log](budgets.md#the-decision-log)
keeps what was decided (rule, skipped rules, attempts, outcome) so it can be reviewed later.

Responses carry `x-humpyard-route` (the route that served) and `x-humpyard-rule` (the rule's name,
`select[<index>]` when unnamed, or `default` when the requested model named the route); the log
line has both and the ledger's route column records the route. Selection happens before the budget
check and the routing policy, which then apply to the chosen route. Loading the config fails when
a rule names an unknown route, an unknown condition, a credential header, repeats a name, or
follows a catch-all (it could never match). See [ADR 0011](adr/0011-selectors-choose-the-route.md).

## Auto routing recipe

The goal: clients always ask for one model, `auto`, and the gateway picks the pool from what the
request is. It needs no new feature, only selector rules; [examples/auto-routing.toml](../examples/auto-routing.toml)
is a complete, tested starting point (the test suite loads that very file).

| # | Rule | Condition | Pool |
|---|---|---|---|
| 1 | `private-profile` / `private-tag` | the client sent `x-humpyard-profile: private`, or `x-humpyard-tag-privacy: private` | `private-pool` (your own hardware) |
| 2 | `images` | the request carries images | `vision` |
| 3 | `long-prompt` | estimated prompt of 30,000 tokens or more | `long-context` |
| 4 | `subagent` | the client marked it as a sub-agent turn | `cheap` |
| 5 | `tool-turns` | the request defines tools | `capable` |
| 6 | `everyday` | anything else sent to `auto` | `cheap` |

Things the recipe gets right, and that you must keep when you adapt it:

- **Scope every rule with `model = "auto"`.** Selectors see every request. Without that condition,
  a client that asks for `capable` directly would be re-routed too. (Tested: a direct request for
  a pool bypasses the rules.)
- **Order encodes priority.** Privacy is first, so a private request that also has images, tools
  and a huge prompt stays private. A sub-agent turn with tools stays cheap because rule 4 comes
  before rule 5. Use `explain` to check any ordering question before sending traffic:
  `curl localhost:8080/v1/route/explain -H "Authorization: Bearer $KEY" -d '{"model":"auto","tools":true,"subagent":true}'`.
- **`auto` is also a real route.** It is where a request lands when no rule applies or every
  matching rule is skipped for the key, so it must be safe for anything.
- **The private pool has no public fallback here.** The private route lists only the local target,
  so a failure is an error, never a silent trip to a public provider. If you prefer to degrade to
  public, add a public target to that route, knowingly. (A first-class way to declare which classes
  a route may fall through to is planned; see `add-auto-routing`.)
- **Keys and the allowlist.** A rule only applies if the key may use its route
  (`allowed_routes`). A key limited to `auto` is therefore never sent to a pool (the rules are
  skipped and `auto` itself serves, as tested), and a key that lists the pool routes can also
  request them directly. There is no "reach pools only through `auto`" setting yet; give each
  consumer the routes it may use, and keep privacy-sensitive routes off keys that should not
  reach them.
- **Prompt size is an estimate** (the same conservative number `count_tokens` returns), so leave
  headroom in the threshold.

How a client expresses intent without code changes is in [clients.md](clients.md#telling-the-gateway-what-a-request-is).
Once traffic flows, `GET /v1/decisions?rule=tool-turns` and `GET /v1/stats?group_by=route` show how
often each rule fires and what each pool costs and how long it takes: tune the thresholds from that.

## Failover

- **Within a target**, endpoints are tried in order. The gateway moves to the next endpoint on
  connection errors, timeouts, HTTP 408, 429 and 5xx. Other 4xx responses stop immediately
  (another endpoint would reject the same request). Nothing fails over once a stream has started.
  `max_retries` on a provider sets extra attempts on that provider *before* failing over.
- **Skipping failing endpoints.** Failing over on every request would make a dead provider cost a
  full timeout each time, so each endpoint (provider + model) has a circuit breaker; see
  [Endpoint health](#endpoint-health).
- **Across targets**, if every endpoint of the selected target fails, the next target the
  algorithm returned is tried. By default falling through happens for a context-window overflow,
  429, 403, 408, any 5xx, connection errors and timeouts. Other client errors (400, 401, 404) stop
  at once and reach the client, because another target would reject the same request. If no
  target answers, the client gets the last error (a timeout is still reported as 504).
- **Choosing what falls through.** A route can limit this with `fallback_on`, a list of failure
  classes (`overflow`, `rate_limit`, `timeout`, `server_error`, `connection`, `forbidden`). A
  failure outside the list ends the request and the client sees the original status and message.
  `fallback_on = []` turns target fallback off. This keeps a paid last resort from being spent on
  a public blip:

  ```toml
  [routes.deep-public]
  type = "passthrough"
  targets = ["pub-deep", "paid-deep"]
  fallback_on = ["overflow"]     # go to paid only when the prompt does not fit
  ```

  It applies to every route type and governs the hand-over between targets; endpoints inside one
  target are a rotation and always fail over.
- If everything fails, the client gets the last error: 429/4xx pass through, unreachable is 502,
  timeout is 504.

Responses carry `x-humpyard-target` (who served) and `x-humpyard-provider` (which endpoint).

## Provider quota

Capped plans (for example OpenCode Go: a monthly fee with usage limits in a 5-hour, a weekly and a
monthly window) fail at a limit the gateway cannot otherwise see until the provider answers 429.
A provider can name a **quota source**, and the gateway then reads its usage windows while serving:

```toml
quota_poll_secs = 60          # how often (default 60)
quota_stale_secs = 300        # older than this counts as unknown (default 300)

[providers.opencode]
base_url = "https://opencode.ai/zen/go/v1"
api_key_env = "OPENCODE_API_KEY"
quota_source = "opencode-go"
```

`GET /v1/quota` shows the latest snapshot per provider: each window's percent used, the provider's
status text, when it resets, and how old the data is. Things to know:

- **It is account-level.** OpenCode's usage endpoint reports one set of windows for the whole
  account; it does not say which model is near its own cap. Treat it as "how hot is this provider".
- **Advisory, and it fails open.** Reading it never delays a request: it runs off the request path
  with a timeout, and if the provider is slow, down or answers nonsense the snapshot goes stale.
  Stale, missing or never-polled data is **unknown**, with the reason shown.
- **The key is only sent to the provider's own host**, over https (the config is rejected for a plain
  http provider URL, except to a loopback host), and never appears in logs or responses.
- **Reloads keep the snapshot** while the provider's URL and source are unchanged.
- **History is kept** (when a ledger is configured) so a percent can be compared with measured cost:
  a poll is stored whenever a window's percent, status or reset time changes, and at least every 15
  minutes, for `decision_retention_days`. `GET /v1/quota/history` returns the samples, each with the
  notional cost of the provider's calls since the previous sample of the same window. Because the
  percent is account-wide, it can move with traffic that did not come through the gateway; compare
  trends, not single steps.
- **Nothing routes on it yet.** This step only observes. Selector rules that match on quota come
  after the percentages have been compared with measured cost (`add-quota-routing`).

## Target classes

A target can carry a **class** label, conventionally `private` (your own hardware), `public` (free
or shared providers) or `paid`, or one of your own (lowercase letters, digits, `-` and `_`):

```toml
[targets.local-model]
class = "private"
endpoints = [{ provider = "local", model = "local-coder" }]
```

Classes **change nothing about routing by themselves**: a route is strict only if it sets
`allowed_classes` ([Strict routes](#strict-routes)). They make a quiet failure visible: a `private` route that
lists a public target after it will, when the private target fails, serve the request from the public
one, and until now nothing said so. With labels:

- `GET /v1/decisions` adds `fallback_crossed_classes` to each decision (`true` when the attempts
  spanned more than one class, `false` when they did not, `null` when a target has no class), and each
  attempt shows its `class`.
- `POST /v1/route/explain` shows each target's `class`, the route's `classes` in order, and
  `fallback_can_cross_classes`, so you can see before sending anything that a route may leave its
  first class.

`GET /v1/decisions?outcome=ok` plus a look at `fallback_crossed_classes` answers "how often did my
private traffic end up somewhere public?". To forbid crossing outright, make the route strict.

### Strict routes

A route can say which classes it may use at all:

```toml
[routes.coding-private]
type = "passthrough"
targets = ["local-coder", "paid-coder"]
allowed_classes = ["private"]
```

Targets whose class is not listed, and targets with no class, are **removed from the route before the
routing algorithm runs**, so no failure, retry or tier substitution can reach them: the public target
in the example is never called, not even when the private one is down. The client gets the private
target's error, and nothing is rewritten.

- **Validated when the config loads.** A listed class must be a valid label; every target the route
  mentions must have a class (an unclassed target on a strict route is an error, not a silent
  exclusion); and at least one target must have an allowed class.
- **Strictness only removes.** Health still never tries to deny service (a cooling-down allowed target
  is tried as before), budgets narrow within the allowed set, and no selector rule or header adds a
  target back. A route without `allowed_classes` behaves exactly as before.
- **Visible.** `explain` shows the route's `allowed_classes`, the `excluded_targets` with the reason,
  and `fallback_can_cross_classes: false`. Each decision lists its `excluded_targets`
  (`GET /v1/decisions`), and `humpyard_fallback_blocked_total{route}` counts requests that failed on a
  strict route that had excluded targets, which is the signal that a fallback you forbade would have
  happened ([Metrics](metrics.md)).
- **The cost is redundancy by design.** A strict route is only as available as its allowed targets, so
  alert on the metric if that matters.

## Capabilities

An endpoint can say what its model can take, so a request is never sent to a target that cannot serve
it:

```toml
[targets.local-coder]
endpoints = [{ provider = "local", model = "qwen3-coder", context_window = 32768, tools = true, vision = false }]
```

- `context_window` is the model's window in tokens, `tools = false` says it does not support tool
  calls, `vision = false` that it does not accept images. **Anything you leave out means unknown,
  which means capable**, so declaring nothing changes nothing.
- **Before routing**, a target is removed from a request's plan when none of its endpoints can serve
  it: the request uses tools and every endpoint says `tools = false`, it carries images and every
  endpoint says `vision = false`, or its prompt is certainly larger than every declared window. This
  is the same planning step as [strict routes](#strict-routes) and [health](#endpoint-health), after
  classes and the routing policy and before health, and it only ever removes targets.
- **Only when certain.** The gateway's prompt estimate errs high on purpose, which would wrongly remove
  a target that fits. Capability uses a separate, deliberately low estimate (about one token per six
  bytes), so a removal means "even an optimistic tokenizer cannot fit this". A prompt close to the
  limit still goes to the target, and the `overflow` [fallback](#failover) handles a real overflow as
  before.
- **A target stays if any endpoint can serve.** Inside a target the endpoints are still tried in
  order; skipping individual endpoints is not done yet.
- **If nothing can serve the request**, it is refused before any upstream call with HTTP 400
  `no_capable_target`, and the message names the reason (for example "the request uses tools but its
  endpoints declare tools = false"). That is a fact you declared, unlike health, so it does not fail
  open; remove or correct the declaration if it is wrong.
- **Visible.** `explain` lists `incapable_targets` with the reason (and shows `no_capable_target` when
  it would refuse); in `explain`, a `prompt_tokens` you pass is used as the estimate. Each decision lists
  its `incapable_targets` (`GET /v1/decisions`), and
  `humpyard_capability_skips_total{route,target,reason}` counts removals ([Metrics](metrics.md)).
- **Where the numbers come from.** `humpyard prices import` proposes `context_window`, `tools` and
  `vision` from [models.dev](https://models.dev) next to prices, for review; `prices check` reports a
  declaration the source contradicts ([Keeping prices current](budgets.md#keeping-prices-current)).

**Tuning the estimate.** The low estimate assumes at least 6 bytes per token, above what any tokenizer
averages (2 to 5). To check it against your own traffic, compare the ordinary estimate the gateway
recorded with the input tokens the provider reported; the ordinary estimate is about twice the low
one, so real counts should stay above half of it:

```sh
sqlite3 humpyard.db "SELECT COUNT(*), ROUND(MIN(1.0*u.input_tokens/d.prompt_tokens),2) AS worst,
  ROUND(AVG(1.0*u.input_tokens/d.prompt_tokens),2) AS typical
  FROM decisions d JOIN usage u ON u.request_id = d.request_id AND u.kind = 'answer'
  WHERE d.prompt_tokens > 0 AND u.input_tokens > 0;"
```

If `worst` is well below 0.5 for traffic you care about, the lower bound is not a bound there and the
divisor should come down. (`prompt_tokens` is recorded when a rule or a declared window needs the
estimate.)

## Changing the configuration while running

Routes, selector rules, targets, providers, keys and budgets can be edited and reloaded without a
restart (SIGHUP, or `reload_poll_secs`); see [deployment.md](deployment.md#reloading-the-configuration).
A reload is all or nothing, in-flight requests keep the configuration they started with, and
endpoint health and unchanged routes' session state carry over.

## Endpoint health

After `failure_threshold` consecutive failures of the kind that trigger failover (connection
errors, timeouts, HTTP 408, 429, 5xx) an endpoint is skipped for `cooldown_secs`. Each time it
trips again the cooldown doubles, up to `max_cooldown_secs`. When a cooldown ends, exactly one
request is sent as a probe: success returns the endpoint to service at once and resets the
cooldown; failure reopens it for longer. Other 4xx responses say nothing about the endpoint and are
never counted.

```toml
[health]               # all optional; these are the defaults
failure_threshold = 3  # 0 turns health tracking off
cooldown_secs = 30
max_cooldown_secs = 300
skip_cold_targets = true  # leave a cold target out of the plan when another target remains
```

- **A cold target is skipped when an alternative exists.** A target whose endpoints are *all*
  cooling down is left out of the route's targets for a request, so the request does not spend its
  first attempt (and a recorded failure) on a target the gateway already knows is down. A tier
  emptied this way is served by the other tier, as with any [routing policy](#routing-policy) removal.
  It happens in the same planning step as [strict routes](#strict-routes) and the budget policy, after
  them: it never adds a target back, and a strict route skips only within its allowed targets.
- **Health never denies service.** If skipping would leave nothing (every eligible target is cold, or
  the only one is), nothing is skipped and the targets are tried as before, soonest-to-reopen first.
  A request is never refused because of health.
- **Recovery is by probe.** A target stops being cold when its cooldown ends; the next request that
  reaches it is the probe, and a success returns it to service. While it is skipped it receives no
  traffic, so recovery takes at most one request after the cooldown.
- **Visible.** `explain` marks each target `cold` or not, lists `health_skipped_targets`, and says
  `health_fail_open` when every eligible target was cold. Each decision lists its `cold_targets`
  (`GET /v1/decisions`), and `humpyard_health_skips_total{route,target}` counts the skips
  ([Metrics](metrics.md)). Set `skip_cold_targets = false` for the old always-in-order behavior.
- State is in memory per process: a restart starts everything healthy. Failures after a stream has
  begun are not counted (nothing fails over then either).
- The log shows each transition ("endpoint is failing; skipping it for the cooldown",
  "endpoint recovered"), and `GET /v1/health` (with a key when the gateway has keys) lists every
  endpoint's `state` (`healthy`, `cooling_down`, `probing`), `consecutive_failures` and
  `cooldown_remaining_ms`.
- Not yet: `Retry-After` is not honored. See [ADR 0009](adr/0009-endpoint-health-lives-in-the-pool.md)
  and [ADR 0018](adr/0018-cold-targets-are-skipped-only-when-an-alternative-remains.md).

## Routing policy

A policy decides, per request, which targets are eligible. Ineligible targets are removed from
every group the algorithm sees, including its fallbacks.

- **Tier substitution:** if the policy removes every `capable` target, `capable` is served by the
  eligible `efficient` targets (and the reverse; a missing `judge` falls back to a remaining tier).
  Limits therefore degrade routing instead of failing requests.
- **Random routes** re-align their weights to the remaining targets.
- If **nothing** is eligible, the request fails with HTTP 503 naming the route.

The gateway always applies the budget policy (see [budgets.md](budgets.md)); an embedder-supplied
policy (default `AllowAll`) is composed with it, and a target must satisfy both. Embedders implement
`humpyard::policy::RoutingPolicy` and start the server with
`server::router_with_policy`. A policy receives the route, the session id and request metadata.
Health-aware target eligibility is a planned change built on this seam.
