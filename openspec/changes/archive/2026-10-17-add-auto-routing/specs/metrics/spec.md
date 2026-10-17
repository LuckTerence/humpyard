# Spec Delta

## ADDED Requirements

### Requirement: Blocked fallbacks are counted
The gateway SHALL count, per route, requests that ended in an upstream failure on a route whose `allowed_classes` excluded one or more targets, as `humpyard_fallback_blocked_total{route}`. The label SHALL be a configured route name.

#### Scenario: Strict route fails
- **WHEN** a request fails on a strict route that excluded a public target
- **THEN** `humpyard_fallback_blocked_total` for that route increases by one

#### Scenario: Strict route succeeds or route is not strict
- **WHEN** a request on a strict route succeeds, or fails on a route with no exclusions
- **THEN** the counter does not change
