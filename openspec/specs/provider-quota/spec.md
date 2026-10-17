# provider-quota Specification

## Purpose
TBD - created by archiving change add-quota-routing. Update Purpose after archive.

## Requirements

### Requirement: Latest provider quota is kept from a published source
A provider MAY name a `quota_source`; while serving, the gateway SHALL poll it on an interval and keep the latest snapshot: for each window its percent used, its status text and reset time, and when it was read.

#### Scenario: Snapshot is available
- **WHEN** the source answers with a usable usage document
- **THEN** the gateway holds a snapshot for that provider with each window's percent, status and reset time, and the time it was fetched

### Requirement: Polling never affects serving
Quota polling SHALL run off the request path, SHALL use a timeout, and SHALL NOT delay, fail or alter any request whether the source is slow, down or returns malformed data.

#### Scenario: Source hangs
- **WHEN** the quota source does not answer
- **THEN** requests are served as before and the snapshot becomes stale

### Requirement: Stale or missing quota is unknown
A snapshot older than `quota_stale_secs`, a provider never polled successfully, and a provider with no quota source SHALL all be reported as unknown, and a quota condition SHALL NOT match on unknown data.

#### Scenario: Data goes stale
- **WHEN** polls keep failing for longer than the staleness limit
- **THEN** the provider's quota is reported as unknown and quota conditions do not match

### Requirement: Source data is untrusted and the credential is scoped
The gateway SHALL accept a percent only as a number from 0 to 100 (clamping and flagging larger values), SHALL drop a window without a usable percent, SHALL treat a document with no usable window as a failed poll, and SHALL send the provider's API key only to the origin of that provider's configured base URL over https.

#### Scenario: Malformed window
- **WHEN** a window's percent is negative, not a number, or missing
- **THEN** that window is dropped and the others are kept

#### Scenario: Key scope
- **WHEN** the gateway polls a quota source
- **THEN** the request goes to the provider's own origin, over https (or loopback http in tests), and the key appears in no log, snapshot or response

### Requirement: Quota is observable
`GET /v1/quota` SHALL return each provider's snapshot with its age, or `unknown` with the reason, for an authenticated caller; it SHALL never include credentials.

#### Scenario: Reading quota
- **WHEN** an authenticated client calls `GET /v1/quota`
- **THEN** it receives each quota-sourced provider's windows, percents, reset times and the snapshot age

### Requirement: Reloads keep the latest snapshot
A configuration reload SHALL keep the latest snapshot of every provider whose quota source and base-URL origin are unchanged.

#### Scenario: Unrelated reload
- **WHEN** the configuration is reloaded without changing a provider's quota source
- **THEN** its snapshot is still available immediately
