# Spec Delta

## ADDED Requirements

### Requirement: Fastest route
The gateway SHALL support a route type `fastest` with `targets`, optional `explore` (a probability from 0 to 1, default 0.1), `min_samples` (default 3), `max_age_secs` (default 600), `seed`, `fallback_on` and `allowed_classes`. For each request it SHALL order the targets that remain eligible after class, policy, capability and health subtraction by their observed time to answer, fastest first, and attempt them in that order with the route's fallback.

#### Scenario: The faster target is tried first
- **WHEN** a `fastest` route has targets `a` and `b`, both measured, and `b` answers faster
- **THEN** requests try `b` first and `a` only if `b` fails over

#### Scenario: Unmeasured targets are tried early
- **WHEN** a target has fewer than `min_samples` answered attempts, or its figure is older than `max_age_secs`
- **THEN** it is ordered before measured targets, in the route's written order

#### Scenario: Exploration
- **WHEN** `explore` is 0.1 and the seeded draw selects exploration for a request
- **THEN** a random eligible target is tried first and the rest follow in speed order, and `explore = 0` never explores

#### Scenario: Ordering never adds a target
- **WHEN** the fastest target is excluded by class, policy, capability or health
- **THEN** it is not tried, however fast its figure

### Requirement: What counts as a sample
The gateway SHALL update a target's observed time only from answered attempts and SHALL keep it in memory as a smoothed figure with a sample count. Failed attempts SHALL NOT change it.

#### Scenario: A failure is not a fast sample
- **WHEN** an attempt on a target times out or returns a server error
- **THEN** the target's figure and sample count are unchanged

### Requirement: Order is visible
The decision log SHALL record the order a `fastest` route used and whether the first target was chosen as fastest or by exploration, and route explain and statistics SHALL show each target's current figure and sample count.

#### Scenario: Explain shows the figures
- **WHEN** a client explains a `fastest` route
- **THEN** each target lists its smoothed time and sample count, and the order a request would use now
