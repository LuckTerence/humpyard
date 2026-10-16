# Spec Delta

## ADDED Requirements

### Requirement: Capability fields are proposed from public data
`humpyard prices import` SHALL also propose `context_window`, `tools` and `vision` for the endpoints it knows, from the same public source, as suggested lines for review, and SHALL NOT change a config file or a running gateway.

#### Scenario: A model with a known window
- **WHEN** the source lists a context limit and tool and image support for an endpoint's model
- **THEN** the output includes suggested `context_window`, `tools` and `vision` for it and nothing is written
