# Spec Delta

## ADDED Requirements

### Requirement: Declared quality
A target MAY declare `quality`, a number from 0 to 1. A pool MAY set `min_quality`; its members SHALL be the targets that carry the strength, satisfy any `min_context_window`, and declare a `quality` of at least `min_quality`. A target with no declared quality SHALL NOT qualify, and SHALL be reported as left out with the reason `quality_undeclared` (or `quality_below`).

#### Scenario: A quality floor
- **WHEN** targets declare qualities 0.7, 0.5 and none, and a pool sets `min_quality = 0.6`
- **THEN** only the first is a member, and explain lists the others with `quality_below` and `quality_undeclared`

#### Scenario: Out of range
- **WHEN** a quality is below 0 or above 1, or `min_quality` is
- **THEN** the configuration is rejected naming the target or route
