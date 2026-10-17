# Spec Delta

## ADDED Requirements

### Requirement: Endpoints may declare what they can take
An endpoint MAY declare `context_window` (tokens), `tools` (boolean) and `vision` (boolean). An endpoint that declares nothing SHALL be treated as capable of any request.

#### Scenario: No declarations
- **WHEN** no endpoint declares a capability
- **THEN** every target is capable and routing is exactly as before

### Requirement: A target that cannot serve a request is removed before routing
For each request, the gateway SHALL remove from the route's targets every target none of whose endpoints can serve it, before the routing algorithm runs. An endpoint cannot serve a request that uses tools if it declares `tools = false`, one that carries images if it declares `vision = false`, or one whose prompt is certainly larger than its declared `context_window`.

#### Scenario: Tools
- **WHEN** a request uses tools, a route lists a target whose only endpoint declares `tools = false` and a target that does not, and the first target's upstream counts calls
- **THEN** the request is served by the second target and the first receives no call

#### Scenario: Context
- **WHEN** a request's prompt is certainly larger than one target's declared window
- **THEN** that target receives no call and a target that can take it serves the request

#### Scenario: Any endpoint is enough
- **WHEN** a target has one endpoint that can serve the request and one that cannot
- **THEN** the target stays in the plan

### Requirement: Removal happens only when it is certain
The gateway SHALL NOT remove a target for context size unless a deliberately low estimate of the prompt exceeds every declared window of the target, so that a target that could have served the request is never removed because of an over-estimate.

#### Scenario: Borderline prompt
- **WHEN** a prompt is close to a target's window but its low estimate is under it
- **THEN** the target stays in the plan and the existing overflow fallback handles a real overflow

### Requirement: A request no target can serve is refused with its reason
When capability removes every target of a route, the gateway SHALL refuse the request before any upstream call with HTTP 400 and outcome `no_capable_target`, naming the reason (tools, images or context).

#### Scenario: Images with no vision target
- **WHEN** a request carries an image and every target of the route declares `vision = false`
- **THEN** the response is 400 `no_capable_target` mentioning images, and no upstream is called

### Requirement: Capability only removes
Capability removal SHALL apply after allowed classes and the routing policy and before health skipping, and SHALL NOT add a target to a plan.

#### Scenario: Strict route
- **WHEN** a strict route's allowed target cannot serve a request
- **THEN** no excluded-class target is added to serve it; the request is refused with `no_capable_target`

### Requirement: Removals are recorded and explained
The decision SHALL list the targets removed for capability, `GET /v1/decisions` SHALL return them, and `explain` SHALL show each target's capability with the reason it would be removed.

#### Scenario: Explain a request with tools
- **WHEN** `explain` describes a request with tools for a route with a `tools = false` target
- **THEN** that target is marked not capable with the reason `tools`
