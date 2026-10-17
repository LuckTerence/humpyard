# price-import Specification

## Purpose
TBD - created by archiving change add-capability-eligibility. Update Purpose after archive.

## Requirements

### Requirement: Capability fields are proposed from public data
`humpyard prices import` SHALL also propose `context_window`, `tools` and `vision` for the endpoints it knows, from the same public source, as suggested lines for review, and SHALL NOT change a config file or a running gateway.

#### Scenario: A model with a known window
- **WHEN** the source lists a context limit and tool and image support for an endpoint's model
- **THEN** the output includes suggested `context_window`, `tools` and `vision` for it and nothing is written

### Requirement: Prices are proposed from a source, never applied automatically
The `prices import` command SHALL print price entries for the config's endpoints, derived from a named source, and SHALL NOT modify the config file or the running gateway.

#### Scenario: Import for review
- **WHEN** an operator runs `humpyard prices import` for a config whose provider names a source
- **THEN** it prints a `price` entry per matched endpoint, with the source and the date read, and leaves the config file unchanged

### Requirement: Drift is detectable and machine-readable
The `prices check` command SHALL report, for each endpoint, whether its configured price matches the source, differs, is absent, or whether the source no longer lists the model, SHALL offer JSON output, and SHALL exit with distinct codes for no drift, drift, and a source or config error.

#### Scenario: A price changed at the source
- **WHEN** the source's output price for a configured model differs from the config
- **THEN** the report names the endpoint, both values, and the command exits with the drift code

#### Scenario: A model disappears
- **WHEN** the source no longer lists a configured model
- **THEN** the report flags it as missing and does not alter any price

### Requirement: Source data is untrusted
The importer SHALL fetch over HTTPS with a timeout and a response size limit, SHALL reject any price that is not a finite, non-negative number below a sanity ceiling, SHALL match models by exact id only, and SHALL NOT send provider API keys to a price source.

#### Scenario: Malformed record
- **WHEN** a source record has a negative, non-numeric or absurd price
- **THEN** that record is reported and skipped, and no default is substituted

### Requirement: The gateway does not fetch prices while serving
The serving process SHALL NOT make network requests to price sources; prices come only from its configuration.

#### Scenario: Source unavailable
- **WHEN** the price source is unreachable while the gateway is running
- **THEN** serving, budgets and cost reporting are unaffected

### Requirement: Tiered prices apply by prompt size
An endpoint price MAY define tiers that raise the rates above a prompt-size threshold; a call's cost SHALL use the tier its prompt tokens fall into, and routing policy and budgets SHALL treat the highest rate as the endpoint's price.

#### Scenario: Long prompt
- **WHEN** a call's prompt exceeds a tier threshold
- **THEN** its cost is computed at that tier's rates
