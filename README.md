<div align="center">

# humpyard

**A gateway that sorts each LLM request onto the right model.**

One small Rust binary in front of your providers: it speaks OpenAI Chat, OpenAI Responses and
Anthropic Messages, routes by what each request *is*, fails over between providers, and tells you
what it decided and why.

[![ci](https://github.com/gschaetz/humpyard/actions/workflows/ci.yml/badge.svg)](https://github.com/gschaetz/humpyard/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/gschaetz/humpyard?include_prereleases)](https://github.com/gschaetz/humpyard/releases)
[![license](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

### [📖 Read the documentation at humpyard.dev](https://humpyard.dev)

</div>

> **Status: pre-alpha.** It runs real traffic (Claude Code, Codex, OpenClaw) but expect rough edges.

*Why the name:* a hump yard is the railroad yard where cars are classified and sent down the right
track, one by one. That is the job here, with requests and models.

## Why humpyard

Agent tools such as Claude Code and Codex send a stream of very different requests: a one-line
question, a 60,000-token tool loop, a sub-agent doing narrow work. Sending all of it to one model is
slow or expensive. humpyard is the **routing and policy layer** in front of your providers.

- **Content-aware routing.** Rules pick the pool from the key, headers, agent metadata, prompt size,
  tools and images. Clients can narrow, never widen, what a key may reach.
- **Explainable and measured.** Ask what a request *would* do before sending it, then see what it
  *did*: every decision, fallback and refusal is recorded, with latency and cost per route.
- **Resilient.** Ordered provider pools, per-endpoint circuit breakers, and control over which
  failures may fall through to the next pool.
- **Easy to run.** A static binary or a small container, a SQLite ledger with virtual keys and
  budgets, hot config reload, and money handled as integers that fail closed.

## Quick start

```sh
docker run -d -p 8080:8080 -e GROQ_API_KEY \
  -v "$PWD/config.toml:/etc/humpyard/config.toml:ro" ghcr.io/gschaetz/humpyard:0.7.0
curl localhost:8080/v1/chat/completions -H 'content-type: application/json' \
  -d '{"model":"fast","messages":[{"role":"user","content":"hi"}]}'
```

No Docker? Download a binary from the [releases](https://github.com/gschaetz/humpyard/releases),
or `cargo build --release`, then `humpyard serve --config config.toml`. A starting config is in
[examples/config.toml](examples/config.toml).

## Where to find what you need

| I want to... | Go to |
|---|---|
| install and run it (binary, container, macOS service) | [Deployment](https://humpyard.dev/deployment/) |
| write a config: providers, targets, routes, keys | [Configuration](https://humpyard.dev/configuration/) · [Examples](https://humpyard.dev/examples/) |
| point Claude Code, Codex or OpenClaw at it | [Clients](https://humpyard.dev/clients/) |
| route one `auto` model to the right pool | [Auto routing recipe](https://humpyard.dev/routing/#auto-routing-recipe) |
| understand failover, selectors and health | [Routing](https://humpyard.dev/routing/) |
| set budgets, read the usage and decision log, see stats | [Keys, budgets and the ledger](https://humpyard.dev/budgets/) |
| call the API and its endpoints | [API and endpoints](https://humpyard.dev/api/) |
| move from LiteLLM | [Migrating from LiteLLM](https://humpyard.dev/migrating-from-litellm/) |
| understand the design and why | [Architecture](https://humpyard.dev/architecture/) · [Decision records](https://humpyard.dev/adr/) |

## Contributing

Specs live in [`openspec/`](openspec/) and decisions in [`docs/adr/`](docs/adr/); the rules that must
keep holding, and what enforces them, are in [Invariants](https://humpyard.dev/invariants/).
Contributions need a DCO sign-off: see [CONTRIBUTING.md](CONTRIBUTING.md).
Locally: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

## License

Apache-2.0, see [LICENSE](LICENSE) and [NOTICE](NOTICE).
