# Spec Delta

## ADDED Requirements

### Requirement: The effective configuration is readable with provenance
The gateway SHALL provide `GET /v1/config`, which returns the configuration it is serving with secrets omitted, the fingerprint and load time of that configuration, and for every target and route its source, `file` or `generated`. It SHALL NOT include any API key value, any key hash or any provider header value.

#### Scenario: A value from each source
- **WHEN** the endpoint is read while a generated target exists
- **THEN** a hand-written target is labelled `file` and the generated target `generated`

#### Scenario: Pool routes show what they resolved to
- **WHEN** the endpoint is read and a route is a pool
- **THEN** the route lists its resolved member targets

#### Scenario: No secrets
- **WHEN** a provider has an API key and a header value
- **THEN** the response says whether a key is set and names the header, but contains neither the key nor the header value

### Requirement: Reading the configuration is opt-in per key
A key SHALL be able to read the effective configuration only if it sets `read_config = true`; any other key SHALL receive HTTP 403 with the error code `config_not_readable`. A gateway with no keys configured SHALL allow the read.

#### Scenario: An ordinary key
- **WHEN** a key without `read_config` requests the endpoint
- **THEN** the answer is 403 `config_not_readable` and nothing of the configuration is returned

#### Scenario: An operator key
- **WHEN** a key with `read_config = true` requests the endpoint
- **THEN** the configuration is returned

#### Scenario: The fingerprint follows a reload
- **WHEN** the configuration file changes and the gateway reloads it
- **THEN** the fingerprint in the response is the new one, equal to the one in `/v1/health`
