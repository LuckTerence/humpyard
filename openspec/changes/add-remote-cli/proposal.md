# Proposal

## Why

An operator running humpyard in a cluster has no way to look at it from their own machine except `curl` and a
metrics dashboard. The gateway already answers questions about itself (`/v1/health`, `/v1/stats`, `/v1/decisions`,
`/v1/quota`, `/v1/route/explain`, `/v1/key/info`, `/v1/models`), but each needs a hand-built request, a key in the
right header and some JSON reading. ADR 0025 plans a UI as a second client of the same API, and a command-line
client comes first for the reasons that ADR gives: it forces the API to be right, it scripts and tests easily, and
it works over a port-forward on a machine with no browser.

## What Changes

- A new `humpyard ctl` command: a read-only client for a running gateway, local or remote.
- Subcommands over the existing endpoints: `status` (one-screen summary), `models`, `health`, `stats`,
  `decisions`, `quota`, `explain` and `whoami` (key info).
- The gateway is named by `--url` or `HUMPYARD_URL`. The key is read only from the environment variable named by
  `--key-env` (default `HUMPYARD_API_KEY`), never from a flag value.
- Plain-text output by default and `--json` for scripts.
- Safe by construction: no redirects, no plain `http://` to a non-loopback host unless asked, never prints the key.
- Out of scope, each its own later change under ADR 0025: changing anything (patches, overrides, database keys),
  an effective-config view with provenance, watch mode, shell completion and any new server endpoint.

## Capabilities

### Modified Capabilities
- `cli`: the `ctl` command.

## Impact

One new subcommand and module in the same binary, so the client and the API it reads ship in lockstep. No server
change. It uses a key's own scope, exactly as `curl` would: a key sees its own decisions and statistics.
