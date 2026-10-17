# auto-routing Specification

## Purpose
TBD - created by archiving change add-auto-routing. Update Purpose after archive.

## Requirements

### Requirement: One client-facing model resolved by rules
An operator SHALL be able to expose a single route that clients always request, whose selector rules choose the serving route from request facts, with a final catch-all rule so every request is routed.

#### Scenario: Intent decides the pool
- **WHEN** a request to the auto route carries a private tag, or is a sub-agent turn, or has a long prompt
- **THEN** it is served by the pool the matching rule names, and the response reports the rule and route

### Requirement: Classification can never fail a request
When a rule delegates the choice to a classifier model, the gateway SHALL apply a timeout and, if the classifier is slow, down or inconclusive, SHALL route to a configured fallback pool instead of failing.

#### Scenario: Classifier unavailable
- **WHEN** the classifier model errors or times out
- **THEN** the request is served by the fallback pool and the decision records that the fallback was used

### Requirement: Strict routes never call a disallowed class
When a route sets `allowed_classes`, the gateway SHALL remove every target whose class is not listed, and every unclassed target, from the route's targets before the routing algorithm runs, so that no failure, retry or tier substitution can reach one. When no allowed target is eligible the request SHALL be refused before any upstream call.

#### Scenario: Strict private route
- **WHEN** a route allows only the `private` class, lists a private and a public target, and the private target fails
- **THEN** the client receives the private target's failure, no public target is called, and the decision lists the public target as excluded

#### Scenario: Default unchanged
- **WHEN** a route sets no `allowed_classes`
- **THEN** it behaves exactly as before, including falling back across classes

#### Scenario: Tier substitution stays inside the allowed set
- **WHEN** a strict route's capable tier has only excluded targets and its efficient tier has allowed ones
- **THEN** the efficient tier's allowed targets serve the request, and no excluded target is used

### Requirement: Strictness is validated when the configuration loads
A route with `allowed_classes` SHALL be rejected at load if a listed class is not a valid label, if any target the route mentions has no class, or if no target of an allowed class remains.

#### Scenario: Unclassed target on a strict route
- **WHEN** a route sets `allowed_classes` and mentions a target without a class
- **THEN** the configuration is rejected and the message names the route and the target

#### Scenario: Route that could never serve
- **WHEN** no target the route mentions has an allowed class
- **THEN** the configuration is rejected

### Requirement: Policy cannot widen a strict route
Health, budgets, selectors and request headers SHALL NOT add a target of a disallowed class to a strict route.

#### Scenario: Cooling-down allowed target
- **WHEN** the only allowed target is cooling down
- **THEN** it is still tried (health never denies service) and no excluded target is added

### Requirement: Exclusions are recorded and explained
The decision SHALL list the targets a route's class list excluded, `GET /v1/decisions` SHALL return them, and `explain` SHALL show each target as allowed or excluded with the reason.

#### Scenario: Explain a strict route
- **WHEN** `explain` describes a request for a strict route
- **THEN** each target shows its class and whether it is allowed, and `fallback_can_cross_classes` is false

### Requirement: Cross-class fallback is visible
When a request is served by a target of a different class than the route's first choice, the decision log and `explain` SHALL flag it.

#### Scenario: Silent fallback made visible
- **WHEN** a private route falls back to a public target
- **THEN** the decision marks the fallback as crossing classes

### Requirement: Choices are explainable
`explain` SHALL show which rule (or classifier verdict) would choose the pool for a described request.

#### Scenario: Explain an auto request
- **WHEN** explain is asked about the auto route with sample headers and size
- **THEN** it reports the rule that fires and the resulting route and targets
