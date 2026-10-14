# Security policy

humpyard sits between your tools and your providers, so it handles API keys and the traffic that
carries them. Security reports are welcome and taken seriously.

## Reporting a vulnerability

Please **do not open a public issue** for a security problem.

Use GitHub's private reporting: on the repository, open **Security**, then **Report a
vulnerability**. Include what you found, how to reproduce it, and the version
(`humpyard --version`). If private reporting is not available to you, ask for a private channel in
the [Discord](https://discord.gg/qfTGr7KTfq) without describing the problem in public.

You can expect an acknowledgement within a few days. This is a small, pre-alpha project maintained
on a best-effort basis, so fixes follow severity, not a fixed schedule. Reporters are credited in
the release notes unless they prefer not to be.

## Supported versions

Only the latest release receives fixes. humpyard is pre-alpha: upgrade rather than expecting
patches to older versions.

## What counts

Reports we care most about:

- a way to **read or leak a key**: a provider key in a log, response, error or decision row, or a
  virtual key stored or compared in a way that exposes it;
- **authentication or authorization bypass**: using a route, budget or endpoint a key should not
  reach, or a client widening what its key allows;
- **budget bypass** that spends beyond a configured limit;
- **server-side request forgery** or credentials sent to an unintended host, including through
  provider URLs, the quota poller or the price importer;
- crashes or unbounded resource use triggered by a crafted request or upstream response.

How keys are meant to be handled is written down in
[docs/invariants.md](docs/invariants.md): virtual keys are stored only as hashes, provider keys come
from the environment and never appear in logs or responses, and the decision log keeps routing
metadata, never prompts or responses.

## Not in scope

- Problems that need an attacker to already control your config file, your environment or your
  host.
- The behavior of a provider or model you route to.
- Missing hardening in a deployment you wrote around humpyard (for example exposing the port to the
  internet with no auth in front of it): see [Deployment](docs/deployment.md).
