# Spec Delta

## ADDED Requirements

### Requirement: Session queries are indexed
The ledger SHALL keep an index over usage rows by session id and time, added by an in-place, idempotent schema upgrade that preserves every existing row.

#### Scenario: Upgrading an older database
- **WHEN** a database at the previous schema version is opened
- **THEN** the index exists afterwards and all earlier rows are intact
