# Design

## Context

`plan_route` narrows a route's targets in a fixed order: the routing policy and `allowed_classes`
(hard eligibility), then health skipping (a soft, fail-open preference). Capability is hard
eligibility of the first kind: a target that cannot take the request will fail it. The difficulty is
that the gateway does not know a model's limits, and its prompt-size estimate is calibrated high on
purpose (3 bytes per token) for budgeting and `count_tokens`.

## Decisions

- **Capabilities are declared per endpoint, unknown means capable.** Endpoints in one target can be
  different models with different windows, so the data lives on `Endpoint`. A field that is absent is
  not a limit. This keeps every existing config unchanged and makes adoption incremental.
- **A target is capable if any endpoint is.** Target-level eligibility is what `plan_route` can decide;
  inside a target the pool still tries endpoints in order, and an incapable endpoint fails over exactly
  as an overflow does today. Skipping single endpoints needs request features at call time and is a
  later change.
- **Exclude only on certainty.** Tools and vision are exact: the request either has them or not, and the
  endpoint either declares `false` or does not. Context uses a new lower-bound estimate
  (`bytes / 6`, against the 3 of the existing estimate) so a removal means "even an optimistic
  tokenizer cannot fit this". Borderline prompts reach the target and the `overflow` fallback, as now.
  A calibration test over representative text, code, JSON and non-Latin fixtures keeps the lower bound
  below real token counts, and the divisor can be tuned from the ledger (estimate versus the provider's
  reported input tokens) without a spec change.
- **Hard eligibility refuses, it does not fail open.** If no target can serve the request, the answer is
  known in advance, so the gateway refuses with 400 `no_capable_target` and a message that names the
  reason. This differs from health (a soft signal that must never deny service) on purpose: a
  declared-false capability is a fact the operator wrote down, and the alternative is an upstream
  failure. Removing a wrong declaration fixes a misconfiguration immediately.
- **Order: classes, policy, capability, then health.** Capability removes targets before health decides
  which of the rest are cold, so health never "keeps" an incapable target to fail open. Strict routes
  stay strict: capability only removes.
- **The estimate is computed when needed.** Today the prompt estimate is built only if a selector rule
  uses `prompt_tokens`. It is also built when any endpoint declares a `context_window`. Requests on
  gateways that declare no windows pay nothing.
- **Recording.** `incapable_targets` is stored like `excluded_targets` and `cold_targets` (nullable JSON,
  ledger schema v6). `explain` adds per-target `capable` with a `reason` and the estimate it used. The
  metric's `reason` label is a fixed set (`tools`, `vision`, `context`); `route` and `target` come from
  configuration (ADR 0016).
- **Importer proposals stay proposals.** models.dev has `limit.context`, `tool_call` and image input for
  many models. `prices import` prints them as suggested lines and never edits a config or the running
  gateway (ADR 0014).

## Risks / trade-offs

- A stale `context_window` (a provider raises its limit) wrongly refuses large prompts. The refusal
  message names the declaration and the importer's `check` mode reports drift, like prices.
- The lower bound lets some doomed requests through; they cost one failed call, as today. That is the
  price of never wrongly excluding a target.
- Capability declarations are per model, not per provider route variant; a provider that serves the same
  model id with a smaller window needs its own endpoint declaration.
