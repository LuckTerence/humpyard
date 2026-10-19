# 0025. The UI has no state of its own

- Status: accepted
- Date: 2026-10-18
- Supersedes: none
- Change: `openspec/changes/define-ui-config-model`

## Context

An operator UI is planned (a read-only status page first, an optional admin GUI later). Tools that grow a UI
usually grow a second home for configuration, and the two drift: a change made in the UI is overwritten by the
next deploy, or the file and the running gateway disagree and nobody can say which is right. Here the
configuration is code, rendered by a pipeline into a read-only file, so a UI that writes configuration would lose
to the next apply, and operators would stop trusting either.

## Decision

The UI holds no state of its own. Three kinds of change, each with one home:

1. **Structural configuration** (providers, targets, routes, key policy, prices) lives only in the configuration
   file. A UI change is a patch against it, validated with the real loader before it is offered. With a read-only
   file the patch is handed to the operator's own pipeline. With a writable file and an explicit opt-in the UI may
   write it atomically, conditional on the fingerprint it read, so a stale edit is refused.
2. **Operational actions** (take a target out of rotation, drain a provider, pin a route, force a cooldown) are
   expiring runtime overrides in the ledger, not configuration. They always expire, can only restrict or steer
   existing things, cannot cross a strict class, are audited, and are visible on decisions, in `explain` and in
   `overrides_active`.
3. **Generated data** keeps its own file and strict loader (ADR 0022).

The gateway exposes its effective configuration with the source of every value (`file`, `generated`,
`override`) and its configuration fingerprint, so drift between the repository and the running gateway can be
detected whether or not a UI exists.

**Keys are credentials, not configuration**, so they may be managed in the database, but their authority is not.
The file defines key profiles (routes, classes, budget ceilings, a maximum lifetime, a cap on active keys). A
database key names a profile and may only narrow it: its limits are the lesser of its own and the profile's, its
routes a subset, its expiry no later. Every database key expires, a key whose profile can reach paid routes has a
budget, the secret is shown once, and revocation is immediate and audited. Config-defined keys stay read-only to
any UI. Until a per-profile rate limit or concurrency cap exists, database keys are for the operator's own agents
and tools, not for other people.

## Alternatives considered

- **The UI writes a database overlay that is merged over the file.** Two sources of truth by design; the overlay
  silently outlives the file's intent. Rejected.
- **The UI always edits the file.** Impossible for a read-only mount, and in a pipeline-managed deployment the
  next deploy erases it. Kept only as an opt-in for a writable file, guarded by the fingerprint.
- **One-click apply through the operator's repository** (open a change from the UI). Useful, and built on the
  patch flow, but it needs a hook into a private system the public project cannot know about. Deferred until a
  real need appears.
- **Defer database keys entirely.** Too blunt: a leaked key has to be revocable immediately, which a file that
  waits for a review and a deploy cannot do. Keys differ from other configuration in how often they change and in
  who sees their secret.
- **Database keys with free-form policy.** A second source of truth for who may do what: a row could grant a
  route or class the file never intended. Rejected in favour of profiles that bound them.
- **No runtime actions at all.** Slows the response to a broken provider to a full review cycle. Expiring
  overrides give a fast, safe lever that cannot become permanent configuration.

## Consequences

Drift through the UI cannot happen, because the UI stores nothing that the gateway reads except bounded,
expiring overrides. Changes made through it are slower than a live edit, by design: they are reviewable. A
fast lever exists for emergencies, and it cleans up after itself. An architecture test keeps it true: only the
file writer and the override store may change effective configuration.
