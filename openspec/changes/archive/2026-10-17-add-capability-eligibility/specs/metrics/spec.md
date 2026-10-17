# Spec Delta

## ADDED Requirements

### Requirement: Capability removals are counted
The gateway SHALL count, per route, target and reason, requests in which a target was removed for capability, as `humpyard_capability_skips_total{route,target,reason}`. `route` and `target` SHALL be configured names and `reason` one of `tools`, `vision` or `context`.

#### Scenario: An oversized prompt
- **WHEN** a target is removed because the prompt exceeds its window
- **THEN** the counter for that route, target and `context` increases by one
