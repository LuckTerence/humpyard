# Spec Delta

## ADDED Requirements

### Requirement: Target strengths
A target MAY declare `strengths`: a list of at most 8 labels, each 1 to 32 characters of lowercase letters, digits, `-` and `_`, with no duplicates. Strengths SHALL NOT affect any request except through the pool routes that name them, and SHALL NOT remove a target from a request.

#### Scenario: Declared strengths
- **WHEN** a target declares `strengths = ["coding", "general"]`
- **THEN** the configuration loads and the target is a candidate for pools named `coding` and `general`

#### Scenario: Invalid strengths
- **WHEN** a strength is empty, longer than 32 characters, contains an uppercase letter or a space, is repeated on a target, or a target lists more than 8
- **THEN** the configuration is rejected with a message naming the target and the strength

### Requirement: Pool routes
A `passthrough`, `fastest` or `random` route SHALL accept `pool = { strength = "...", min_context_window = N }` in place of `targets`, with `min_context_window` optional. The route's targets SHALL be every target that carries the strength and, when a window is given, has at least one endpoint declaring a `context_window` of at least that value, resolved when the configuration is loaded or reloaded and ordered by target name. `pool` and `targets` SHALL NOT be combined, and `weights` SHALL NOT be combined with `pool`.

#### Scenario: Resolved at load
- **WHEN** targets `a`, `b` and `c` carry `general` and `d` does not, and a route has `pool = { strength = "general" }`
- **THEN** the route's targets are `a`, `b`, `c` and a request to the route never considers `d`

#### Scenario: Minimum context window
- **WHEN** a pool sets `min_context_window = 32000` and target `a` declares a 64000 window, `b` declares 8000 and `c` declares none
- **THEN** only `a` is a member, and `b` and `c` are reported as left out with the reason

#### Scenario: Empty pool
- **WHEN** no target carries the pool's strength, or none qualifies
- **THEN** the configuration is rejected naming the strength and listing the strengths that exist

#### Scenario: Reload changes membership
- **WHEN** a reload adds the strength to another target
- **THEN** the route is rebuilt with the new membership and the reload log names the pool whose membership changed

#### Scenario: Strengths never refuse a request
- **WHEN** a request is served by a pool route
- **THEN** the only removals are the usual per-request ones (class, policy, capability, health); strengths remove nothing
