# Spec Delta

## ADDED Requirements

### Requirement: The effective configuration is readable with provenance and a fingerprint
The gateway SHALL expose its effective configuration, secrets omitted, in which every value is labelled `file`, `generated` or `override`, together with the fingerprint it computed when it loaded that configuration. The fingerprint SHALL also be a metric.

#### Scenario: A value from each source
- **WHEN** the effective configuration is read while a generated target and an override exist
- **THEN** a hand-written route is labelled `file`, the generated target `generated`, and the override `override`

#### Scenario: The fingerprint follows a reload
- **WHEN** the configuration file changes and the gateway reloads it
- **THEN** the exposed fingerprint and the metric change to the new value

### Requirement: Runtime overrides are bounded and expire
The gateway SHALL accept runtime overrides only to take a target out of rotation, drain a provider, pin a route to an existing target, or force a cooldown. An override SHALL carry an expiry no later than a configured maximum, SHALL expire without any action, and SHALL be recorded as an audit row. An override SHALL NOT create or change a target, provider, price or key, SHALL NOT widen what a key may use, and SHALL NOT make a target of a class that a strict route forbids eligible for that route.

#### Scenario: An override expires
- **WHEN** a target is taken out of rotation with a one-hour expiry
- **THEN** it is skipped for requests during that hour and eligible again afterwards, with no other action

#### Scenario: An override cannot add anything
- **WHEN** an override names a target, provider or key that is not in the configuration
- **THEN** it is refused and nothing changes

#### Scenario: An override cannot cross a strict class
- **WHEN** an override tries to pin a private-only route to a public target
- **THEN** it is refused

#### Scenario: Overrides are visible
- **WHEN** an override is active
- **THEN** decisions and `explain` for the affected route show it, `overrides_active` counts it with its age, and an alert condition exists for an override older than the configured threshold

### Requirement: Structural configuration changes are validated patches to the file
Any interface for changing providers, targets, routes, key policy or prices SHALL produce a patch against the configuration file, validated by loading the result with the real configuration loader before it is offered, and SHALL NOT change the running gateway's configuration by any other path.

#### Scenario: A read-only file
- **WHEN** a structural change is requested and the configuration file is not writable
- **THEN** a validated patch is returned and nothing is written

#### Scenario: A patch that does not load
- **WHEN** a requested change would produce a configuration the loader rejects
- **THEN** no patch is offered and the loader's reason is returned

### Requirement: A direct write needs an opt-in and a current fingerprint
An interface SHALL NOT write the configuration file unless the file is writable and the operator has opted in; a permitted write SHALL be atomic and SHALL be refused unless the fingerprint the editor last read is still the file's.

#### Scenario: Writable but not opted in
- **WHEN** the file is writable and the opt-in is absent
- **THEN** the change is returned as a patch and nothing is written

#### Scenario: A stale edit
- **WHEN** a direct write is requested with a fingerprint that is no longer the file's
- **THEN** the write is refused and the current fingerprint is returned

### Requirement: The interface keeps no configuration state of its own
No interface to the gateway SHALL persist configuration anywhere except the configuration file, the generated file, and the bounded override store.

#### Scenario: Deleting interface state
- **WHEN** all interface storage is deleted
- **THEN** the effective configuration of the gateway is unchanged
