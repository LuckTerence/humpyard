# Proposal

## Why

An operator can see what the gateway did (`decisions`, `stats`) but not what it is configured to do. Whether the running
gateway matches the repository, which targets the discovery sidecar added, and what a pool resolved to are answered by
reading a ConfigMap and a generated file by hand. ADR 0025 and `define-ui-config-model` (task 2.1) decided that the
effective configuration, with where each part came from and a fingerprint, is the first thing an interface needs: it
makes drift visible with or without a UI. This is that first slice, and it gives `humpyard ctl` its first command
beyond reading what happened.

## What Changes

- `GET /v1/config` returns the effective configuration with secrets omitted: providers, targets (with endpoints,
  prices, capabilities, class, strengths, quality), routes (pool routes with their resolved members), selectors and
  the main settings. Each target and route is labelled with its source, `file` or `generated`.
- It carries the fingerprint and the load time, and the fingerprint is also a metric.
- Reading it is an opt-in per key (`read_config = true`), because it shows provider addresses and prices that an
  ordinary consumer key has no need for. A gateway with no keys is open, and so is this endpoint.
- `humpyard ctl config` shows it (`--json` unchanged, `--fingerprint` prints only the fingerprint), and
  `humpyard check-config` prints the fingerprint it would have, so a repository can be compared with a running gateway.
- Out of scope, each its own later change under `define-ui-config-model`: the `override` source (tasks 3.1), a status
  page, patches and direct writes, profiles and database keys.

## Capabilities

### Modified Capabilities
- `gateway-api`: the effective configuration endpoint.
- `metrics`: the configuration fingerprint.
- `cli`: `ctl config` and the fingerprint in `check-config`.

## Impact

One endpoint, one key setting, one metric and two CLI changes. No change to routing or to what any existing key can do.
