# Proposal

## Why

humpyard is configured as code and observed through health, stats and metrics. An operator UI is on the roadmap
(a read-only status page, later an optional admin GUI). Tools that add a UI usually add a second place where
configuration lives, and the two drift: someone changes a route in the UI, the file or the deployment pipeline
overwrites it on the next deploy, or the file says one thing and the running gateway another. In this project the
configuration is rendered by Ansible into a read-only ConfigMap, so a UI that writes config would be overwritten by
the next apply. Deciding the model before any UI code exists is much cheaper than retrofitting it.

## What Changes

This change is a design and a set of requirements; it adds no UI. It decides:

- **The UI has no state of its own.** Everything it shows or does is either read from the config file, the ledger
  or metrics; a patch against the config file; or a time-limited runtime override.
- **Structural configuration lives only in the file.** The UI produces a reviewable patch (comments and ordering
  preserved) and never changes the running gateway's configuration directly. An opt-in direct write exists for a
  writable file, guarded by the config fingerprint; a read-only file (a ConfigMap) falls back to patches.
- **Operational actions are expiring overrides**, not configuration: they can only restrict or steer things that
  already exist, always carry an expiry, are visible on decisions, `explain` and in metrics, and are audited.
- **Effective configuration with provenance**, and an exposed config fingerprint, so drift between the repository
  and the running gateway can be detected with or without a UI.
- **Database-managed keys stay out** until a real need appears; the architecture direction that allowed them
  beside config keys is amended.
- One-click apply (opening a change in the operator's repository) is **deferred**, not designed in.

## Capabilities

### New Capabilities
- `admin-config`: the effective-configuration view with provenance and fingerprint, the override store, and the
  rules for how a UI may change anything.

## Impact

Docs and a new ADR now. Later work, each step standing alone: the effective-config endpoint and fingerprint, the
override store and its metrics, the read-only status page, patch generation, and the optional direct-write mode.
No code changes in this change.
