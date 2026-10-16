# Spec Delta

## ADDED Requirements

### Requirement: Session id fallbacks
When none of the headers the routing engine recognizes as a session id carries a usable one, the gateway SHALL take the session id from the `session_id` header, then from `x-session-affinity`. A usable id is 1 to 256 visible ASCII characters; any other value SHALL be ignored without failing the request. The id SHALL be used wherever a session id is used (the decision, the usage rows and the routing engine's per-session state).

#### Scenario: A client that sends session_id
- **WHEN** a request carries `session_id: abc` and no recognized session header
- **THEN** its decision and usage rows carry the session `abc` and the routing engine sees it

#### Scenario: Recognized headers win
- **WHEN** a request carries both `x-switchyard-session-id: one` and `session_id: two`
- **THEN** the session is `one`

#### Scenario: Header order among the fallbacks
- **WHEN** a request carries both `session_id: first` and `x-session-affinity: second` and no recognized header
- **THEN** the session is `first`

#### Scenario: Unusable values
- **WHEN** the value is empty, longer than 256 characters, or contains a space or control character
- **THEN** it is ignored, the next candidate is tried, and the request is served normally

#### Scenario: Not a session
- **WHEN** a request carries only `x-client-request-id`
- **THEN** it has no session id
