# Design

## The flow

1. **Providers.** Interactive on a terminal, flags otherwise. The built-in list is a small table of presets (name,
   base URL, the conventional key variable). Custom endpoints take a base URL and a variable name. The command
   reads the variable from its own environment; it reports presence by length only and never prints a value. A
   provider whose variable is unset is kept in the config but skipped for the live checks, and the command says so.
2. **Models.** For each provider with a key, fetch the model list (the same fetch the discovery sync uses) and drop
   what is not a chat model. Choose by a short per-preset preference list; when nothing on it is listed, ask in the
   interactive form and fail with the list in the non-interactive one.
3. **Confirm.** Send each chosen model one small request. A model that does not answer is dropped with the status,
   so a retired model can never reach the written file. This is the same "the provider must say it exists" principle
   as discovery (ADR 0022), applied once at setup.
4. **Write.** Providers with `api_key_env`, one target per kept model, a `fast` route over them and a `deep` route
   preferring the largest, `listen = "127.0.0.1:8080"`, and no keys. The file is loaded by the normal config parser
   before it is written; a file that would not load is never written. An existing output file is never overwritten.
5. **Prove.** The command starts the gateway in process on an ephemeral loopback port, sends one request to `fast`,
   prints the reply, which target served it and how long it took (no cost: `init` writes no prices), and stops it. Then it prints the `serve` command and client
   snippets.

## Exposing the gateway

`--listen` with a non-loopback address makes the command also create a virtual key with a budget, as `keygen` does,
and print the key once. Without a key a non-loopback listener is refused, matching the warning in the quick start.

## What it never does

It never prints, logs or writes a key value, never contacts anything but the providers the user chose, sends no
telemetry, and never edits an existing config. Provider presets are data, so a retired default is a one-line change,
and the live check means a stale preset degrades to a clear message instead of a broken file.
