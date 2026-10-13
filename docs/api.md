# API and endpoints

Clients can speak any of three protocols; providers are reached over OpenAI Chat Completions.
Set `"stream": true` for server-sent events.

```sh
# OpenAI Chat Completions
curl localhost:8080/v1/chat/completions -H 'content-type: application/json' \
  -d '{"model":"fast","messages":[{"role":"user","content":"hi"}]}'

# Anthropic Messages (e.g. Claude Code: ANTHROPIC_BASE_URL=http://localhost:8080)
curl localhost:8080/v1/messages -H 'content-type: application/json' \
  -d '{"model":"fast","max_tokens":100,"messages":[{"role":"user","content":"hi"}]}'

# OpenAI Responses (e.g. Codex CLI)
curl localhost:8080/v1/responses -H 'content-type: application/json' \
  -d '{"model":"fast","input":"hi"}'
```

## Everything else

| Endpoint | What it does |
|---|---|
| `GET /v1/models` | the routes and targets a key may use |
| `GET /healthz` | liveness |
| `GET /v1/key/info` | a key's limits, spend and budget state |
| `GET /v1/health` | which provider endpoints are being skipped, and which config version is loaded |
| `POST /v1/route/explain` | what a request would do, without sending it ([Routing](routing.md#selecting-the-route)) |
| `GET /v1/decisions` | what the router did for your past requests; every response carries an `x-humpyard-request-id` to look one up ([the decision log](budgets.md#the-decision-log)) |
| `GET /v1/stats` | how routes, targets and endpoints have performed: failure mix, fallbacks, latency percentiles, cost per request ([statistics](budgets.md#statistics)) |
| `GET /v1/quota` | the latest usage windows (percent used, status, reset time) of each provider that names a `quota_source`, with the age of the data, or why it is unknown ([Provider quota](routing.md#provider-quota)) |
| `POST /v1/messages/count_tokens`, `POST /v1/responses/input_tokens` | conservative local estimates: no provider call, no cost |

Verified with real Claude Code, Codex CLI and OpenClaw sessions: see [Clients](clients.md).
