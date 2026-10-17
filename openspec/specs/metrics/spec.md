# metrics Specification

## Purpose
TBD - created by archiving change add-health-eligibility. Update Purpose after archive.

## Requirements

### Requirement: Health skips are counted
The gateway SHALL count, per route and target, requests in which a target was skipped because it was cold, as `humpyard_health_skips_total{route,target}`. Both labels SHALL be configured names.

#### Scenario: A cold target is skipped
- **WHEN** a request on a route skips a cold target
- **THEN** the counter for that route and target increases by one

#### Scenario: Nothing skipped
- **WHEN** no target is cold, or every eligible target is cold (fail open)
- **THEN** the counter does not change

### Requirement: Capability removals are counted
The gateway SHALL count, per route, target and reason, requests in which a target was removed for capability, as `humpyard_capability_skips_total{route,target,reason}`. `route` and `target` SHALL be configured names and `reason` one of `tools`, `vision` or `context`.

#### Scenario: An oversized prompt
- **WHEN** a target is removed because the prompt exceeds its window
- **THEN** the counter for that route, target and `context` increases by one

### Requirement: Cache tokens are counted
`humpyard_tokens_total` SHALL also carry the `direction` values `cached_input` and `cache_creation`, so the cache-hit ratio can be derived in a dashboard. Session ids SHALL NOT be used as a metric label.

#### Scenario: A cached call
- **WHEN** a call reports cached input tokens
- **THEN** the `cached_input` series for its route and target increases by that number
