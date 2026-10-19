# Proposal

## Why

A new user's first contact with humpyard is writing a config by hand. A clean-machine run of the README quick start
failed with a 404 because the example named a model its provider had since retired, and a missing key was only
discovered when the container exited. Hand-maintained example configs go stale, and every stale example costs a
first impression. A guided command that asks a few questions, checks the answers against the live providers and
proves one request works removes that class of failure at the source, and is the path from "installed" to "running"
that the project is missing.

## What Changes

- A new `humpyard init` command that writes a working `config.toml`.
- It asks which providers to use from a short built-in list (or a custom OpenAI-compatible endpoint), reads each
  key from the environment variable the user names, and never asks for or stores a key value.
- It lists each provider's models, keeps only models that exist now, and confirms each chosen model with one small
  request before writing it.
- It writes a loopback-only config with a `fast` and a `deep` route, loads it with the same parser `serve` uses,
  sends one test request through it, and prints the answer, the route, the cost and a ready snippet for Claude Code,
  Codex or OpenClaw.
- A non-interactive form takes the same choices as flags, for scripts and CI.
- Out of scope: running as a service, editing an existing config (the admin work in ADR 0025), prompting for or
  storing key values, and a remote mode (it follows the admin API).

## Capabilities

### Modified Capabilities
- `cli`: the `init` command.

## Impact

A new subcommand and a small table of provider presets. It reuses the model-list fetching and the config loader
and changes no request-path behavior. The README quick start can then point at `humpyard init` first.
