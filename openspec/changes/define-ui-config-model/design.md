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

Config-defined keys are read-only to any UI. Database-managed keys are the one feature in the earlier direction
that would give a second source of truth for who may do what; they are deferred. If a need arises they must live
in a visibly separate namespace, show their origin, and be unable to grant more than the config allows.

## Enforcement

An architecture test fails the build if any code path other than the file writer and the override store can
change the effective configuration, and a test pins that overrides cannot add entities.

## Deferred

One-click apply (open a change in the operator's repository through a pluggable hook) is not designed here. The
patch flow is the interface it would sit on, so adding it later changes nothing underneath.
