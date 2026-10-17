# Proposal

## Why

ModelRelay ranks free models by a blend of speed and quality: each model has a quality score (Artificial
Analysis's coding index when known, an estimate otherwise), and its `auto-fastest` is quality-modulated, so a
tiny quick model does not beat a strong one by being quick. humpyard's `fastest` route (ADR 0020) orders by
speed alone, and its pools hold targets by strength and context window, so on a mixed free pool the fastest
model can be a weak one. The answers are visibly worse. Keeping comparable models in one pool by hand works
but is the kind of tending this work is meant to remove. Item 3 of the gaps against ModelRelay.

## What Changes

- **A declared `quality`** on a target: a number from 0 to 1 that the operator states (a benchmark score they
  trust). It is never fetched at runtime and may be absent (unknown).
- **A quality floor on pools**: `pool = { strength = "general", min_quality = 0.6 }` keeps only targets that
  declare at least that quality, resolved when the configuration loads, like `min_context_window` (an
  undeclared quality does not qualify). Explain lists targets left out and why.
- **A quality weight on `fastest` routes**: `quality_weight` from 0 to 1 (default 0, which is exactly today's
  speed ordering). With a weight above 0 a measured target's score blends a speed score (how its observed time
  compares with `latency_target_ms`, default 2000) and its quality (an unknown quality counts as
  `unknown_quality`, default 0.5): `score = (1 - w) * speed + w * quality`. Targets are tried in descending
  score, ties by speed then written order. Unmeasured targets are still tried first so they get measured, and
  exploration is unchanged.
- **The migration carries ModelRelay's scores honestly.** `migrate-modelrelay` writes `quality` from a
  model's score only when ModelRelay's own data marks it as measured (not estimated); estimated scores are
  left as a comment (`--estimated-quality` includes them). On the reference instance 23 of 53 are measured.
- **Visible**: `explain` shows each target's quality and, for a route with a weight, the score and its
  parts; the decision log's recorded order is unchanged in shape.
- **Out of scope:** fetching benchmark data at runtime or from a bundled table, learning quality from
  outcomes, per-task or per-strength quality, using quality outside `fastest` routes and pool membership.

## Capabilities

### Modified Capabilities
- `gateway-config`: `quality` on targets, `min_quality` on pools.
- `route-selectors`: `quality_weight` and `latency_target_ms` on `fastest` routes.
- `cli`: the migration carries measured scores.

## Impact

`src/config` (target `quality`, pool `min_quality`, `fastest` settings), `src/latency.rs` and `src/routing.rs`
(the blended order), `src/server/explain.rs`, `src/migrate.rs`. Docs: `routing.md`, `configuration.md`,
`migrating-from-modelrelay.md`. No ledger or metrics change.
