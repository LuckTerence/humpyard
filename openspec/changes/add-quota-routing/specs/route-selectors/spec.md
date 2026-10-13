# Spec Delta

## ADDED Requirements

### Requirement: Quota condition
A selector's `when` MAY contain `quota = { provider, window, at_least | below }`, matching when the named provider's named window (`rolling`, `weekly` or `monthly`) has a known percent used that is at least, or below, the given value; an unknown quota SHALL NOT match.

#### Scenario: Pacing a scarce provider
- **WHEN** a rule requires the weekly window of a provider to be at least 70 and the snapshot says 82
- **THEN** the rule matches and its route serves the request

#### Scenario: Unknown quota
- **WHEN** the provider has no fresh snapshot
- **THEN** the quota condition is false and selection continues with the next rule

### Requirement: Quota conditions are explainable and validated
`explain` SHALL show the quota state each quota condition saw and whether it matched, decisions SHALL record when a quota rule selected the route, and a quota condition SHALL be rejected at configuration time when its provider has no `quota_source`, its window is unknown, or its bound is outside 0 to 100 (or it names both or neither bound).

#### Scenario: Explain a quota rule
- **WHEN** an operator explains a hypothetical request that a quota rule would match
- **THEN** the report names the rule, the provider and window, the percent seen and its age

#### Scenario: Invalid condition
- **WHEN** a rule's quota condition names a provider without a quota source
- **THEN** the configuration is rejected with the rule and provider named
