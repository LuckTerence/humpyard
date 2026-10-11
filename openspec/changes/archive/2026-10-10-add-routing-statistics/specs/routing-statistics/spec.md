# Spec Delta

## ADDED Requirements

### Requirement: Performance statistics per route, target and endpoint
The gateway SHALL answer `GET /v1/stats` with, for each group (route, target or endpoint, as `group_by` selects) over the requested window: the number of requests and attempts, the attempt outcome counts by class, the fallback rate, attempt latency percentiles (p50, p90, p99), and the average cost and tokens per request.

#### Scenario: Failure mix is visible
- **WHEN** a target returned rate limits for part of the window
- **THEN** its group shows the `rate_limit` count and its share of attempts

#### Scenario: Latency percentiles
- **WHEN** attempts took known times
- **THEN** the reported p50, p90 and p99 match those times by the nearest-rank method

### Requirement: Statistics follow the decision log's read model
A key SHALL see statistics over its own requests only, an open gateway over all, and the window SHALL be bounded by the decision retention.

#### Scenario: Isolation
- **WHEN** two keys have made requests
- **THEN** each key's statistics count only its own

### Requirement: Statistics are read-only
Computing or reading statistics SHALL NOT change routing, and an empty window SHALL return empty groups rather than an error.

#### Scenario: Empty window
- **WHEN** no requests fall inside the window
- **THEN** the response is 200 with no groups

### Requirement: Explain shows recent behavior
When a ledger is configured, `explain` SHALL report each eligible target's recent failure rate and p50 latency (over the default window) alongside its health.

#### Scenario: Annotated plan
- **WHEN** explain is asked about a route whose first target has been failing
- **THEN** the report shows that target's failure rate
