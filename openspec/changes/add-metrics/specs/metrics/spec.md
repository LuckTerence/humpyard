# Spec Delta

## ADDED Requirements

### Requirement: Metrics are served only when enabled
The gateway SHALL serve `GET /metrics` only when `[metrics] enabled = true`. Otherwise the path SHALL behave as any unknown path.

#### Scenario: Disabled by default
- **WHEN** the gateway runs with no `[metrics]` block
- **THEN** `GET /metrics` returns 404

### Requirement: Metrics have their own credential
On the main listener, `/metrics` SHALL require the configured metrics credential and SHALL refuse a virtual key. The metrics credential SHALL NOT authorize any other endpoint. Enabling metrics on the main listener without a credential SHALL fail validation.

#### Scenario: Client key is refused
- **WHEN** a request to `/metrics` carries a valid virtual key but not the metrics credential
- **THEN** the response is 401

#### Scenario: Metrics credential cannot call inference
- **WHEN** a request to `/v1/chat/completions` carries the metrics credential
- **THEN** the response is 401

### Requirement: Labels come only from configuration or fixed sets
Every metric label value SHALL be a name from the loaded configuration or a member of a fixed set. A string supplied by a client (requested model, header, tag, prompt) SHALL NOT become a label value.

#### Scenario: Unknown model name
- **WHEN** a client requests a model name no rule matches
- **THEN** the request is counted under `route="none"` and the requested name appears in no series

### Requirement: Metrics never carry identity or content
By default no series SHALL carry a key id, key hash, tenant or request content. Key ids SHALL appear as a label only when `key_labels = true`.

#### Scenario: Key labels are off
- **WHEN** a keyed request completes
- **THEN** no exposed series contains the key id

### Requirement: Exposition is valid and monotonic
The output SHALL be valid Prometheus text format with HELP and TYPE lines, counters SHALL never decrease across scrapes within a process, and histogram buckets SHALL be cumulative.

#### Scenario: Strict parse
- **WHEN** the output is parsed by a strict text-format reader
- **THEN** it parses with no error and every histogram's buckets are non-decreasing

### Requirement: Metrics never slow or fail a request
Updating the registry SHALL be non-blocking and SHALL NOT fail or delay a request. A scrape SHALL NOT take a lock that a request path holds.

#### Scenario: Concurrent scrape
- **WHEN** requests are served while `/metrics` is scraped repeatedly
- **THEN** no request fails or is measurably delayed by the scrape

### Requirement: Registry is shaped for other views
The registry SHALL expose a read-only snapshot of every metric with its name, type, help text and label sets, from which the text encoder is built, so that another view can be added without a second source of numbers.

#### Scenario: Snapshot matches exposition
- **WHEN** the snapshot is rendered as text
- **THEN** it equals the `/metrics` body for the same instant
