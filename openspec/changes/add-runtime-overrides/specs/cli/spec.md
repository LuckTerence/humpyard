# Spec Delta

## ADDED Requirements

### Requirement: ctl overrides manages overrides
`humpyard ctl overrides` SHALL provide `list` (with `--history`), `add` (with exactly one of `--disable-target`, `--drain-provider` or `--pin ROUTE=TARGET`, a required `--reason`, and an optional `--for` duration) and `remove ID`. `add` SHALL print the override's id and expiry. A refusal because the key may not manage overrides SHALL exit 1.

#### Scenario: Taking a target out for two hours
- **WHEN** `humpyard ctl overrides add --disable-target T --reason "slow" --for 2h` is run with a permitted key
- **THEN** it prints the new override's id and its expiry about two hours ahead

#### Scenario: A reason is required
- **WHEN** `add` is run without `--reason`
- **THEN** it fails before sending anything
