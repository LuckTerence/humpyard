# Spec Delta

## ADDED Requirements

### Requirement: Pools are explainable
Route explain SHALL report, for a pool route, its strength, its `min_context_window` when set, its members, and each target that carries the strength but was left out with the reason (`context_window_below` or `context_window_undeclared`).

#### Scenario: Explain shows why a target is missing
- **WHEN** a client explains a pool route and a target with the strength was left out for an undeclared window
- **THEN** the report lists that target with the reason `context_window_undeclared`
