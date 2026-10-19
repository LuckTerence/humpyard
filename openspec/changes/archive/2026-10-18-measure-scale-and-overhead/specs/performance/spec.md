# Spec Delta

## ADDED Requirements

### Requirement: A reproducible ledger scale harness
The repository SHALL contain a harness, excluded from the default test run, that fills a ledger with a configurable number of seeded synthetic usage, decision and quota rows and reports the time of startup budget hydration, per-key spend lookups, the stats queries grouped by route, target and endpoint over a one-day and a thirty-day window, the session scan, the decision prune, and sustained writes through the ledger queue, together with the database size.

#### Scenario: A run is reproducible
- **WHEN** the harness is run twice with the same row count and seed
- **THEN** it generates identical rows and reports the same row counts and database content

#### Scenario: The default test run is unaffected
- **WHEN** the default test suite is run
- **THEN** the harness is not executed

#### Scenario: The shape is visible
- **WHEN** the harness is run at 100000 and at 1000000 rows
- **THEN** each report states its row count so the growth of each timing can be compared

### Requirement: A gateway overhead harness
The repository SHALL contain a harness, excluded from the default test run, that sends the same requests directly to a mock upstream and through the gateway at concurrency 1, 16 and 64, with and without a fixed upstream delay, and reports p50, p90 and p99 latency for each path, the difference between them, and requests per second.

#### Scenario: Direct and gateway paths are compared
- **WHEN** the harness runs against an instant mock upstream
- **THEN** it prints the latency percentiles for both paths and their difference at each concurrency

#### Scenario: Upstream time is separated
- **WHEN** the harness runs against a mock upstream with a fixed delay
- **THEN** the reported overhead excludes the delay

### Requirement: Performance statements rest on recorded measurements
The documentation SHALL record each measurement run in `docs/performance.md` with its date, version, machine, command and results and SHALL state its limits, and SHALL NOT make a performance or capacity claim elsewhere without linking to a recorded measurement. The documentation SHALL state, for each ledger table, whether its rows are pruned and by which setting.

#### Scenario: A claim has a source
- **WHEN** any document states a latency, throughput or scale figure
- **THEN** it links to the recorded measurement it comes from

#### Scenario: Retention is stated per table
- **WHEN** the performance page is read
- **THEN** it says for `usage`, `decisions` and `quota_samples` whether they are pruned and which setting controls it
