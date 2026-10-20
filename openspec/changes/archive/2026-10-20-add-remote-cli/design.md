# Design

## Shape

`humpyard ctl [--url URL] [--key-env NAME] [--json] <command>`. It is a thin client: each subcommand is one request
to an existing endpoint, a small renderer for the answer, and `--json` prints the response body unchanged.

| Command | Request | Shows |
|---|---|---|
| `status` | `/healthz`, `/v1/health`, `/v1/key/info` | reachable, config version, endpoints being skipped, key spend |
| `models` | `/v1/models` | the routes and targets this key may use |
| `health` | `/v1/health` | skipped endpoints, cooldowns, loaded config version |
| `stats` | `/v1/stats` (`--route`, `--since`) | failure mix, fallbacks, latency percentiles, cost per request |
| `decisions` | `/v1/decisions` (`--limit`, `--request-id`) | what the router did for this key's requests |
| `quota` | `/v1/quota` | provider quota state |
| `explain` | `POST /v1/route/explain` | what a request would do (`--model`, or `--body FILE`) |
| `whoami` | `/v1/key/info` | limits, spend, budget state |

## Connecting safely

The key travels in the `Authorization` header, so the client refuses to send it where it could be read or
redirected. Redirects are never followed. A `http://` URL is accepted for a loopback host (a port-forward, a local
gateway) and refused for any other unless `--allow-insecure-http` is given, with a message saying why; `https://`
is accepted anywhere. The key value is read from the named environment variable, never from a flag (which would
land in shell history and process listings), is never printed, and an unset variable is an error that names the
variable. Errors report the status and the gateway's message, never the request headers.

## Exit codes

0 when the gateway answered, 1 when it answered with a refusal (401, 403, 404, 429), 2 when it could not be
reached or the answer was not understood. Scripts can tell "the key is wrong" from "the gateway is down".

## Why not an admin API first

Everything here is already served and scoped to the caller's key, so the first useful client needs no server work
and cannot change anything. Writes (patches, expiring overrides, database-managed keys) belong to the pieces
ADR 0025 already lists, each of which adds an endpoint and a matching `ctl` subcommand when it ships. Because
config stays defined in files (in this cluster, a ConfigMap), the write subcommands will print a patch or set an
expiring override and will never edit the running configuration directly.
