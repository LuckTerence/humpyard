# Proposal

## Why

Cost per request, budgets and any cost-aware routing rest on endpoint prices, and today an operator
types every `price = { input, output, cached_input }` by hand from a provider's web page. That is
slow, goes stale silently, and was already wrong once in practice: a model was assumed retired
because a docs page omitted it while the provider's API still listed it. Prices and model lists are
published in machine-readable form (for example the open models.dev dataset, which carries per-token
prices and context tiers for the common providers, including OpenCode Go and OpenCode Zen), so the
data can be fetched and *checked* instead of copied.

## What Changes

- **`humpyard prices import`.** A CLI command that reads a price source and prints the `price`
  entries for the endpoints in a config, as a TOML snippet or a reviewable diff against the current
  file. It never edits the file or the running gateway.
- **`humpyard prices check`.** Compares a config's prices with a source and reports drift: a price
  that changed, an endpoint with no price, a model the source no longer lists. Human or `--json`
  output, and a non-zero exit when there is drift, so a scheduled job (a CronJob, CI) can alert.
- **A source registry.** One small adapter per source behind a common shape. The first adapter is
  models.dev; others (OpenRouter's models API, a hand-maintained table for providers that publish
  only a web page) are added the same way. Each provider in a config may name its source
  (`price_source = "models-dev:opencode-go"`).
- **Tiered prices.** `price` gains optional `tiers` (a higher rate above a prompt-size threshold),
  because the common sources publish them and one flat price under-reports long-context calls.
- **Out of scope:** the gateway fetching prices at runtime or applying them on its own (see the ADR),
  provider quota polling (a later proposal; some providers expose remaining quota, which would let
  routing avoid a nearly exhausted pool), and cost-aware ordering.

## Capabilities

### New Capabilities
- `price-import`: fetch prices from a source, propose them for review, and detect drift.

### Modified Capabilities
- `usage-ledger`: a call's cost uses the tier its prompt size falls into.
- `gateway-config`: `price.tiers` and the per-provider `price_source`.

## Impact

New `src/prices.rs` and a `prices` subcommand in `src/main.rs`; config schema and validation
(`tiers`, `price_source`); metering applies the tier. The only new network use is in the CLI
subcommand, never in `serve`. Docs: configuration, budgets, deployment (the scheduled check).
