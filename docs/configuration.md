# Configuration

See the [example configs](examples.md); `humpyard check-config <file>` validates one. Entries:

- `[providers.<name>]`: an OpenAI-compatible endpoint. The API key is read from the environment
  variable named by `api_key_env`; an inline key is rejected. Optional `headers = { ... }` adds
  static HTTP headers to every call (authentication headers are rejected).
- `[targets.<name>]`: a model served by an ordered list of `{ provider, model }` endpoints.
  Clients may request a target by name; endpoints are tried in order, failing over on
  connection errors, timeouts, HTTP 429 and 5xx (not on other 4xx, and not once a stream has begun).
  Responses carry `x-humpyard-target` and `x-humpyard-provider` headers.
- `[routes.<name>]`: a built-in Switchyard algorithm over targets, requested by clients as the
  `model`: `passthrough`, `random` (weights, seed), `stage_router` (tool-result signals pick
  efficient vs capable), or `llm_classifier` (`capability` judges task difficulty, `escalation`
  latches to capable after repeated judge verdicts; both need a `judge` target). Send
  `x-switchyard-session-id` so per-session state (latches, holds) persists across requests.
  If the selected target fails entirely, the next target the algorithm returned is tried; a
  route's `fallback_on = ["overflow", ...]` limits which failures may do that (default: all).
- `price_source = "models-dev:<provider id>"` on a provider (optional): where `humpyard prices`
  reads its prices; serving never uses it. See [Keeping prices current](budgets.md#keeping-prices-current).
- `price = { input, output, cached_input, tiers }` on an endpoint (USD per million tokens;
  `tiers` raise the rates above a prompt size): see [Prices and cost](budgets.md#prices-and-cost).
- `class = "private"` on a target (optional): a label (`private`, `public`, `paid`, or your own) that
  decisions and `explain` use to flag a fallback that crosses classes; see
  [Target classes](routing.md#target-classes).
- `[[select]]`: ordered rules that choose the route from the key, the requested model, headers
  (`x-humpyard-profile`, `x-humpyard-tag-<name>`, or any named header), agent metadata, and what
  the request contains (estimated prompt size, tools, images). First
  match wins; a rule is skipped when the key may not use its route, so clients can narrow but never
  widen. Responses carry `x-humpyard-route` and `x-humpyard-rule`. See
  [Routing](routing.md#selecting-the-route).
- Auto routing (one `auto` model, pool chosen by intent): a tested recipe in
  [examples/auto-routing.toml](examples.md) and
  [Routing](routing.md#auto-routing-recipe).
- `quota_source = "opencode-go"` on a provider (optional), with `quota_poll_secs` (top level,
  default 60, 1 to 3600) and `quota_stale_secs` (default 300, at least the poll interval): the
  gateway reads that provider's published usage windows while serving; see
  [Provider quota](routing.md#provider-quota).
- `[metrics]` (optional, off by default): the Prometheus `/metrics` endpoint. `enabled`, `listen` (a
  separate address; recommended), `bearer_sha256` (the credential for serving it on the main
  listener), `key_labels` and `key_label_limit`. See [Metrics and alerts](metrics.md).
- `decision_retention_days` (top level, default 90): how long routing decisions and provider quota
  samples are kept; see [Keys, budgets and the ledger](budgets.md#the-decision-log).
- `[health]`: optional endpoint circuit breaker (`failure_threshold`, `cooldown_secs`,
  `max_cooldown_secs`); `GET /v1/health` shows which endpoints are being skipped.

Logging is controlled by `RUST_LOG` (default `info`).

**Reloading.** Edit the config file and send SIGHUP (or set `reload_poll_secs` so the gateway
notices by itself, which is how to do it under Kubernetes). The new configuration replaces the old
one atomically; a file that fails to load is rejected and the old one keeps serving, with the
reason in `GET /v1/health`. Running requests finish on the old configuration. `listen`, the ledger
path and `shutdown_grace_secs` need a restart, as does rotating a provider API key. Details in
[Deployment](deployment.md#reloading-the-configuration).

**Shutdown.** SIGTERM or Ctrl-C starts a graceful stop: new connections are refused, requests
already running may finish for up to `shutdown_grace_secs` (default 30; a second signal stops
waiting at once), anything still running is then cancelled (streams end with an error event), and
queued usage entries are written to the ledger before the process exits 0. A kill -9 or crash can
still lose entries that were queued but not yet written. `check-config` validates a file and reports
dangling provider/target references and missing key variables.

Not supported yet: health-driven target eligibility, database-managed keys, an admin API,
`previous_response_id` (returns 400).
