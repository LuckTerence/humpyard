# Spec Delta

## ADDED Requirements

### Requirement: Quality-weighted ordering
A `fastest` route SHALL accept `quality_weight` from 0 to 1 (default 0) and `latency_target_ms` (default 2000, at least 1). With a weight of 0 the order SHALL be exactly the speed ordering. With a weight `w` above 0, each target with a trustworthy speed figure SHALL be ranked by `score = (1 - w) * speed + w * quality`, where `speed = min(1, latency_target_ms / observed_ms)` and `quality` is the declared quality, or the route's `unknown_quality` (default 0.5) when none is declared; ties SHALL break by observed speed, then written order. Targets with too few samples or a stale figure SHALL still be tried first, and exploration SHALL be unchanged.

#### Scenario: Weight zero is today's behavior
- **WHEN** a route has no `quality_weight`
- **THEN** its order is the speed ordering, whatever the targets' qualities

#### Scenario: Quality outweighs a small speed gap
- **WHEN** weight is 0.7 and a target with quality 0.8 answers in 600 ms while one with quality 0.4 answers in 300 ms (target 2000 ms)
- **THEN** the first is tried first

#### Scenario: A very slow strong model does not lead
- **WHEN** a quality 0.9 target answers in 20000 ms and a quality 0.6 target in 800 ms, with weight 0.5
- **THEN** the faster, weaker target is tried first

#### Scenario: Explain shows the parts
- **WHEN** a client explains a route with a weight
- **THEN** each target shows its quality, speed score and score
