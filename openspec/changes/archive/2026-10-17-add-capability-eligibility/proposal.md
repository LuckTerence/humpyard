# Proposal

## Why

A request can only be served by a target whose model can take it: it must fit the context window, and
a request that uses tools or images needs a model that supports them. Today humpyard learns this the
hard way. An oversized prompt is sent to a target whose window it exceeds, the provider answers with a
context error, and the `overflow` fallback moves to the next target: a wasted round trip, a recorded
failure, and (on a free tier) a call counted against a limit. A request with tools sent to a model that
does not support them can fail, or worse, be answered without the tools and look fine. Switchyard
discussion #427 lists the same need ("capability invariants") as a pre-routing eligibility rule, and the
router literature's "right-sizing the task to the model" starts from knowing what a model can take.

Eligibility already has a home. Strict routes (ADR 0017) and health skipping (ADR 0018) remove targets in
`plan_route` before the algorithm runs. A target that cannot serve the request is the same kind of
removal.

## What Changes

- **Declared capabilities on an endpoint.** Optional `context_window` (tokens), `tools` (bool) and
  `vision` (bool). An endpoint that declares nothing is assumed capable, so existing configs behave
  exactly as before.
- **Incapable targets are removed before routing.** A target is removed for a request when none of its
  endpoints can serve it: the request uses tools and every declared endpoint says `tools = false`, it
  carries images and every declared endpoint says `vision = false`, or the prompt is certainly larger
  than every declared window.
- **Only when certain.** The prompt-size check uses a deliberately low estimate (the existing estimate
  errs high, which would exclude targets that fit). A borderline prompt still goes to the target and the
  existing `overflow` fallback handles it, so the feature never excludes a target that could have served.
- **A clear refusal when nothing can.** If no target of the route can serve the request, it is refused
  before any upstream call with HTTP 400 `no_capable_target`, naming the reason (tools, images or
  context), instead of a guaranteed upstream failure.
- **Visible.** Decisions list `incapable_targets` (ledger schema v6), `explain` shows each target's
  capability and the reason it was removed, and `humpyard_capability_skips_total{route,target,reason}`
  counts removals.
- **Proposed from public data.** `humpyard prices import` also proposes `context_window`, `tools` and
  `vision` from models.dev for review, never applied automatically (ADR 0014).
- **Out of scope:** skipping individual endpoints inside a target, output-length (`max_tokens`) limits,
  learning capabilities from failures, probing providers at runtime, and cost or quality preferences.

## Capabilities

### Modified Capabilities
- `routing-policy`: capability eligibility and the `no_capable_target` refusal.
- `metrics`: a capability skip counter.
- `price-import`: proposing capability fields.

## Impact

`src/config` (endpoint fields and validation), `src/estimate.rs` (a lower-bound estimate), `src/server.rs`
(`plan_route`, request features, the prompt estimate computed whenever a window is declared),
`src/error.rs` (the refusal), `src/ledger` (schema v6), `src/server/decisions.rs`, `explain.rs`,
`src/metrics.rs`, `src/prices.rs` (proposals). Docs: configuration, routing, api, invariants,
architecture, roadmap, the metrics examples and the price-import page.
