# Spec Delta

## ADDED Requirements

### Requirement: Answers that describe the endpoint count as failures
An upstream answer of 401, 403 or 404, or of 400 or 404 whose error code or type is `model_not_found`, `model_not_available` or `model_decommissioned`, SHALL count as a failure of that endpoint for health, exactly as a 5xx does. A 400 or 422 otherwise SHALL NOT count. A success SHALL reset the count.

#### Scenario: A removed model cools
- **WHEN** an endpoint answers 404 `failure_threshold` times in a row
- **THEN** it is cooling down, is skipped while another target remains, and is probed after the cooldown

#### Scenario: A request error does not count
- **WHEN** an endpoint answers 400 (not a model-gone code) or 422
- **THEN** the endpoint's health is unchanged

#### Scenario: A stray failure does not cool a good endpoint
- **WHEN** an endpoint that usually succeeds answers one 403
- **THEN** it keeps serving

#### Scenario: Health never denies service
- **WHEN** every endpoint of a route is cooling because of these answers
- **THEN** the route is still tried, as with any cooling endpoint
