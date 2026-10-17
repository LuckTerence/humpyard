# Spec Delta

## ADDED Requirements

### Requirement: A cold target is skipped when an alternative exists
When every endpoint of a target is cooling down, the gateway SHALL leave that target out of the route's targets for a request, provided at least one other target remains after classes and the routing policy have narrowed the route. Such a target SHALL NOT receive an attempt from that request.

#### Scenario: Healthy alternative
- **WHEN** a route lists a cold target and a warm target
- **THEN** the request is served by the warm target and the cold target's upstream receives no call

#### Scenario: A cold tier is substituted
- **WHEN** every target of a route's `capable` tier is cold and its `efficient` tier is warm
- **THEN** the efficient targets serve the request, as with any emptied tier

### Requirement: Health never denies service
When every target that classes and the routing policy leave eligible is cold, the gateway SHALL NOT skip any of them: the route SHALL be tried as it would be without health skipping, and the request SHALL NOT be refused because of health.

#### Scenario: Everything is cold
- **WHEN** every eligible target is cooling down
- **THEN** the targets are tried in order and a recovered target serves the request

### Requirement: A cooled target is re-admitted by its probe
A target SHALL stop being cold when its cooldown ends. The next request that reaches it SHALL be admitted as the single probe, and a success SHALL return it to service.

#### Scenario: Recovery
- **WHEN** a cold target's cooldown ends and its upstream is healthy
- **THEN** the next request is sent to it as a probe and, on success, later requests use it normally

### Requirement: Health skipping cannot widen a strict route
Health skipping SHALL apply only within the targets a route's `allowed_classes` and the routing policy leave eligible, and SHALL NOT add a target.

#### Scenario: Strict route with one cold allowed target
- **WHEN** a strict route's only allowed target is cold
- **THEN** it is still tried (health never denies service) and no excluded target is called

### Requirement: Skips are recorded and explained
The decision SHALL list the targets skipped for health, `GET /v1/decisions` SHALL return them, and `explain` SHALL show each target as cold or warm and report when a route would fail open.

#### Scenario: Explain with a cold target
- **WHEN** `explain` describes a request for a route with a cold target and a warm one
- **THEN** the cold target is marked cold and the plan lists only the warm one

### Requirement: Skipping can be turned off
With `[health] skip_cold_targets = false` the gateway SHALL not skip cold targets, and routes SHALL behave as before this change.

#### Scenario: Switch off
- **WHEN** `skip_cold_targets` is false and a route's first target is cold
- **THEN** that target is still tried first
