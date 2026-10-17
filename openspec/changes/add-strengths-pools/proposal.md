# Proposal

## Why

The two free pools ModelRelay backs on the reference deployment are "the fastest free model with at least N
tokens of context" and "the best free general model". After honoring `Retry-After` and ordering by speed
(steps 1 and 2), the remaining gap is the pool itself: today an operator lists every member by hand, and a new
free model has to be added to every route that should use it. ModelRelay solves this with tags. Step 3 of
"Replacing ModelRelay", and the soft half of "capabilities and strengths" on the roadmap.

## What Changes

- **A target can carry `strengths`**: free-form lowercase labels such as `coding`, `reasoning`, `general`,
  `fast`, `agentic`. They are soft facts about what a model is good at. They are not capabilities (which are
  hard, declared on endpoints and enforced on certainty) and not request tags (`x-humpyard-tag-*`, which are
  about the request).
- **A pool route**: a `passthrough`, `fastest` or `random` route may take `pool = { strength = "general",
  min_context_window = 32000 }` instead of `targets`. Its targets are every target that carries the strength
  (and, when `min_context_window` is set, has an endpoint that declares at least that window), **resolved
  when the configuration loads or reloads**, in target-name order. Nothing is evaluated per request, so the
  membership is deterministic and explainable.
- **Strengths never remove a target at request time.** They only decide what a pool contains. A wrong label
  costs a slightly worse pool, never a refused request.
- **Visible**: `route/explain` shows a pool route's strength, window, members and any target that has the
  strength but was left out and why; an empty pool is a configuration error that names the strengths that
  do exist.
- **Out of scope:** preferring among pool members by strength, selecting a strength per request, assigning
  strengths from a catalog (step 4), importing from ModelRelay (step 5), pools by more than one strength, and
  any change to capability or health removal.

## Capabilities

### Modified Capabilities
- `gateway-config`: target strengths and pool routes.
- `route-selectors`: pool membership shown by explain.

## Impact

`src/config/{schema,mod,validate}.rs` (strengths, `pool`, resolution), `src/server/explain.rs` (pool report).
Docs: `routing.md`, `configuration.md`, `roadmap.md`. No ledger, metric or API change.
