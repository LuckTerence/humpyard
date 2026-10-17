# Spec Delta

## ADDED Requirements

### Requirement: Failover on unavailable endpoints
A 401 or 404 from an endpoint SHALL move the request to the next endpoint of the same target for every route. A fallback class `unavailable` (401 and 404) SHALL move a request to the next target; it SHALL be included by default for `fastest` routes and pool routes, SHALL NOT be included by default for any other route, and SHALL be taken exactly as written when a route lists its `fallback_on` classes.

#### Scenario: A dead first target in a fastest pool
- **WHEN** a `fastest` pool's first target answers 404 and another target is healthy
- **THEN** the request is served by the healthy target, and after `failure_threshold` such answers the dead target is skipped

#### Scenario: Other routes keep their behavior
- **WHEN** a `passthrough` route without `unavailable` in `fallback_on` has a first target that answers 404
- **THEN** the 404 is returned to the client and the second target is not called

#### Scenario: Opting in
- **WHEN** a route lists `fallback_on = ["unavailable"]` and its first target answers 401
- **THEN** the request moves to the next target

#### Scenario: Within a target
- **WHEN** a target has two endpoints and the first answers 404
- **THEN** the second is tried, on every route

#### Scenario: Strict routes
- **WHEN** an unavailable target is on a route with `allowed_classes`
- **THEN** fallback goes only to targets of an allowed class

#### Scenario: The client still gets an error when nothing works
- **WHEN** every target answers 404
- **THEN** the client receives the last 404
