# Spec Delta

## ADDED Requirements

### Requirement: Cache tokens are counted
`humpyard_tokens_total` SHALL also carry the `direction` values `cached_input` and `cache_creation`, so the cache-hit ratio can be derived in a dashboard. Session ids SHALL NOT be used as a metric label.

#### Scenario: A cached call
- **WHEN** a call reports cached input tokens
- **THEN** the `cached_input` series for its route and target increases by that number
