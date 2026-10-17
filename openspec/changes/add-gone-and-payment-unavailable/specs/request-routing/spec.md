# Spec Delta

## ADDED Requirements

### Requirement: 410 and 402 describe the endpoint
An upstream answer of HTTP 410 (Gone) or 402 (Payment Required) SHALL be treated as describing the endpoint, not the request: it SHALL count against the endpoint's health like a 5xx, move to the next endpoint of the same target, carry the `unavailable` fallback class, and be restored to its real status if no target answers.

#### Scenario: A pool member answers 410
- **WHEN** the first member of a `fastest` pool route answers 410 and the second answers 200
- **THEN** the request is served by the second member, and the first counts a failure toward its cooldown

#### Scenario: Every member is gone
- **WHEN** every target answers 410
- **THEN** the client receives the real 410, not a 503

#### Scenario: Payment required
- **WHEN** a pool member answers 402 and another member answers 200
- **THEN** the request is served by the other member and the 402 member cools after the failure threshold
