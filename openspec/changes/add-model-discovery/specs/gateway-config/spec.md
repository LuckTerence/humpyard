# Spec Delta

## ADDED Requirements

### Requirement: Two switches
Discovery SHALL be off unless `[discovery] enabled` is true (which requires `file`) and the provider has `discover` with `enabled` true (the default). With the master switch off the generated file SHALL NOT be read and `models sync` SHALL refuse to write; a provider with `enabled = false` SHALL have its generated targets skipped by the loader at once and removed by the next sync.

#### Scenario: Master switch off
- **WHEN** `[discovery] file` is set and `enabled` is false or absent
- **THEN** the file is not merged, the state is reported as disabled, and `models sync` exits 2 naming the switch

#### Scenario: One provider opts out
- **WHEN** two providers have generated targets and one sets `discover.enabled = false`
- **THEN** the other provider's targets still load and the opted-out provider's are skipped

### Requirement: Discovery opt-in and settings
A provider MAY set `discover = { strengths = [...], confirm = true }` (requires `price_source`), and the configuration MAY have `[discovery]` with `file`, `class` (default `public`), `max_added_per_run` (default 10), `max_removed_fraction` (default 0.5) and `exclude` (a list of `provider/model` ids).

#### Scenario: Opting in without a source
- **WHEN** a provider sets `discover` but not `price_source`
- **THEN** the configuration is rejected naming the provider

### Requirement: The generated file is merged, strictly, and fails safe
When `[discovery] file` names a file, `serve` SHALL parse it at load and on every reload and merge its targets, accepting it only if it contains nothing but `[targets.auto-*]` on providers that exist and have `discover` set, no more than 500 targets, no (provider, model) pair that a hand-written target already uses, and every endpoint priced at zero. A file that does not meet this, or is missing or empty, SHALL be ignored with a log line and a gauge, and the gateway SHALL serve with the main configuration alone. `serve` SHALL NOT fetch any data to do this.

#### Scenario: A valid generated file
- **WHEN** the file holds `auto-groq-x` with the strength `general`
- **THEN** a pool of `general` includes it after the next reload

#### Scenario: An invalid generated file
- **WHEN** the file contains a table other than a target, or a priced endpoint
- **THEN** it is ignored in full and the gateway serves with the main configuration

#### Scenario: Reload sees the file
- **WHEN** only the generated file changes
- **THEN** the gateway reloads

### Requirement: Generated targets are visible
Generated targets SHALL be named `auto-*`, SHALL be marked as generated in `route/explain`, and the number loaded and the file's state SHALL be exposed as metrics.

#### Scenario: Explain marks them
- **WHEN** a client explains a route whose pool includes a generated target
- **THEN** the target is reported with `generated = true`
