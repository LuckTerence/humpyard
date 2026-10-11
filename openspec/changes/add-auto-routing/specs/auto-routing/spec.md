# Spec Delta

## ADDED Requirements

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

### Requirement: Choices are explainable
`explain` SHALL show which rule (or classifier verdict) would choose the pool for a described request.

#### Scenario: Explain an auto request
- **WHEN** explain is asked about the auto route with sample headers and size
- **THEN** it reports the rule that fires and the resulting route and targets
