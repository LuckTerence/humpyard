# Spec Delta

## ADDED Requirements

### Requirement: 413 is overflow and does not count against health
An upstream answer of HTTP 413 SHALL carry the `overflow` fallback class, SHALL move to the next endpoint of the same target and, where the route's guard allows `overflow`, to the next target, SHALL NOT count against the endpoint's health, and SHALL be restored to its real status and message if no target answers.

#### Scenario: A pool member is too small for the request
- **WHEN** the first member of a `fastest` pool route answers 413 and the second answers 200
- **THEN** the request is served by the second member

#### Scenario: A 413 does not cool the endpoint
- **WHEN** an endpoint answers 413 on more consecutive requests than the failure threshold
- **THEN** it is still asked on the next request and is not cooling down

#### Scenario: Every member is too small
- **WHEN** every pool member answers 413 and the route has a paid tail whose guard lists `overflow`
- **THEN** the request is served by the tail

#### Scenario: Nothing takes the request
- **WHEN** every target answers 413
- **THEN** the client receives 413 with the provider's message
