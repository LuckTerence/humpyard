# Design

## The rule

The UI holds no state. Delete all of the UI's storage and nothing is lost, because anything it could persist is
already in the file, the ledger or an expiring override. The consequence is that configuration drift between a UI
and the gateway cannot happen: there is nothing for the UI to disagree with.

## Three kinds of change

1. **Structural configuration** (providers, targets, routes, key policy, prices): the file only. A UI change is a
   patch. The patch is produced by editing the TOML with a format-preserving editor so comments and ordering
   survive, and is validated by loading the result with the real loader before it is offered.
   - Read-only file (a Kubernetes ConfigMap): the UI offers the patch to copy, download or hand to the
     operator's own pipeline. Detected automatically.
   - Writable file and an explicit opt-in (`[admin] allow_config_writes`): the UI may write the file atomically,
     conditional on the fingerprint it last read, so a stale edit is refused. The file remains the single store;
     the existing reload picks the change up.
2. **Operational actions**: runtime overrides in the ledger. Examples: take a target out of rotation, drain a
   provider, pin a route to a target, force a cooldown. Rules: always an expiry (default a few hours, bounded by
   a maximum); only restrict or steer existing targets and routes (never create targets, providers, prices or
   keys, never widen what a key may use, never reach a class a strict route forbids); recorded as audit rows;
   marked on decisions and `explain`; counted in `overrides_active` with an age; an alert fires for an old one.
   They apply in the planning stage as one more subtract-only or reorder-only input, like health.
3. **Generated data**: the discovery file keeps its own file and strict loader (ADR 0022).

## Drift detection without a UI

The gateway already computes a fingerprint when it loads configuration. Expose it (an endpoint and a metric) and
let the operator's pipeline publish the fingerprint of the configuration it rendered; a mismatch is an alert.
This catches drift from any cause, UI or not.

## Provenance

The effective-configuration view labels each value as `file`, `generated` or `override`, so an operator can see
why the gateway behaves as it does and an editor knows what it may touch.

## Keys

A key is a credential with a lifecycle, not a structural setting: it is created, rotated, expired and revoked
constantly, a leaked one must die immediately (not after a review and an apply), and git can only ever see its
hash. What must not drift is the *authority* attached to it.

- **The file defines key profiles** (for example `agent`, `trial`): allowed routes, allowed classes, budget
  ceilings (`daily_usd`, `monthly_usd`, `daily_tokens`, `monthly_tokens`, the `over_budget` mode), a maximum
  lifetime, and `max_active_keys`.
- **The database owns the credential and its lifecycle**: the hash, a label, the profile, an expiry, last use,
  revoked-or-not, and optional *narrowing*. A key's effective limits are `min(key, profile)` for every limit; its
  routes are a subset of the profile's; its expiry is no later than the profile allows; `block` may replace
  `free_only` but never the reverse. A database key can never exceed its profile.
- **Every database key has a profile and an expiry** (a default applies). A key whose profile can reach paid
  routes must have a budget.
- **`max_active_keys` bounds the total.** The ceiling bounds each key, not their sum, so a profile caps how many
  may be active. Minting needs the admin credential, which is the real boundary; an aggregate budget across a
  profile's keys would need shared counters and is deferred unless keys are shared with others.
- **The secret is shown once**, when the key is created; afterwards only the hash and metadata exist.
- **Revocation is immediate**, takes effect without a reload, and writes an audit row.
- **Config-defined keys stay read-only** to any UI, for service accounts, and are listed with their origin
  beside database keys.
- **Sharing gate.** Humpyard has no per-key rate limit or concurrency cap. Before keys are given to people other
  than the operator, profiles need one; until then database keys are for the operator's own agents and tools.

## Enforcement

An architecture test fails the build if any code path other than the file writer and the override store can
change the effective configuration, and a test pins that overrides cannot add entities.

## Deferred

One-click apply (open a change in the operator's repository through a pluggable hook) is not designed here. The
patch flow is the interface it would sit on, so adding it later changes nothing underneath.
