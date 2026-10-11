# Proposal

## Why

Today a client picks a model (a route name) for every conversation, for example in a model picker.
Most agent harnesses send nothing else about the request. The operator already has the pieces to
route by content: selector rules match the key, headers and tags, agent and sub-agent metadata,
prompt size, tools and images (ADR 0011). What is missing is a way to make that the default
experience: one model name the client always uses, with the gateway choosing the pool, and a way to
see and tune those choices from real traffic.

## What Changes

- **An `auto` pattern, built from existing parts.** A documented recipe and example config: one
  client-facing route (for example `auto`) is the catch-all of a rule set that picks a pool by
  intent: private or tagged-private traffic to the private pool, sub-agents and short prompts to a
  cheap pool, long or tool-heavy turns to a capable or long-context pool, with a safe default.
  No new routing engine is needed for this first step.
- **Intent hints from clients.** Standardize and document what a client can send to express intent
  without code changes: the existing `x-humpyard-profile` and `x-humpyard-tag-*` headers, set as
  static provider headers (one provider entry per intent where a client cannot vary headers per
  request).
- **Optional classifier-backed routing.** A route type or rule action `classify` that asks a cheap
  judge model which pool a request needs (reusing the existing `llm_classifier` machinery), with a
  timeout and a deterministic fallback pool, so classification can never block or fail a request.
- **Visibility to tune it.** Depends on the router decision log (see the roadmap): which rule or
  classifier verdict chose the pool, with enough context to compare outcomes. Until then the
  `x-humpyard-rule` header and `explain` are the tools.

Out of scope: learning routing from outcomes automatically, per-conversation stickiness (picks are
per request today; a later change), changes to what clients send.

## Capabilities

### New Capabilities
- `auto-routing`: a single client-facing model that the gateway resolves to a pool by intent.

### Modified Capabilities

## Impact

Mostly documentation and example config in step 1 (no code). Step 2 adds a `classify` action
(config, `select.rs`, server) and tests; step 3 depends on the decision log.
