# Design

## Context

Prices are stored as integers of micro-dollars per token and drive budgets, which fail closed
(`budget-enforcement`). A wrong price changes who is blocked and what a request is said to cost, so
the importer must make a wrong price *visible* rather than quietly installing it. Sources are
third-party and untrusted data.

## Decisions

- **Propose, never apply.** `import` prints; the operator reviews and commits through their normal
  config change. `check` only reports. Nothing in the gateway process fetches prices (ADR 0014), so a
  source outage, a malformed file or a hostile response cannot affect serving.
- **Drift matters as much as import.** The first import saves typing; the recurring value is
  noticing that a price changed or a model vanished. `check` is therefore a first-class command with
  stable exit codes (0 no drift, 1 drift, 2 source or config error) and a JSON form.
- **Adapters normalize to one record**: provider, model id, input, output, cached input, tiers, and
  the time the data was read. Adapters never touch the config; a pure function compares records with
  config endpoints, which is what the tests exercise (no network in tests; adapters parse recorded
  fixtures).
- **Treat source data as untrusted.** HTTPS only, a request timeout and a response size cap, strict
  parsing, and every number must be finite, non-negative and below a sanity ceiling; a record that
  fails is reported and skipped, never defaulted. API keys are never sent to a price source.
- **Mapping is explicit.** A provider names its source and id (`price_source =
  "models-dev:opencode-go"`); a model is matched by exact id. No fuzzy matching: an unmatched
  endpoint is reported as unpriced, which is the safe default.
- **Tiers keyed on prompt size.** `tiers = [{ above_tokens = 200000, input, output, cached_input }]`
  selects the rate by the call's prompt tokens, matching how sources describe context tiers. The
  highest price (base or any tier) is what policy and budgets compare, so a long-context model is
  never mistaken for cheap. Time-of-day (peak) pricing is not modelled; the base tier is used and
  the report says so.
- **Provenance in the output.** Generated snippets carry the source, the date read and the source's
  own identifier, so a later reader knows how stale a number is.

## Risks

- A source can be wrong or change shape. Mitigated by review-before-apply, strict parsing, the
  sanity ceiling, and drift reporting that includes "source unreachable" as its own outcome.
- Provider-specific billing (flat-fee plans with dollar caps) makes cost mean "cap used" rather than
  money. The importer supplies list prices; what they mean is the operator's choice, documented in
  budgets.
