# Spec Delta

## ADDED Requirements

### Requirement: Stated retry times
When an attempt fails with status 429 or 503 and the failure states a retry time, the gateway SHALL cool that endpoint for exactly that time (at least 1 second and at most `[health] max_retry_after_secs`, default 300), opening it immediately instead of after `failure_threshold` failures. The retry time SHALL be read from the upstream response when it is available as a header, and otherwise from a small set of well-known phrasings and fields in the error body, reading at most its first 4 KiB. A failure with no readable retry time SHALL behave as before.

#### Scenario: A 429 with a retry time
- **WHEN** an endpoint returns 429 with a body that says to try again in 20 seconds
- **THEN** the endpoint is skipped for 20 seconds even on its first failure, and one request probes it afterwards

#### Scenario: The cap
- **WHEN** the stated retry time is larger than `max_retry_after_secs`
- **THEN** the endpoint cools for `max_retry_after_secs`

#### Scenario: Unreadable or out of range
- **WHEN** the body has no recognized retry time, or one that is zero or negative or not a number
- **THEN** the failure is handled as before (counted toward `failure_threshold`, guessed cooldown)

#### Scenario: Other statuses
- **WHEN** a 500 response body says to try again in 20 seconds
- **THEN** the stated time is ignored

#### Scenario: Late failures
- **WHEN** a request sent before the endpoint cooled fails while it is cooling
- **THEN** the cooldown is neither shortened nor extended

### Requirement: Cooldown source is visible
Endpoint health SHALL report whether the current cooldown came from a stated retry time or from consecutive failures, and the gateway SHALL count honored retry times by provider and source.

#### Scenario: Health shows the source
- **WHEN** an endpoint is cooling because of a stated retry time
- **THEN** its health entry shows the source `retry_after` and the time remaining
