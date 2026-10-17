# Spec Delta

## ADDED Requirements

### Requirement: Health skips are counted
The gateway SHALL count, per route and target, requests in which a target was skipped because it was cold, as `humpyard_health_skips_total{route,target}`. Both labels SHALL be configured names.

#### Scenario: A cold target is skipped
- **WHEN** a request on a route skips a cold target
- **THEN** the counter for that route and target increases by one

#### Scenario: Nothing skipped
- **WHEN** no target is cold, or every eligible target is cold (fail open)
- **THEN** the counter does not change
