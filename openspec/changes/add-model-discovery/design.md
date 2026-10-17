# Design

## Context

`prices` shows the pattern: an operator command fetches the open models.dev dataset over https with a size
limit and strict parsing, proposes or reports, and `serve` never fetches (a test enforces it). models.dev
records, per model, the context limit, `tool_call`, `reasoning`, input and output modalities, `release_date`
and `cost`, and covers most providers (some thinly: it lists 2 Cerebras models where ModelRelay knows 5).
Providers' own `GET /models` is the ground truth for what a key can use, but carries no price.

## Decisions

- **Two sources must agree.** A model is added only when models.dev says it is a free chat model (cost 0 for
  input and output, text output, a context window) and the provider's `/models` lists its id for the
  operator's key. Either source alone can be wrong or poisoned; the intersection bounds the damage to models
  the provider really offers to this key and the dataset says cost nothing. `discover` may set `confirm =
  false` for a provider with no usable `/models`; that is stated as weaker in the guide.
- **A generated file, not an edit of the config.** `models sync` owns one file and rebuilds it from scratch
  each run, so removals are automatic and nothing hand-written is ever rewritten. `serve` merges it as extra
  `[targets.auto-*]`; pools resolve over the merged set, so `strengths` and the `all` catch-all work unchanged.
- **Strict loader that fails safe.** The file may contain only targets named `auto-*`, on providers that exist
  and have `discover` set, with at most 500 targets, no duplicate of a hand-written (provider, model) pair, and
  every endpoint priced at zero. Any violation ignores the whole file (logged, counted by a gauge): the cost of
  a bad generated file is a stale pool, never an outage or a paid route.
- **Atomic and idempotent.** Written to a temp file in the same directory and renamed; not rewritten when the
  content is unchanged, so the reload poll does not churn. The previous version is kept as `.prev` for one
  generation for rollback.
- **Reload sees both files.** The reload fingerprint covers the main config text and the generated file's
  text, so a new generated file reloads the gateway exactly as an edit would, and an unchanged pair does not.
- **Strengths are the operator's default.** Providers do not report `coding`, `fast` or `agentic`.
  `discover = { strengths = ["general"] }` says what new models from this provider are treated as; `reasoning`
  is added when the source flags it; `all` is always added. A hand edit of a generated target is not possible
  (it is regenerated); to label one, write a hand target for the same (provider, model) and it is no longer
  generated.
- **Guardrails are numbers with defaults.** `max_added_per_run = 10`, `max_removed_fraction = 0.5` (refuse a run
  that would drop more than half of a non-trivial generated set, which catches an empty or truncated source),
  and a floor: at least one source model per opted-in provider or the provider is skipped and reported.
- **Where it runs is not the gateway's concern, and the simplest place is the same machine.** The one-shot
  `models sync` is the primary form: any scheduler (cron, a systemd timer, launchd, Task Scheduler) that can
  run a command on the host where the gateway runs, writing the file the gateway reads; the gateway's existing
  reload poll picks it up, so there is no shared storage, no signal and no second service. For a container, the
  scheduler lives on the host and the generated file is a bind mount. `models sync --every 6h` loops
  in-process for setups with no scheduler (a second container, a Kubernetes sidecar, which also avoids the
  shared storage a CronJob would need). The guide gives each recipe and says which to choose. Running it
  inside `serve` is a later opt-in built on the same module.
- **Keys.** `models sync` and `check` read provider keys from the same `api_key_env` variables as `serve`, send
  them only to the provider's own `base_url` host over https (or loopback http), and send none to models.dev.

## Risks / trade-offs

- Applied without review is a real change from ADR 0014. The scope that makes it acceptable is narrow: free
  models, opted-in providers, a bounded delta, a fail-safe loader, visible `auto-` names, and a kill switch.
- "Free" means zero per-token cost in the dataset, not unlimited: a free tier has rate limits that health
  skipping and `Retry-After` handling absorb. The guide says so.
- The dataset can lag a launch or omit a provider's models; a provider-listed model the dataset lacks is not
  added (agreement required). That is the conservative side.
